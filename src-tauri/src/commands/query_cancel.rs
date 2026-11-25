use super::{AppState, RunningQuery, driver_for};
use crate::db::{DatabaseDriver, DatabaseSession};
use crate::models::{CancelQueryResult, QueryResult};
use crate::storage;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, State};

/// First version uses a fixed timeout; it is not configurable yet.
pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) fn new_operation_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn driver_kind(app: &AppHandle, connection_id: &str) -> String {
    storage::load_connections(app)
        .ok()
        .and_then(|connections| {
            connections
                .into_iter()
                .find(|item| item.id == connection_id)
                .map(|item| item.driver)
        })
        .unwrap_or_default()
}

/// Runs one statement on a dedicated session with a fixed timeout and a
/// cancellation registry entry.
///
/// Cancellation is delivered by the `cancel_query` command through a second
/// pooled connection (`KILL QUERY` / `pg_cancel_backend`). SQLite cannot be
/// interrupted by SQLx, so the entry has no backend id and the frontend is
/// told it will end at the timeout.
pub(crate) async fn execute_with_cancel(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
    operation_id: &str,
    session: &mut Box<dyn DatabaseSession>,
    sql: &str,
    max_rows: usize,
    driver: &dyn DatabaseDriver,
) -> Result<QueryResult, String> {
    let kind = driver_kind(app, connection_id);
    let backend_id = match session.backend_id().await {
        Ok(id) => id,
        Err(_) => None,
    };
    let cancel_requested = Arc::new(AtomicBool::new(false));

    state.running_queries.lock().await.insert(
        operation_id.to_string(),
        RunningQuery {
            connection_id: connection_id.to_string(),
            driver_kind: kind.clone(),
            backend_id,
            cancel_requested: cancel_requested.clone(),
            started_at: std::time::Instant::now(),
        },
    );

    let outcome = tokio::time::timeout(QUERY_TIMEOUT, session.execute(sql, max_rows)).await;

    let result = match outcome {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => {
            if cancel_requested.load(Ordering::Relaxed) {
                Err("已取消".to_string())
            } else {
                Err(error)
            }
        }
        Err(_) => {
            cancel_requested.store(true, Ordering::Relaxed);
            let mut message = "查询超时（60 秒）".to_string();
            if let Some(backend_id) = backend_id {
                match driver.cancel_backend_query(backend_id).await {
                    Ok(true) => {}
                    Ok(false) => message.push_str("；数据库未接受取消，语句可能仍在运行"),
                    Err(error) => {
                        message.push_str("；");
                        message.push_str(&error);
                    }
                }
            } else if kind == "sqlite" {
                message.push_str("；SQLite 不支持中断");
            }
            Err(message)
        }
    };

    state.running_queries.lock().await.remove(operation_id);
    result
}

/// Issues the driver-specific cancel request for a running statement.
#[tauri::command]
pub async fn cancel_query(
    app: AppHandle,
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<CancelQueryResult, String> {
    let entry = state
        .running_queries
        .lock()
        .await
        .get(&operation_id)
        .cloned();

    let Some(entry) = entry else {
        return Ok(CancelQueryResult {
            cancelled: false,
            message: "查询已结束".to_string(),
        });
    };

    entry.cancel_requested.store(true, Ordering::Relaxed);

    if entry.driver_kind == "sqlite" {
        return Ok(CancelQueryResult {
            cancelled: false,
            message: "SQLite 不支持中断，将在超时后结束".to_string(),
        });
    }

    let Some(backend_id) = entry.backend_id else {
        return Ok(CancelQueryResult {
            cancelled: false,
            message: "当前连接无法定位正在执行的语句".to_string(),
        });
    };

    let driver = driver_for(&app, &state, &entry.connection_id).await?;
    match driver.cancel_backend_query(backend_id).await {
        Ok(true) => Ok(CancelQueryResult {
            cancelled: true,
            message: format!(
                "已发送取消请求（已运行 {} 秒）",
                entry.started_at.elapsed().as_secs()
            ),
        }),
        Ok(false) => Ok(CancelQueryResult {
            cancelled: false,
            message: "数据库未接受取消，请稍后重试".to_string(),
        }),
        Err(error) => Ok(CancelQueryResult {
            cancelled: false,
            message: error,
        }),
    }
}
