use super::{AppState, driver_for};
use crate::db::query::first_keyword;
use crate::db::sql_guard::ensure_read_only_sql;
use crate::db::sql_script::split_sql_script;
use crate::history;
use crate::models::{
    ScriptOptions, SqlFilePreview, SqlFileStatementPreview, SqlFileViolation, SqlScriptFailure,
    SqlScriptProgress, SqlScriptSummary,
};
use crate::storage;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const MAX_SCRIPT_BYTES: u64 = 100 * 1024 * 1024;
const PREVIEW_LIMIT: usize = 50;
const VIOLATION_LIMIT: usize = 20;

fn preview_text(sql: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in sql.chars() {
        if character.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;
        normalized.push(character);
        if normalized.chars().count() >= 201 {
            break;
        }
    }
    let mut preview: String = normalized.chars().take(200).collect();
    if sql.chars().count() > preview.chars().count() {
        preview.push('…');
    }
    preview
}

fn statement_kind(sql: &str) -> &'static str {
    match first_keyword(sql).to_ascii_lowercase().as_str() {
        "select" | "with" | "show" | "describe" | "desc" | "explain" | "pragma" => "query",
        "insert" | "update" | "delete" | "merge" | "replace" => "dml",
        "create" | "alter" | "drop" | "truncate" | "rename" | "comment" => "ddl",
        "begin" | "start" | "commit" | "rollback" | "savepoint" | "release" | "end" => {
            "transaction"
        }
        _ => "other",
    }
}

fn is_transaction_control(sql: &str) -> bool {
    matches!(
        first_keyword(sql).to_ascii_lowercase().as_str(),
        "begin" | "start" | "commit" | "rollback" | "savepoint" | "release" | "end"
    )
}

fn read_script(path: &str) -> Result<(String, u64), String> {
    let target = Path::new(path.trim());
    if path.trim().is_empty() {
        return Err("SQL 文件路径不能为空".to_string());
    }
    let metadata =
        std::fs::metadata(target).map_err(|error| format!("读取 SQL 文件失败: {error}"))?;
    if !metadata.is_file() {
        return Err("所选路径不是文件".to_string());
    }
    if metadata.len() > MAX_SCRIPT_BYTES {
        return Err(format!(
            "SQL 文件超过 100 MB 上限（当前 {}）",
            metadata.len()
        ));
    }
    let bytes = std::fs::read(target).map_err(|error| format!("读取 SQL 文件失败: {error}"))?;
    if bytes.contains(&0) {
        return Err("SQL 文件包含 NUL 字节，已拒绝执行".to_string());
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| "SQL 文件不是有效的 UTF-8 编码，暂不支持自动猜测编码".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
    Ok((text, metadata.len()))
}

fn emit_progress(app: Option<&AppHandle>, progress: SqlScriptProgress) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit("sql-script:progress", progress);
}

fn summarize_guard_error(
    statements: &[crate::db::sql_script::ScriptStatement],
    driver: &str,
) -> Result<usize, Vec<SqlFileViolation>> {
    let mut violations = Vec::new();
    let mut total = 0usize;
    for (index, statement) in statements.iter().enumerate() {
        if let Err(message) = ensure_read_only_sql(&statement.sql, driver) {
            total += 1;
            if violations.len() < VIOLATION_LIMIT {
                violations.push(SqlFileViolation {
                    index: index + 1,
                    line: statement.line,
                    preview: preview_text(&statement.sql),
                    message,
                });
            }
        }
    }
    if total > 0 {
        Err(violations)
    } else {
        Ok(total)
    }
}

#[tauri::command]
pub async fn preview_sql_file(
    app: AppHandle,
    connection_id: String,
    path: String,
) -> Result<SqlFilePreview, String> {
    let config = storage::load_connections(&app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持执行 SQL 文件".to_string());
    }
    let (text, bytes) = read_script(&path)?;
    let statements = split_sql_script(&text)?;
    if statements.is_empty() {
        return Err("SQL 文件没有可执行语句".to_string());
    }
    let mut violations = Vec::new();
    let mut violation_count = 0usize;
    if config.read_only {
        for (index, statement) in statements.iter().enumerate() {
            if let Err(message) = ensure_read_only_sql(&statement.sql, &config.driver) {
                violation_count += 1;
                if violations.len() < VIOLATION_LIMIT {
                    violations.push(SqlFileViolation {
                        index: index + 1,
                        line: statement.line,
                        preview: preview_text(&statement.sql),
                        message,
                    });
                }
            }
        }
    }
    let previews = statements
        .iter()
        .take(PREVIEW_LIMIT)
        .enumerate()
        .map(|(index, statement)| SqlFileStatementPreview {
            index: index + 1,
            line: statement.line,
            preview: preview_text(&statement.sql),
            kind: statement_kind(&statement.sql).to_string(),
        })
        .collect();
    Ok(SqlFilePreview {
        path: path.trim().to_string(),
        bytes,
        statement_count: statements.len(),
        statements: previews,
        violation_count,
        violations,
        read_only: config.read_only,
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_script_with_session(
    app: Option<&AppHandle>,
    session: &mut Box<dyn crate::db::DatabaseSession>,
    statements: &[crate::db::sql_script::ScriptStatement],
    options: &ScriptOptions,
    driver_kind: &str,
    cancel: &AtomicBool,
    operation_id: Option<&str>,
    path: &str,
) -> Result<SqlScriptSummary, String> {
    let started = Instant::now();
    let total = statements.len();
    let mut notes = Vec::new();
    let mut use_transaction = options.use_transaction;
    let mut stop_on_error = options.stop_on_error;
    if use_transaction
        && statements
            .iter()
            .any(|item| is_transaction_control(&item.sql))
    {
        use_transaction = false;
        notes.push("脚本包含 BEGIN / COMMIT 等事务控制语句，已关闭外层整脚本事务。".to_string());
    }
    if use_transaction && !stop_on_error {
        stop_on_error = true;
        notes.push("整脚本事务开启时，遇错会停止并回滚。".to_string());
    }
    if use_transaction && driver_kind == "mysql" {
        notes.push("MySQL 的 DDL 会隐式提交，整脚本事务无法保证所有语句都可回滚。".to_string());
    }

    if use_transaction {
        session.begin().await?;
    }
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut rows_affected = 0u64;
    let mut failures = Vec::new();
    let mut cancelled = false;

    for (index, statement) in statements.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            skipped = total - index;
            break;
        }
        let preview = preview_text(&statement.sql);
        emit_progress(
            app,
            SqlScriptProgress {
                operation_id: operation_id.map(str::to_string),
                index: index + 1,
                total,
                line: statement.line,
                preview: preview.clone(),
                status: "running".to_string(),
                rows_affected: 0,
                elapsed_ms: started.elapsed().as_millis() as u64,
                error: None,
            },
        );
        let statement_started = Instant::now();
        match session.execute(&statement.sql, 100).await {
            Ok(result) => {
                succeeded += 1;
                rows_affected += result.rows_affected;
                emit_progress(
                    app,
                    SqlScriptProgress {
                        operation_id: operation_id.map(str::to_string),
                        index: index + 1,
                        total,
                        line: statement.line,
                        preview,
                        status: "success".to_string(),
                        rows_affected: result.rows_affected,
                        elapsed_ms: statement_started.elapsed().as_millis() as u64,
                        error: None,
                    },
                );
            }
            Err(message) => {
                failed += 1;
                failures.push(SqlScriptFailure {
                    index: index + 1,
                    line: statement.line,
                    preview: preview.clone(),
                    message: message.clone(),
                });
                emit_progress(
                    app,
                    SqlScriptProgress {
                        operation_id: operation_id.map(str::to_string),
                        index: index + 1,
                        total,
                        line: statement.line,
                        preview,
                        status: "failed".to_string(),
                        rows_affected: 0,
                        elapsed_ms: statement_started.elapsed().as_millis() as u64,
                        error: Some(message),
                    },
                );
                if stop_on_error {
                    skipped = total - index - 1;
                    break;
                }
            }
        }
    }

    let mut rolled_back = false;
    if use_transaction {
        if cancelled || failed > 0 {
            if let Err(error) = session.rollback().await {
                notes.push(format!("回滚整脚本事务失败：{error}"));
            } else {
                rolled_back = true;
            }
        } else if let Err(error) = session.commit().await {
            return Err(format!("提交整脚本事务失败: {error}"));
        }
    }

    Ok(SqlScriptSummary {
        path: path.trim().to_string(),
        total,
        succeeded,
        failed,
        skipped,
        rows_affected,
        elapsed_ms: started.elapsed().as_millis() as u64,
        rolled_back,
        cancelled,
        failures,
        notes,
    })
}

#[tauri::command]
pub async fn execute_sql_file(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    options: ScriptOptions,
    operation_id: Option<String>,
) -> Result<SqlScriptSummary, String> {
    let config = storage::load_connections(&app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持执行 SQL 文件".to_string());
    }
    let (text, _) = read_script(&path)?;
    let statements = split_sql_script(&text)?;
    if statements.is_empty() {
        return Err("SQL 文件没有可执行语句".to_string());
    }
    if config.read_only {
        if let Err(violations) = summarize_guard_error(&statements, &config.driver) {
            let first = violations
                .first()
                .map(|item| {
                    format!(
                        "第 {} 条（文件第 {} 行）：{}",
                        item.index, item.line, item.message
                    )
                })
                .unwrap_or_else(|| "包含写语句".to_string());
            return Err(format!(
                "只读连接拒绝执行：{first}。脚本包含 {} 条不允许的语句。",
                violations.len().max(1)
            ));
        }
    }

    let driver = driver_for(&app, &state, &connection_id).await?;
    let mut session = driver.open_session().await?;
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(id) = &operation_id {
        state
            .export_cancels
            .lock()
            .await
            .insert(id.clone(), cancel.clone());
    }

    let outcome = run_script_with_session(
        Some(&app),
        &mut session,
        &statements,
        &options,
        &config.driver,
        &cancel,
        operation_id.as_deref(),
        &path,
    )
    .await;

    if let Some(id) = &operation_id {
        state.export_cancels.lock().await.remove(id);
    }

    if let Ok(summary) = &outcome {
        let file_name = Path::new(path.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("script.sql");
        let success = summary.failed == 0 && !summary.cancelled;
        let history_error = if success {
            None
        } else if summary.cancelled {
            Some("执行已取消".to_string())
        } else {
            summary
                .failures
                .first()
                .map(|item| format!("第 {} 条执行失败：{}", item.index, item.message))
        };
        let _ = history::record(
            &app,
            &connection_id,
            &config.name,
            &format!("-- SQL 脚本：{file_name}（{} 条语句）", summary.total),
            summary.elapsed_ms,
            success,
            history_error,
        );
    }

    outcome
}

#[tauri::command]
pub async fn cancel_sql_script(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<(), String> {
    if let Some(flag) = state.export_cancels.lock().await.get(&operation_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sql_script::split_sql_script;
    use crate::db::{DatabaseDriver, SqliteDriver};

    async fn session_for(path: &str) -> Box<dyn crate::db::DatabaseSession> {
        let driver = SqliteDriver::connect(path).await.expect("connect");
        driver.open_session().await.expect("session")
    }

    #[tokio::test]
    async fn runs_sqlite_script_and_rolls_back_on_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("script.db");
        let file_path = file.to_str().expect("path");
        let driver = SqliteDriver::connect(file_path).await.expect("connect");
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                100,
            )
            .await
            .expect("create table");

        let mut session = session_for(file_path).await;
        let statements = split_sql_script(
            "INSERT INTO items (id, name) VALUES (1, 'a');\nINSERT INTO missing VALUES (1);\nINSERT INTO items (id, name) VALUES (2, 'b');",
        )
        .expect("split");
        let cancel = AtomicBool::new(false);
        let summary = run_script_with_session(
            None,
            &mut session,
            &statements,
            &ScriptOptions {
                stop_on_error: true,
                use_transaction: true,
            },
            "sqlite",
            &cancel,
            None,
            "memory.sql",
        )
        .await
        .expect("run");
        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert!(summary.rolled_back);
        let rows = driver
            .read_rows("items", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert!(rows.rows.is_empty());

        let statements = split_sql_script(
            "INSERT INTO items (id, name) VALUES (3, 'c'); INSERT INTO items (id, name) VALUES (4, 'd');",
        )
        .expect("split");
        let summary = run_script_with_session(
            None,
            &mut session,
            &statements,
            &ScriptOptions {
                stop_on_error: true,
                use_transaction: true,
            },
            "sqlite",
            &cancel,
            None,
            "memory.sql",
        )
        .await
        .expect("run success");
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 0);
        assert!(!summary.rolled_back);
        let rows = driver
            .read_rows("items", 10, 0, None, Some("id"), Some("asc"))
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 2);
    }

    #[tokio::test]
    async fn script_transaction_control_disables_outer_transaction() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("script-tx.db");
        let file_path = file.to_str().expect("path");
        let driver = SqliteDriver::connect(file_path).await.expect("connect");
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                100,
            )
            .await
            .expect("create table");
        let mut session = session_for(file_path).await;
        let statements =
            split_sql_script("BEGIN; INSERT INTO items (id, name) VALUES (1, 'a'); COMMIT;")
                .expect("split");
        let cancel = AtomicBool::new(false);
        let summary = run_script_with_session(
            None,
            &mut session,
            &statements,
            &ScriptOptions {
                stop_on_error: true,
                use_transaction: true,
            },
            "sqlite",
            &cancel,
            None,
            "memory.sql",
        )
        .await
        .expect("run");
        assert!(summary.notes.iter().any(|note| note.contains("外层")));
        assert!(!summary.rolled_back);
        assert_eq!(summary.succeeded, 3);
        let rows = driver
            .read_rows("items", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 1);
    }

    #[test]
    fn detects_read_only_violations() {
        let statements = split_sql_script("SELECT 1; DELETE FROM items; SELECT 2;").expect("split");
        let violations =
            summarize_guard_error(&statements, "sqlite").expect_err("must reject writes");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].index, 2);
    }
}
