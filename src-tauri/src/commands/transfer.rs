use super::{AppState, driver_for, ensure_writable, isolated_driver_for};
use crate::db::driver::rename_ddl_object;
use crate::models::{
    ColumnValue, TransferFailure, TransferOptions, TransferProgress, TransferResult, TransferTable,
};
use crate::storage;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

fn quote(name: &str, driver: &str) -> String {
    if driver == "mysql" {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

async fn resolve_driver(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
    database: Option<&str>,
    schema: Option<&str>,
) -> Result<Arc<dyn crate::db::DatabaseDriver>, String> {
    let has_database = database.is_some_and(|value| !value.trim().is_empty());
    let has_schema = schema.is_some_and(|value| !value.trim().is_empty());
    if has_database || has_schema {
        isolated_driver_for(app, state, connection_id, database, schema).await
    } else {
        driver_for(app, state, connection_id).await
    }
}

fn mapped_values(
    source_columns: &[String],
    target_columns: &[String],
    row: &[Value],
) -> Vec<ColumnValue> {
    let mut target_index: HashMap<&str, usize> = HashMap::new();
    for (index, column) in target_columns.iter().enumerate() {
        target_index.insert(column.as_str(), index);
    }
    source_columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            target_index
                .contains_key(column.as_str())
                .then(|| ColumnValue {
                    column: column.clone(),
                    value: row.get(index).cloned().unwrap_or(Value::Null),
                })
        })
        .collect()
}

#[tauri::command]
pub async fn transfer_table_data(
    app: AppHandle,
    state: State<'_, AppState>,
    source_connection_id: String,
    target_connection_id: String,
    source_database: Option<String>,
    source_schema: Option<String>,
    target_database: Option<String>,
    target_schema: Option<String>,
    tables: Vec<TransferTable>,
    options: TransferOptions,
    operation_id: Option<String>,
) -> Result<TransferResult, String> {
    if tables.is_empty() {
        return Err("请至少选择一张表".to_string());
    }
    ensure_writable(&app, &target_connection_id)?;
    let connections = storage::load_connections(&app)?;
    let source_config = connections
        .iter()
        .find(|item| item.id == source_connection_id)
        .ok_or_else(|| "源连接不存在".to_string())?;
    let target_config = connections
        .iter()
        .find(|item| item.id == target_connection_id)
        .ok_or_else(|| "目标连接不存在".to_string())?;
    if source_config.driver != target_config.driver {
        return Err("数据传输第一版只支持同类型数据库".to_string());
    }
    if source_config.driver == "redis" {
        return Err("Redis 不支持表数据传输".to_string());
    }
    let driver_kind = source_config.driver.clone();
    let mode = options.mode.to_ascii_lowercase();
    if !matches!(mode.as_str(), "append" | "upsert" | "truncate") {
        return Err("未知传输模式".to_string());
    }

    let source = resolve_driver(
        &app,
        &state,
        &source_connection_id,
        source_database.as_deref(),
        source_schema.as_deref(),
    )
    .await?;
    let target = resolve_driver(
        &app,
        &state,
        &target_connection_id,
        target_database.as_deref(),
        target_schema.as_deref(),
    )
    .await?;
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(id) = &operation_id {
        state
            .export_cancels
            .lock()
            .await
            .insert(id.clone(), cancel.clone());
    }

    let total_tables = tables.len();
    let mut total_rows = 0u64;
    let mut failures = Vec::new();
    let mut completed_tables = 0usize;
    let mut cancelled = false;

    for (index, table) in tables.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let _ = app.emit(
            "data-transfer:progress",
            TransferProgress {
                operation_id: operation_id.clone(),
                table: table.target_table.clone(),
                table_index: index + 1,
                table_total: total_tables,
                rows: total_rows,
                message: format!("正在传输 {}", table.source_table),
            },
        );
        let outcome: Result<u64, String> = async {
            let target_tables = target.list_tables().await?;
            let target_exists = target_tables
                .iter()
                .any(|item| item.name == table.target_table && item.kind == "table");
            if !target_exists {
                if !options.create_missing {
                    return Err(format!("目标表 {} 不存在", table.target_table));
                }
                let ddl = source.dump_table_ddl(&table.source_table).await?;
                let renamed = rename_ddl_object(&ddl, &quote(&table.target_table, &driver_kind))?;
                target.execute(&renamed, 1).await?;
            }

            let target_columns = target
                .list_columns(&table.target_table)
                .await?
                .into_iter()
                .map(|column| column.name)
                .collect::<Vec<_>>();
            let primary_keys = target
                .list_columns(&table.target_table)
                .await?
                .into_iter()
                .filter(|column| column.primary_key)
                .map(|column| column.name)
                .collect::<Vec<_>>();
            if mode == "upsert" && primary_keys.is_empty() {
                return Err("upsert 模式要求目标表有主键".to_string());
            }
            if mode == "truncate" {
                target
                    .execute(
                        &format!("DELETE FROM {}", quote(&table.target_table, &driver_kind)),
                        1,
                    )
                    .await?;
            }

            let mut source_columns: Option<Vec<String>> = None;
            let source_meta = source.list_columns(&table.source_table).await?;
            let order_by = source_meta
                .iter()
                .find(|column| column.primary_key)
                .map(|column| column.name.clone());
            let mut offset = 0u64;
            let mut table_rows = 0u64;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("传输已取消".to_string());
                }
                let page = source
                    .read_rows(
                        &table.source_table,
                        500,
                        offset,
                        None,
                        order_by.as_deref(),
                        Some("asc"),
                    )
                    .await?;
                if page.rows.is_empty() {
                    break;
                }
                let columns = source_columns.get_or_insert_with(|| {
                    page.columns
                        .iter()
                        .map(|column| column.name.clone())
                        .collect()
                });
                for row in &page.rows {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("传输已取消".to_string());
                    }
                    let values = mapped_values(columns, &target_columns, row);
                    if values.is_empty() {
                        return Err("源表与目标表没有同名字段".to_string());
                    }
                    if mode == "upsert" {
                        let keys: Vec<ColumnValue> = values
                            .iter()
                            .filter(|value| primary_keys.iter().any(|key| key == &value.column))
                            .cloned()
                            .collect();
                        if keys.len() != primary_keys.len() {
                            return Err("源数据缺少目标主键字段".to_string());
                        }
                        let changes: Vec<ColumnValue> = values
                            .iter()
                            .filter(|value| !primary_keys.iter().any(|key| key == &value.column))
                            .cloned()
                            .collect();
                        let affected = target
                            .update_row(&table.target_table, &keys, &changes, false)
                            .await?;
                        if affected == 0 {
                            target.insert_row(&table.target_table, &values).await?;
                        }
                    } else {
                        target.insert_row(&table.target_table, &values).await?;
                    }
                    table_rows += 1;
                    total_rows += 1;
                }
                offset += page.rows.len() as u64;
                let _ = app.emit(
                    "data-transfer:progress",
                    TransferProgress {
                        operation_id: operation_id.clone(),
                        table: table.target_table.clone(),
                        table_index: index + 1,
                        table_total: total_tables,
                        rows: total_rows,
                        message: format!("正在传输 {}：{} 行", table.target_table, table_rows),
                    },
                );
            }
            Ok(table_rows)
        }
        .await;
        match outcome {
            Ok(_) => completed_tables += 1,
            Err(message) if message == "传输已取消" => {
                cancelled = true;
                break;
            }
            Err(message) => failures.push(TransferFailure {
                table: table.target_table.clone(),
                message,
            }),
        }
    }

    if let Some(id) = &operation_id {
        state.export_cancels.lock().await.remove(id);
    }
    Ok(TransferResult {
        tables: completed_tables,
        rows: total_rows,
        failures,
        cancelled,
    })
}

#[tauri::command]
pub async fn cancel_data_transfer(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<(), String> {
    if let Some(flag) = state.export_cancels.lock().await.get(&operation_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
