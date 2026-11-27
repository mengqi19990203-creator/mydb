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

fn cross_driver_type(source_type: &str, target: &str, auto_increment: bool) -> String {
    let source = source_type.to_ascii_uppercase();
    if source.contains("INT") || source.contains("SERIAL") {
        return match target {
            "postgres" if auto_increment => "BIGSERIAL".to_string(),
            "postgres" => "BIGINT".to_string(),
            "mysql" => "BIGINT".to_string(),
            _ => "INTEGER".to_string(),
        };
    }
    if source.contains("BOOL") {
        return match target {
            "postgres" => "BOOLEAN".to_string(),
            "mysql" => "TINYINT(1)".to_string(),
            _ => "INTEGER".to_string(),
        };
    }
    if source.contains("REAL")
        || source.contains("FLOAT")
        || source.contains("DOUBLE")
        || source.contains("DECIMAL")
        || source.contains("NUMERIC")
        || source.contains("MONEY")
    {
        return match target {
            "postgres" => "DOUBLE PRECISION".to_string(),
            "mysql" => "DOUBLE".to_string(),
            _ => "REAL".to_string(),
        };
    }
    if source.contains("DATE") || source.contains("TIME") {
        return match target {
            "postgres" => "TIMESTAMP".to_string(),
            "mysql" => "DATETIME".to_string(),
            _ => "TEXT".to_string(),
        };
    }
    if source.contains("JSON") {
        return match target {
            "postgres" => "JSONB".to_string(),
            "mysql" => "JSON".to_string(),
            _ => "TEXT".to_string(),
        };
    }
    if source.contains("BLOB")
        || source.contains("BINARY")
        || source.contains("BYTEA")
        || source.contains("IMAGE")
    {
        return match target {
            "postgres" => "BYTEA".to_string(),
            "mysql" => "LONGBLOB".to_string(),
            _ => "BLOB".to_string(),
        };
    }
    match target {
        "postgres" => "TEXT".to_string(),
        "mysql" => "TEXT".to_string(),
        _ => "TEXT".to_string(),
    }
}

fn cross_driver_create_table(
    columns: &[crate::models::ColumnInfo],
    target_table: &str,
    target: &str,
) -> Result<String, String> {
    if columns.is_empty() {
        return Err("源表没有字段，无法创建目标表".to_string());
    }
    let primary_keys = columns
        .iter()
        .filter(|column| column.primary_key)
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let single_auto = primary_keys.len() == 1
        && columns
            .iter()
            .any(|column| column.primary_key && column.auto_increment);
    let inline_primary = single_auto && matches!(target, "sqlite" | "mysql");
    let mut definitions = Vec::new();
    for column in columns {
        let mut definition = format!(
            "{} {}",
            quote(&column.name, target),
            cross_driver_type(&column.data_type, target, column.auto_increment)
        );
        if column.primary_key && column.auto_increment && inline_primary {
            definition.push_str(" PRIMARY KEY");
            if target == "sqlite" {
                definition.push_str(" AUTOINCREMENT");
            } else {
                definition.push_str(" AUTO_INCREMENT");
            }
        } else if !column.nullable {
            definition.push_str(" NOT NULL");
        }
        definitions.push(definition);
    }
    if !inline_primary && !primary_keys.is_empty() {
        definitions.push(format!(
            "PRIMARY KEY ({})",
            primary_keys
                .iter()
                .map(|name| quote(name, target))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(format!(
        "CREATE TABLE {} ({})",
        quote(target_table, target),
        definitions.join(", ")
    ))
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
    let source_kind = source_config.driver.clone();
    let target_kind = target_config.driver.clone();
    if !["sqlite", "mysql", "postgres"].contains(&source_kind.as_str()) {
        return Err("源连接不是支持表数据传输的 SQL 数据库".to_string());
    }
    if !["sqlite", "mysql", "postgres"].contains(&target_kind.as_str()) {
        return Err("目标连接不是支持表数据传输的 SQL 数据库".to_string());
    }
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
                if source_kind == target_kind {
                    let ddl = source.dump_table_ddl(&table.source_table).await?;
                    let renamed =
                        rename_ddl_object(&ddl, &quote(&table.target_table, &target_kind))?;
                    target.execute(&renamed, 1).await?;
                } else {
                    let columns = source.list_columns(&table.source_table).await?;
                    let ddl = cross_driver_create_table(
                        &columns,
                        &table.target_table,
                        &target_kind,
                    )?;
                    target.execute(&ddl, 1).await?;
                }
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
                        &format!("DELETE FROM {}", quote(&table.target_table, &target_kind)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ColumnInfo;

    fn column(name: &str, data_type: &str, primary_key: bool, auto: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            nullable: !primary_key,
            primary_key,
            auto_increment: auto,
            default_value: None,
            comment: None,
        }
    }

    #[test]
    fn creates_cross_driver_tables_from_source_columns() {
        let columns = vec![
            column("id", "INTEGER", true, true),
            column("name", "VARCHAR(80)", false, false),
            column("payload", "BLOB", false, false),
        ];
        let postgres = cross_driver_create_table(&columns, "events", "postgres").unwrap();
        assert!(postgres.contains("\"id\" BIGSERIAL"));
        assert!(postgres.contains("\"name\" TEXT"));
        assert!(postgres.contains("\"payload\" BYTEA"));
        assert!(postgres.contains("PRIMARY KEY (\"id\")"));

        let mysql = cross_driver_create_table(&columns, "events", "mysql").unwrap();
        assert!(mysql.contains("`id` BIGINT PRIMARY KEY AUTO_INCREMENT"));

        let sqlite = cross_driver_create_table(&columns, "events", "sqlite").unwrap();
        assert!(sqlite.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
    }
}
