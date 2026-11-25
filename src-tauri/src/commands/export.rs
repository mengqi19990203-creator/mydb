use super::{AppState, driver_for};
use crate::db::DatabaseDriver;
use crate::models::{
    DatabaseExportFailure, DatabaseExportOptions, DatabaseExportProgress, DatabaseExportSummary,
    ExportObject, ForeignKeyInfo,
};
use crate::storage;
use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

const MAX_INSERT_BYTES: usize = 1_000_000;
const FATAL_PREFIX: &str = "__mydb_export_fatal__:";

fn fatal_error(message: String) -> String {
    format!("{FATAL_PREFIX}{message}")
}

fn strip_fatal(error: &str) -> String {
    error
        .strip_prefix(FATAL_PREFIX)
        .unwrap_or(error)
        .to_string()
}

fn quote(name: &str, driver: &str) -> String {
    if driver == "mysql" {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn qualified_scope(driver: &str, database: &str, schema: &str) -> String {
    if driver == "mysql" && !database.is_empty() {
        format!("{}.", quote(database, driver))
    } else if driver == "postgres" && !schema.is_empty() {
        format!("{}.", quote(schema, driver))
    } else {
        String::new()
    }
}

fn table_reference(driver: &str, database: &str, schema: &str, table: &str) -> String {
    // MySQL exports intentionally stay unqualified so the script can be
    // restored into whichever database is selected by the operator.
    if driver == "postgres" {
        format!(
            "{}{}",
            qualified_scope(driver, database, schema),
            quote(table, driver)
        )
    } else {
        quote(table, driver)
    }
}

fn ensure_semicolon(statement: &str) -> String {
    let trimmed = statement.trim();
    if trimmed.is_empty() {
        String::new()
    } else if trimmed.ends_with(';') {
        format!("{trimmed}\n")
    } else {
        format!("{trimmed};\n")
    }
}

fn comment_line(text: &str) -> String {
    let sanitized = text.replace(['\r', '\n'], " ");
    format!("-- {sanitized}\n")
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mydb-export.sql");
    let temp_name = format!(".{file_name}.mydb-tmp-{}", std::process::id());
    path.with_file_name(temp_name)
}

fn write_text(out: &mut BufWriter<File>, text: &str, bytes: &mut u64) -> Result<(), String> {
    out.write_all(text.as_bytes())
        .map_err(|error| fatal_error(format!("写入导出文件失败: {error}")))?;
    *bytes += text.len() as u64;
    Ok(())
}

fn emit_progress(
    app: Option<&AppHandle>,
    operation_id: Option<&str>,
    phase: &str,
    object_name: &str,
    object_index: usize,
    object_total: usize,
    rows: u64,
    bytes: u64,
    message: String,
) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit(
        "database-export:progress",
        DatabaseExportProgress {
            operation_id: operation_id.map(str::to_string),
            phase: phase.to_string(),
            object_name: object_name.to_string(),
            object_index,
            object_total,
            rows,
            bytes,
            message,
        },
    );
}

fn add_foreign_key_statement(
    driver: &str,
    schema: &str,
    table: &str,
    foreign_key: &ForeignKeyInfo,
) -> Option<String> {
    if foreign_key.columns.is_empty() || foreign_key.referenced_columns.is_empty() {
        return None;
    }
    let table_ref = if driver == "postgres" && !schema.is_empty() {
        format!("{}.{}", quote(schema, driver), quote(table, driver))
    } else {
        quote(table, driver)
    };
    let referenced = if driver == "postgres" && !schema.is_empty() {
        format!(
            "{}.{}",
            quote(schema, driver),
            quote(&foreign_key.referenced_table, driver)
        )
    } else {
        quote(&foreign_key.referenced_table, driver)
    };
    let columns = foreign_key
        .columns
        .iter()
        .map(|column| quote(column, driver))
        .collect::<Vec<_>>()
        .join(", ");
    let referenced_columns = foreign_key
        .referenced_columns
        .iter()
        .map(|column| quote(column, driver))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = format!(
        "ALTER TABLE {table_ref} ADD CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {referenced} ({referenced_columns})",
        quote(&foreign_key.name, driver)
    );
    if !foreign_key.on_delete.trim().is_empty()
        && !foreign_key.on_delete.eq_ignore_ascii_case("NO ACTION")
    {
        statement.push_str(&format!(" ON DELETE {}", foreign_key.on_delete));
    }
    if !foreign_key.on_update.trim().is_empty()
        && !foreign_key.on_update.eq_ignore_ascii_case("NO ACTION")
    {
        statement.push_str(&format!(" ON UPDATE {}", foreign_key.on_update));
    }
    statement.push(';');
    Some(statement)
}

fn mysql_routine_definition(ddl: &str) -> String {
    let trimmed = ddl.trim().trim_end_matches(';').trim_end();
    format!("DELIMITER $$\n{trimmed}$$\nDELIMITER ;\n")
}

async fn write_object_ddl(
    driver: &dyn DatabaseDriver,
    driver_kind: &str,
    kind: &str,
    name: &str,
    remove_definer: bool,
    out: &mut BufWriter<File>,
    bytes: &mut u64,
) -> Result<(), String> {
    let ddl = driver.object_ddl(kind, name).await?;
    let ddl = if driver_kind == "mysql" && remove_definer {
        crate::db::driver::strip_mysql_definer(&ddl)
    } else {
        ddl
    };
    if driver_kind == "mysql" && matches!(kind, "procedure" | "function" | "trigger" | "event") {
        write_text(out, &mysql_routine_definition(&ddl), bytes)
    } else {
        write_text(out, &ensure_semicolon(&ddl), bytes)
    }
}

async fn export_table_data(
    app: Option<&AppHandle>,
    driver: &dyn DatabaseDriver,
    driver_kind: &str,
    database: &str,
    schema: &str,
    table: &str,
    options: &DatabaseExportOptions,
    cancel: &AtomicBool,
    operation_id: Option<&str>,
    object_index: usize,
    object_total: usize,
    total_rows: &mut u64,
    total_bytes: &mut u64,
    out: &mut BufWriter<File>,
) -> Result<u64, String> {
    let (columns, insert_clause) = driver.dump_insert_plan(table).await?;
    if columns.is_empty() {
        return Ok(0);
    }
    let column_list = columns
        .iter()
        .map(|column| quote(column, driver_kind))
        .collect::<Vec<_>>()
        .join(", ");
    let table_ref = table_reference(driver_kind, database, schema, table);
    let prefix = format!("INSERT INTO {table_ref} ({column_list}){insert_clause} VALUES\n");
    let rows_per_insert = options.rows_per_insert.clamp(1, 500) as usize;

    let mut batch: Vec<String> = Vec::new();
    let mut batch_bytes = 0usize;
    let mut table_rows = 0u64;
    let mut last_progress = Instant::now();

    let flush_batch = |batch: &mut Vec<String>,
                       batch_bytes: &mut usize,
                       out: &mut BufWriter<File>,
                       bytes: &mut u64|
     -> Result<(), String> {
        if batch.is_empty() {
            return Ok(());
        }
        write_text(out, &prefix, bytes)?;
        for (index, row) in batch.iter().enumerate() {
            if index > 0 {
                write_text(out, ",\n  ", bytes)?;
            } else {
                write_text(out, "  ", bytes)?;
            }
            write_text(out, row, bytes)?;
        }
        write_text(out, ";\n", bytes)?;
        batch.clear();
        *batch_bytes = 0;
        Ok(())
    };

    let result = driver
        .stream_sql_literal_rows(table, &columns, &mut |values| {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            let row = format!(
                "({})",
                values
                    .iter()
                    .map(|value| value.as_deref().unwrap_or("NULL"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            batch_bytes += row.len();
            batch.push(row);
            table_rows += 1;
            if batch.len() >= rows_per_insert || batch_bytes >= MAX_INSERT_BYTES {
                if let Err(error) = flush_batch(&mut batch, &mut batch_bytes, out, total_bytes) {
                    return Err(error);
                }
            }
            if last_progress.elapsed() >= Duration::from_millis(500) {
                last_progress = Instant::now();
                emit_progress(
                    app,
                    operation_id,
                    "data",
                    table,
                    object_index,
                    object_total,
                    *total_rows + table_rows,
                    *total_bytes,
                    format!("正在导出 {table}：{table_rows} 行"),
                );
            }
            Ok(())
        })
        .await;

    match result {
        Ok(streamed) => {
            if let Err(error) = flush_batch(&mut batch, &mut batch_bytes, out, total_bytes) {
                return Err(error);
            }
            *total_rows += streamed;
            Ok(streamed)
        }
        Err(error) => {
            let _ = flush_batch(&mut batch, &mut batch_bytes, out, total_bytes);
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            *total_rows += table_rows;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn export_database_sql(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    objects: Vec<ExportObject>,
    options: DatabaseExportOptions,
    operation_id: Option<String>,
) -> Result<DatabaseExportSummary, String> {
    run_database_export(
        &app,
        state.inner(),
        connection_id,
        path,
        objects,
        options,
        operation_id,
    )
    .await
}

/// Shared by the export dialog and the in-app backup scheduler.
pub(crate) async fn run_database_export(
    app: &AppHandle,
    state: &AppState,
    connection_id: String,
    path: String,
    objects: Vec<ExportObject>,
    options: DatabaseExportOptions,
    operation_id: Option<String>,
) -> Result<DatabaseExportSummary, String> {
    if path.trim().is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    if objects.is_empty() {
        return Err("请至少选择一个对象".to_string());
    }
    if !options.include_structure && !options.include_data {
        return Err("请至少选择结构或数据".to_string());
    }

    let config = storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持 SQL 导出".to_string());
    }
    let driver_kind = config.driver.clone();
    let driver = driver_for(app, state, &connection_id).await?;
    let (database, schema) = driver.context().await;
    let database = database.unwrap_or_else(|| config.database.clone());
    let schema = schema.unwrap_or_default();

    let allowed = match driver_kind.as_str() {
        "mysql" => [
            "table",
            "view",
            "sequence",
            "procedure",
            "function",
            "trigger",
            "event",
        ]
        .as_slice(),
        "postgres" => [
            "table",
            "view",
            "matview",
            "sequence",
            "procedure",
            "function",
            "trigger",
        ]
        .as_slice(),
        _ => ["table", "view", "trigger"].as_slice(),
    };
    for object in &objects {
        if !allowed.contains(&object.kind.as_str()) {
            return Err(format!(
                "{} 不支持导出对象类型: {}",
                driver_kind, object.kind
            ));
        }
    }

    let mut objects = objects;
    objects.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
    let object_total = objects.len();

    let groups = |kind: &str| {
        objects
            .iter()
            .filter(|object| object.kind == kind)
            .cloned()
            .collect::<Vec<_>>()
    };
    let tables = groups("table");
    let views = groups("view");
    let matviews = groups("matview");
    let sequences = groups("sequence");
    let procedures = groups("procedure");
    let functions = groups("function");
    let triggers = groups("trigger");
    let events = groups("event");

    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(id) = &operation_id {
        state
            .export_cancels
            .lock()
            .await
            .insert(id.clone(), cancel.clone());
    }

    let target = PathBuf::from(path.trim());
    let temp_path = temp_path_for(&target);
    let result = export_database_inner(
        Some(app),
        driver.as_ref(),
        &driver_kind,
        &config.name,
        &database,
        &schema,
        &tables,
        &views,
        &matviews,
        &sequences,
        &procedures,
        &functions,
        &triggers,
        &events,
        &options,
        &cancel,
        operation_id.as_deref(),
        &target,
        &temp_path,
        object_total,
    )
    .await;

    if let Some(id) = &operation_id {
        state.export_cancels.lock().await.remove(id);
    }

    match result {
        Ok(summary) => Ok(summary),
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(strip_fatal(&error))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn export_database_inner(
    app: Option<&AppHandle>,
    driver: &dyn DatabaseDriver,
    driver_kind: &str,
    connection_name: &str,
    database: &str,
    schema: &str,
    tables: &[ExportObject],
    views: &[ExportObject],
    matviews: &[ExportObject],
    sequences: &[ExportObject],
    procedures: &[ExportObject],
    functions: &[ExportObject],
    triggers: &[ExportObject],
    events: &[ExportObject],
    options: &DatabaseExportOptions,
    cancel: &AtomicBool,
    operation_id: Option<&str>,
    target: &Path,
    temp_path: &Path,
    object_total: usize,
) -> Result<DatabaseExportSummary, String> {
    let _ = std::fs::remove_file(temp_path);
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(temp_path)
        .map_err(|error| format!("创建临时导出文件失败: {error}"))?;
    let mut out = BufWriter::new(file);
    let mut bytes = 0u64;
    let mut rows = 0u64;
    let mut failures: Vec<DatabaseExportFailure> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut processed = 0usize;
    let mut successes = 0usize;
    let scope = if !database.is_empty() {
        database.to_string()
    } else if !schema.is_empty() {
        schema.to_string()
    } else {
        "当前范围".to_string()
    };

    write_text(&mut out, &comment_line("mydb 逻辑导出"), &mut bytes)?;
    write_text(
        &mut out,
        &comment_line(&format!(
            "生成时间：{}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        )),
        &mut bytes,
    )?;
    write_text(
        &mut out,
        &comment_line(&format!(
            "连接：{connection_name}（{driver_kind}） · 范围：{scope}"
        )),
        &mut bytes,
    )?;
    write_text(
        &mut out,
        &comment_line(&format!(
            "选项：结构={} 数据={} 删除语句={} 每条 INSERT 行数={} 数据事务={}",
            yes_no(options.include_structure),
            yes_no(options.include_data),
            yes_no(options.include_drops),
            options.rows_per_insert.clamp(1, 500),
            yes_no(options.use_transaction)
        )),
        &mut bytes,
    )?;
    write_text(
        &mut out,
        &comment_line("逻辑备份，不替代数据库原生 dump / 物理备份；请在隔离环境验证后再用于生产。"),
        &mut bytes,
    )?;
    write_text(&mut out, "\n", &mut bytes)?;

    match driver_kind {
        "mysql" => {
            write_text(
                &mut out,
                "SET NAMES utf8mb4;\nSET FOREIGN_KEY_CHECKS=0;\n",
                &mut bytes,
            )?;
            if options.create_database && !database.is_empty() {
                write_text(
                    &mut out,
                    &format!(
                        "CREATE DATABASE IF NOT EXISTS {} DEFAULT CHARACTER SET utf8mb4;\nUSE {};\n",
                        quote(database, driver_kind),
                        quote(database, driver_kind)
                    ),
                    &mut bytes,
                )?;
            }
        }
        "postgres" => {
            write_text(
                &mut out,
                "SET client_encoding = 'UTF8';\nSET standard_conforming_strings = on;\n",
                &mut bytes,
            )?;
            if !schema.is_empty() {
                write_text(
                    &mut out,
                    &format!("SET search_path TO {};\n", quote(schema, driver_kind)),
                    &mut bytes,
                )?;
            }
        }
        _ => {
            write_text(&mut out, "PRAGMA foreign_keys=OFF;\n", &mut bytes)?;
        }
    }
    write_text(&mut out, "\n", &mut bytes)?;

    if options.include_drops && options.include_structure {
        let drop_groups: Vec<&[ExportObject]> = vec![
            views.as_ref(),
            matviews.as_ref(),
            triggers.as_ref(),
            events.as_ref(),
            procedures.as_ref(),
            functions.as_ref(),
        ];
        for group in drop_groups {
            for object in group {
                if cancel.load(Ordering::Relaxed) {
                    return Err("导出已取消".to_string());
                }
                match driver.dump_object_drop(&object.kind, &object.name).await {
                    Ok(statement) => {
                        write_text(&mut out, &ensure_semicolon(&statement), &mut bytes)?;
                    }
                    Err(error) => warnings.push(format!(
                        "{} {} 的删除语句未生成：{error}",
                        object.kind, object.name
                    )),
                }
            }
        }

        if driver_kind == "postgres" && !tables.is_empty() {
            for table in tables {
                match driver.table_structure(&table.name).await {
                    Ok(structure) => {
                        for foreign_key in &structure.foreign_keys {
                            write_text(
                                &mut out,
                                &format!(
                                    "ALTER TABLE IF EXISTS {}.{} DROP CONSTRAINT IF EXISTS {};\n",
                                    quote(schema, driver_kind),
                                    quote(&table.name, driver_kind),
                                    quote(&foreign_key.name, driver_kind)
                                ),
                                &mut bytes,
                            )?;
                        }
                    }
                    Err(error) => warnings.push(format!(
                        "读取 {} 的外键失败，删除表时可能受依赖影响：{error}",
                        table.name
                    )),
                }
            }
        }

        for table in tables {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match driver.dump_object_drop("table", &table.name).await {
                Ok(statement) => {
                    write_text(&mut out, &ensure_semicolon(&statement), &mut bytes)?;
                }
                Err(error) => warnings.push(format!("表 {} 的删除语句未生成：{error}", table.name)),
            }
        }
        for sequence in sequences {
            match driver.dump_object_drop("sequence", &sequence.name).await {
                Ok(statement) => {
                    write_text(&mut out, &ensure_semicolon(&statement), &mut bytes)?;
                }
                Err(error) => {
                    warnings.push(format!("序列 {} 的删除语句未生成：{error}", sequence.name))
                }
            }
        }
        write_text(&mut out, "\n", &mut bytes)?;
    }

    if options.include_structure {
        for sequence in sequences {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match write_object_ddl(
                driver,
                driver_kind,
                "sequence",
                &sequence.name,
                options.remove_definer,
                &mut out,
                &mut bytes,
            )
            .await
            {
                Ok(()) => {
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &sequence.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出序列 {}", sequence.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: "sequence".to_string(),
                    name: sequence.name.clone(),
                    message: error,
                }),
            }
        }

        for table in tables {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match driver.dump_table_ddl(&table.name).await {
                Ok(ddl) => {
                    write_text(&mut out, &ensure_semicolon(&ddl), &mut bytes)?;
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &table.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出表结构 {}", table.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: "table".to_string(),
                    name: table.name.clone(),
                    message: error,
                }),
            }
        }
    }

    if options.include_data && !tables.is_empty() {
        if options.use_transaction {
            write_text(
                &mut out,
                match driver_kind {
                    "mysql" => "START TRANSACTION;\n",
                    _ => "BEGIN;\n",
                },
                &mut bytes,
            )?;
        }
        for table in tables {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            emit_progress(
                app,
                operation_id,
                "data",
                &table.name,
                processed,
                object_total,
                rows,
                bytes,
                format!("正在导出 {} 的数据", table.name),
            );
            match export_table_data(
                app,
                driver,
                driver_kind,
                database,
                schema,
                &table.name,
                options,
                cancel,
                operation_id,
                processed,
                object_total,
                &mut rows,
                &mut bytes,
                &mut out,
            )
            .await
            {
                Ok(_) => {
                    if !options.include_structure {
                        successes += 1;
                    }
                }
                Err(error) if error == "导出已取消" => {
                    return Err(error);
                }
                Err(error) if error.starts_with(FATAL_PREFIX) => {
                    return Err(strip_fatal(&error));
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: "data".to_string(),
                    name: table.name.clone(),
                    message: error,
                }),
            }
            if !options.include_structure {
                processed = (processed + 1).min(object_total);
                emit_progress(
                    app,
                    operation_id,
                    "data",
                    &table.name,
                    processed,
                    object_total,
                    rows,
                    bytes,
                    format!("已导出 {} 的数据", table.name),
                );
            }
        }
        if options.use_transaction {
            write_text(&mut out, "COMMIT;\n", &mut bytes)?;
        }
        write_text(&mut out, "\n", &mut bytes)?;
    }

    if options.include_structure {
        for table in tables {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            if driver.dump_indexes_separately() {
                match driver.dump_index_statements(&table.name).await {
                    Ok(statements) => {
                        for statement in statements {
                            write_text(&mut out, &ensure_semicolon(&statement), &mut bytes)?;
                        }
                    }
                    Err(error) => failures.push(DatabaseExportFailure {
                        kind: "index".to_string(),
                        name: table.name.clone(),
                        message: error,
                    }),
                }
            }
            if driver.dump_foreign_keys_separately() {
                match driver.table_structure(&table.name).await {
                    Ok(structure) => {
                        for foreign_key in &structure.foreign_keys {
                            if let Some(statement) = add_foreign_key_statement(
                                driver_kind,
                                schema,
                                &table.name,
                                foreign_key,
                            ) {
                                write_text(&mut out, &ensure_semicolon(&statement), &mut bytes)?;
                            }
                        }
                    }
                    Err(error) => failures.push(DatabaseExportFailure {
                        kind: "foreignKey".to_string(),
                        name: table.name.clone(),
                        message: error,
                    }),
                }
            }
        }
        write_text(&mut out, "\n", &mut bytes)?;

        for object in views {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match write_object_ddl(
                driver,
                driver_kind,
                "view",
                &object.name,
                options.remove_definer,
                &mut out,
                &mut bytes,
            )
            .await
            {
                Ok(()) => {
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &object.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出视图 {}", object.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: "view".to_string(),
                    name: object.name.clone(),
                    message: error,
                }),
            }
        }
        for object in matviews {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match write_object_ddl(
                driver,
                driver_kind,
                "matview",
                &object.name,
                options.remove_definer,
                &mut out,
                &mut bytes,
            )
            .await
            {
                Ok(()) => {
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &object.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出物化视图 {}", object.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: "matview".to_string(),
                    name: object.name.clone(),
                    message: error,
                }),
            }
        }
        for object in procedures.iter().chain(functions.iter()) {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match write_object_ddl(
                driver,
                driver_kind,
                &object.kind,
                &object.name,
                options.remove_definer,
                &mut out,
                &mut bytes,
            )
            .await
            {
                Ok(()) => {
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &object.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出 {} {}", object.kind, object.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: object.kind.clone(),
                    name: object.name.clone(),
                    message: error,
                }),
            }
        }
        for object in triggers.iter().chain(events.iter()) {
            if cancel.load(Ordering::Relaxed) {
                return Err("导出已取消".to_string());
            }
            match write_object_ddl(
                driver,
                driver_kind,
                &object.kind,
                &object.name,
                options.remove_definer,
                &mut out,
                &mut bytes,
            )
            .await
            {
                Ok(()) => {
                    processed += 1;
                    successes += 1;
                    emit_progress(
                        app,
                        operation_id,
                        "structure",
                        &object.name,
                        processed,
                        object_total,
                        rows,
                        bytes,
                        format!("已导出 {} {}", object.kind, object.name),
                    );
                }
                Err(error) => failures.push(DatabaseExportFailure {
                    kind: object.kind.clone(),
                    name: object.name.clone(),
                    message: error,
                }),
            }
        }
    }

    match driver_kind {
        "mysql" => write_text(&mut out, "SET FOREIGN_KEY_CHECKS=1;\n", &mut bytes)?,
        "sqlite" => write_text(&mut out, "PRAGMA foreign_keys=ON;\n", &mut bytes)?,
        _ => {}
    }
    out.flush()
        .map_err(|error| format!("刷新导出文件失败: {error}"))?;
    drop(out);

    if target.exists() {
        std::fs::remove_file(target).map_err(|error| format!("替换已有导出文件失败: {error}"))?;
    }
    std::fs::rename(temp_path, target).map_err(|error| format!("保存导出文件失败: {error}"))?;

    emit_progress(
        app,
        operation_id,
        "done",
        &scope,
        object_total,
        object_total,
        rows,
        bytes,
        format!("导出完成：{}", target.display()),
    );

    if successes == 0 && !failures.is_empty() {
        let _ = std::fs::remove_file(target);
        return Err("所有对象导出失败，未保留文件".to_string());
    }

    Ok(DatabaseExportSummary {
        path: target.to_string_lossy().to_string(),
        object_count: successes,
        table_count: tables.len(),
        rows,
        bytes,
        failures,
        warnings,
    })
}

fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DatabaseDriver, SqliteDriver};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[tokio::test]
    async fn exports_sqlite_database_to_executable_sql() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("source.db");
        let source = SqliteDriver::connect(source_path.to_str().expect("path"))
            .await
            .expect("source connect");
        source
            .execute(
                "CREATE TABLE items (\
                   id INTEGER PRIMARY KEY, \
                   name TEXT, \
                   payload BLOB\
                 )",
                1000,
            )
            .await
            .expect("create table");
        source
            .execute(
                "INSERT INTO items (id, name, payload) \
                 VALUES (1, 'O''Brien', X'00FF10')",
                1000,
            )
            .await
            .expect("insert");
        source
            .execute("CREATE INDEX idx_items_name ON items(name)", 1000)
            .await
            .expect("index");
        source
            .execute("CREATE VIEW item_names AS SELECT name FROM items", 1000)
            .await
            .expect("view");
        source
            .execute(
                "CREATE TRIGGER trg_items AFTER INSERT ON items \
                 BEGIN UPDATE items SET name = name WHERE id = new.id; END",
                1000,
            )
            .await
            .expect("trigger");

        let tables = vec![ExportObject {
            kind: "table".to_string(),
            name: "items".to_string(),
        }];
        let views = vec![ExportObject {
            kind: "view".to_string(),
            name: "item_names".to_string(),
        }];
        let triggers = vec![ExportObject {
            kind: "trigger".to_string(),
            name: "trg_items".to_string(),
        }];
        let target = dir.path().join("dump.sql");
        let temp = temp_path_for(&target);
        let cancel = AtomicBool::new(false);
        let options = DatabaseExportOptions::default();

        let summary = export_database_inner(
            None,
            &source,
            "sqlite",
            "test",
            "",
            "",
            &tables,
            &views,
            &[],
            &[],
            &[],
            &[],
            &triggers,
            &[],
            &options,
            &cancel,
            None,
            &target,
            &temp,
            3,
        )
        .await
        .expect("export");

        assert_eq!(summary.rows, 1);
        assert!(summary.failures.is_empty());
        let sql = std::fs::read_to_string(&target).expect("read dump");
        assert!(sql.contains("X'00FF10'"));
        assert!(sql.contains("CREATE VIEW"));
        assert!(sql.contains("CREATE TRIGGER"));
        assert!(!sql.contains("<BLOB"));

        let target_db = dir.path().join("restored.db");
        let connect_options = SqliteConnectOptions::new()
            .filename(&target_db)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .expect("target connect");
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.clone()))
            .execute(&pool)
            .await
            .expect("restore dump");
        let payload: String = sqlx::query_scalar("SELECT hex(payload) FROM items")
            .fetch_one(&pool)
            .await
            .expect("read payload");
        assert_eq!(payload, "00FF10");
        let name: String = sqlx::query_scalar("SELECT name FROM item_names")
            .fetch_one(&pool)
            .await
            .expect("read view");
        assert_eq!(name, "O'Brien");
        let objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE name IN ('idx_items_name', 'trg_items')",
        )
        .fetch_one(&pool)
        .await
        .expect("count objects");
        assert_eq!(objects, 2);
    }
}
