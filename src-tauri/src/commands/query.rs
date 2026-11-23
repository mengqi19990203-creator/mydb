use super::{AppState, connection_read_only, driver_for};
use crate::db::sql_guard;
use crate::history;
use crate::models::{QueryResult, TableInfo};
use crate::storage;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn list_tables(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<TableInfo>, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.list_tables().await
}

#[tauri::command]
pub async fn get_index_table(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    index_name: String,
) -> Result<String, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver
        .index_table(&index_name)
        .await?
        .ok_or_else(|| format!("未找到索引 {index_name} 对应的表"))
}

#[tauri::command]
pub async fn execute_query(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    sql: String,
    max_rows: Option<usize>,
) -> Result<QueryResult, String> {
    ensure_query_allowed(&app, &connection_id, &sql)?;

    let driver = driver_for(&app, &state, &connection_id).await?;
    let max_rows = max_rows.unwrap_or(1000).clamp(1, 10_000);

    let outcome = driver.execute(&sql, max_rows).await;
    record_query_history(&app, &connection_id, &sql, &outcome);

    outcome
}

pub(crate) fn ensure_query_allowed(
    app: &AppHandle,
    connection_id: &str,
    sql: &str,
) -> Result<(), String> {
    if connection_read_only(app, connection_id)? {
        let driver_kind = storage::load_connections(app)?
            .into_iter()
            .find(|item| item.id == connection_id)
            .map(|item| item.driver)
            .unwrap_or_default();
        sql_guard::ensure_read_only_sql(sql, &driver_kind)?;
    }
    Ok(())
}

pub(crate) fn record_query_history(
    app: &AppHandle,
    connection_id: &str,
    sql: &str,
    outcome: &Result<QueryResult, String>,
) {
    let connection_name = storage::load_connections(app)
        .ok()
        .and_then(|connections| {
            connections
                .into_iter()
                .find(|item| item.id == connection_id)
                .map(|item| item.name)
        })
        .unwrap_or_default();

    match outcome {
        Ok(result) => {
            let _ = history::record(
                app,
                connection_id,
                &connection_name,
                sql,
                result.elapsed_ms,
                true,
                None,
            );
        }
        Err(error) => {
            let _ = history::record(
                app,
                connection_id,
                &connection_name,
                sql,
                0,
                false,
                Some(error.clone()),
            );
        }
    }
}
