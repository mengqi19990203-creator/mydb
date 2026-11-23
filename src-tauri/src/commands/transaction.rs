use super::{
    AppState, TransactionEntry, driver_for, ensure_query_allowed, record_query_history,
};
use crate::models::QueryResult;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

async fn entry_for(
    state: &State<'_, AppState>,
    session_id: &str,
    connection_id: &str,
) -> Result<TransactionEntry, String> {
    let entry = state
        .transactions
        .lock()
        .await
        .get(session_id)
        .cloned()
        .ok_or_else(|| "事务不存在或已结束".to_string())?;
    if entry.connection_id != connection_id {
        return Err("事务与当前连接不匹配".to_string());
    }
    Ok(entry)
}

#[tauri::command]
pub async fn begin_transaction(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    session_id: String,
) -> Result<(), String> {
    // Hold the map lock across creation so two quick BEGIN clicks cannot
    // open two competing sessions for the same tab.
    let mut transactions = state.transactions.lock().await;
    if transactions.contains_key(&session_id) {
        return Err("该查询标签已有未结束的事务".to_string());
    }
    let driver = driver_for(&app, &state, &connection_id).await?;
    let mut session = driver.open_session().await?;
    session.begin().await?;
    transactions.insert(
        session_id,
        TransactionEntry {
            connection_id,
            session: Arc::new(Mutex::new(session)),
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn execute_transaction_query(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    session_id: String,
    sql: String,
    max_rows: Option<usize>,
) -> Result<QueryResult, String> {
    let entry = entry_for(&state, &session_id, &connection_id).await?;
    ensure_query_allowed(&app, &connection_id, &sql)?;
    let max_rows = max_rows.unwrap_or(1000).clamp(1, 10_000);

    let outcome = {
        let mut session = entry.session.lock().await;
        session.execute(&sql, max_rows).await
    };
    record_query_history(&app, &connection_id, &sql, &outcome);
    outcome
}

#[tauri::command]
pub async fn commit_transaction(
    state: State<'_, AppState>,
    connection_id: String,
    session_id: String,
) -> Result<(), String> {
    let entry = {
        let mut transactions = state.transactions.lock().await;
        let entry = transactions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| "事务不存在或已结束".to_string())?;
        if entry.connection_id != connection_id {
            return Err("事务与当前连接不匹配".to_string());
        }
        transactions.remove(&session_id);
        entry
    };

    let mut session = entry.session.lock().await;
    let outcome = session.commit().await;
    if outcome.is_err() {
        let _ = session.rollback().await;
    }
    outcome
}

#[tauri::command]
pub async fn rollback_transaction(
    state: State<'_, AppState>,
    connection_id: String,
    session_id: String,
) -> Result<(), String> {
    let entry = {
        let mut transactions = state.transactions.lock().await;
        let entry = transactions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| "事务不存在或已结束".to_string())?;
        if entry.connection_id != connection_id {
            return Err("事务与当前连接不匹配".to_string());
        }
        transactions.remove(&session_id);
        entry
    };

    let mut session = entry.session.lock().await;
    session.rollback().await
}

#[tauri::command]
pub async fn close_transaction_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let entry = state.transactions.lock().await.remove(&session_id);
    if let Some(entry) = entry {
        let mut session = entry.session.lock().await;
        let _ = session.rollback().await;
    }
    Ok(())
}

/// Rolls back every interactive transaction opened on `connection_id`.
pub(crate) async fn close_connection_transactions(state: &AppState, connection_id: &str) {
    let entries: Vec<TransactionEntry> = {
        let mut transactions = state.transactions.lock().await;
        let ids: Vec<String> = transactions
            .iter()
            .filter(|(_, entry)| entry.connection_id == connection_id)
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| transactions.remove(&id))
            .collect()
    };
    for entry in entries {
        let mut session = entry.session.lock().await;
        let _ = session.rollback().await;
    }
}