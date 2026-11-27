use super::{AppState, driver_for, record_query_history};
use crate::db::query::first_keyword;
use crate::db::sql_script::split_sql_script;
use crate::models::ExplainResult;
use crate::storage;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn explain_query(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    sql: String,
    history_sql: Option<String>,
) -> Result<ExplainResult, String> {
    let statements = split_sql_script(&sql)?;
    if statements.len() != 1 {
        return Err("一次只能解释一条语句".to_string());
    }
    let statement = statements[0].sql.trim().to_string();
    let history_sql = history_sql.as_deref().unwrap_or(&sql).to_string();
    let keyword = first_keyword(&statement).to_ascii_lowercase();
    if keyword == "explain" {
        return Err("请选中原始查询，不要重复 EXPLAIN".to_string());
    }
    if !matches!(keyword.as_str(), "select" | "with" | "table" | "values") {
        return Err("只允许解释 SELECT / WITH / TABLE / VALUES 查询".to_string());
    }
    let config = storage::load_connections(&app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持执行计划".to_string());
    }
    let driver = driver_for(&app, &state, &connection_id).await?;

    let (explain_sql, format) = match config.driver.as_str() {
        "mysql" => (format!("EXPLAIN FORMAT=JSON {statement}"), "json"),
        "postgres" => (
            format!("EXPLAIN (FORMAT JSON, COSTS TRUE, VERBOSE FALSE, SETTINGS FALSE) {statement}"),
            "json",
        ),
        _ => (format!("EXPLAIN QUERY PLAN {statement}"), "table"),
    };

    if config.driver == "mysql" {
        match driver.execute(&explain_sql, 100).await {
            Ok(result) => {
                record_query_history(&app, &connection_id, &history_sql, &Ok(result.clone()));
                Ok(ExplainResult {
                    driver: config.driver,
                    statement,
                    explain_sql,
                    format: format.to_string(),
                    result,
                })
            }
            Err(_) => {
                let fallback = format!("EXPLAIN {statement}");
                let outcome = driver.execute(&fallback, 200).await;
                record_query_history(&app, &connection_id, &history_sql, &outcome);
                outcome.map(|result| ExplainResult {
                    driver: config.driver,
                    statement,
                    explain_sql: fallback,
                    format: "table".to_string(),
                    result,
                })
            }
        }
    } else {
        let outcome = driver.execute(&explain_sql, 200).await;
        record_query_history(&app, &connection_id, &history_sql, &outcome);
        outcome.map(|result| ExplainResult {
            driver: config.driver,
            statement,
            explain_sql,
            format: format.to_string(),
            result,
        })
    }
}
