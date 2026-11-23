use super::{AppState, driver_for, ensure_writable};
use crate::models::{ColumnInfo, ColumnValue, ErDiagram, TableRows, TableStructure};
use crate::storage;
use serde_json::Value;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn list_columns(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    ensure_table(&table)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.list_columns(&table).await
}

#[tauri::command]
pub async fn get_table_structure(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
) -> Result<TableStructure, String> {
    ensure_table(&table)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.table_structure(&table).await
}

#[tauri::command]
pub async fn get_er_diagram(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ErDiagram, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.er_diagram().await
}

#[tauri::command]
pub async fn read_table_rows(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    limit: Option<u64>,
    offset: Option<u64>,
    filter: Option<String>,
    order_by: Option<String>,
    order_dir: Option<String>,
) -> Result<TableRows, String> {
    ensure_table(&table)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let limit = limit.unwrap_or(100).clamp(1, 1000);
    let offset = offset.unwrap_or(0);
    driver
        .read_rows(
            &table,
            limit,
            offset,
            filter.as_deref(),
            order_by.as_deref(),
            order_dir.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn update_table_row(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    keys: Vec<ColumnValue>,
    changes: Vec<ColumnValue>,
) -> Result<u64, String> {
    ensure_table(&table)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.update_row(&table, &keys, &changes).await
}

#[tauri::command]
pub async fn insert_table_row(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    values: Vec<ColumnValue>,
) -> Result<u64, String> {
    ensure_table(&table)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.insert_row(&table, &values).await
}

#[tauri::command]
pub async fn delete_table_row(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    keys: Vec<ColumnValue>,
) -> Result<u64, String> {
    ensure_table(&table)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.delete_row(&table, &keys).await
}

#[tauri::command]
pub async fn get_object_ddl(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    kind: String,
    name: String,
) -> Result<String, String> {
    ensure_table(&name)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.object_ddl(&kind, &name).await
}

#[tauri::command]
pub async fn rename_object(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    kind: String,
    name: String,
    new_name: String,
) -> Result<(), String> {
    ensure_table(&name)?;
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err("新名称不能为空".to_string());
    }
    if new_name == name {
        return Ok(());
    }
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.rename_object(&kind, &name, new_name).await
}

#[tauri::command]
pub async fn drop_object(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    kind: String,
    name: String,
) -> Result<(), String> {
    ensure_table(&name)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.drop_object(&kind, &name).await
}

/// Exports a table/view to a file. `limit = None` exports every row.
/// Rows are streamed from the driver so large tables are not buffered.
/// Returns the number of exported rows (0 for DDL exports).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn export_object(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    kind: String,
    name: String,
    format: String,
    path: String,
    limit: Option<u64>,
    with_header: Option<bool>,
    operation_id: Option<String>,
) -> Result<u64, String> {
    ensure_table(&name)?;
    if path.trim().is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    let driver = driver_for(&app, &state, &connection_id).await?;
    let format = format.to_ascii_lowercase();

    if format == "ddl" {
        let ddl = driver.object_ddl(&kind, &name).await?;
        std::fs::write(&path, ddl).map_err(|error| format!("写入文件失败: {error}"))?;
        return Ok(0);
    }

    let database_kind = connection_driver(&app, &connection_id)?;
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(id) = &operation_id {
        state
            .export_cancels
            .lock()
            .await
            .insert(id.clone(), cancel.clone());
    }

    let result = export_stream(
        driver.as_ref(),
        &database_kind,
        &kind,
        &name,
        &format,
        &path,
        limit,
        with_header.unwrap_or(true),
        &cancel,
    )
    .await;

    if let Some(id) = &operation_id {
        state.export_cancels.lock().await.remove(id);
    }
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

#[tauri::command]
pub async fn cancel_export(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<(), String> {
    if let Some(flag) = state.export_cancels.lock().await.get(&operation_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn export_stream(
    driver: &dyn crate::db::DatabaseDriver,
    database_kind: &str,
    kind: &str,
    name: &str,
    format: &str,
    path: &str,
    limit: Option<u64>,
    with_header: bool,
    cancel: &AtomicBool,
) -> Result<u64, String> {
    let columns = driver.list_columns(name).await?;
    let ddl = if format == "sql" && kind == "table" {
        driver.object_ddl("table", name).await.ok()
    } else {
        None
    };

    let file = std::fs::File::create(path).map_err(|error| format!("创建文件失败: {error}"))?;
    let mut out = std::io::BufWriter::new(file);
    let mysql = database_kind == "mysql";
    let cancelled = || cancel.load(Ordering::Relaxed);

    match format {
        "csv" => {
            let mut writer = csv::WriterBuilder::new().from_writer(out);
            if with_header {
                writer
                    .write_record(columns.iter().map(|column| column.name.as_str()))
                    .map_err(|error| format!("写入 CSV 表头失败: {error}"))?;
            }
            let count = driver
                .export_rows(name, limit, &mut |row| {
                    if cancelled() {
                        return Err("导出已取消".to_string());
                    }
                    writer
                        .write_record(row.iter().map(csv_cell))
                        .map_err(|error| format!("写入 CSV 行失败: {error}"))?;
                    Ok(())
                })
                .await?;
            writer
                .flush()
                .map_err(|error| format!("写入 CSV 失败: {error}"))?;
            Ok(count)
        }
        "json" => {
            out.write_all(b"{\n  \"table\": ")
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            serde_json::to_writer(&mut out, name)
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            out.write_all(b",\n  \"columns\": ")
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            serde_json::to_writer(&mut out, &columns)
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            out.write_all(b",\n  \"rows\": [")
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            let mut first = true;
            let count = driver
                .export_rows(name, limit, &mut |row| {
                    if cancelled() {
                        return Err("导出已取消".to_string());
                    }
                    let separator = if first {
                        first = false;
                        "\n    "
                    } else {
                        ",\n    "
                    };
                    out.write_all(separator.as_bytes())
                        .map_err(|error| format!("写入 JSON 失败: {error}"))?;
                    serde_json::to_writer(&mut out, &row)
                        .map_err(|error| format!("写入 JSON 失败: {error}"))?;
                    Ok(())
                })
                .await?;
            out.write_all(b"\n  ]\n}\n")
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            out.flush()
                .map_err(|error| format!("写入 JSON 失败: {error}"))?;
            Ok(count)
        }
        "sql" => {
            let table_ident = export_identifier(name, mysql);
            let column_list = columns
                .iter()
                .map(|column| export_identifier(&column.name, mysql))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "-- mydb export: {name}")
                .map_err(|error| format!("写入 SQL 失败: {error}"))?;
            if kind != "table" {
                writeln!(
                    out,
                    "-- 视图数据导出：INSERT 默认指向视图名，执行前请改为目标表。"
                )
                .map_err(|error| format!("写入 SQL 失败: {error}"))?;
            }
            if let Some(ddl) = &ddl {
                let ddl = ddl.trim().trim_end_matches(';');
                writeln!(out, "{ddl};")
                    .map_err(|error| format!("写入 SQL 失败: {error}"))?;
                writeln!(out).map_err(|error| format!("写入 SQL 失败: {error}"))?;
            }
            let count = driver
                .export_rows(name, limit, &mut |row| {
                    if cancelled() {
                        return Err("导出已取消".to_string());
                    }
                    let values = row
                        .iter()
                        .map(|value| sql_literal(value, mysql))
                        .collect::<Vec<_>>()
                        .join(", ");
                    writeln!(
                        out,
                        "INSERT INTO {table_ident} ({column_list}) VALUES ({values});"
                    )
                    .map_err(|error| format!("写入 SQL 失败: {error}"))?;
                    Ok(())
                })
                .await?;
            out.flush()
                .map_err(|error| format!("写入 SQL 失败: {error}"))?;
            Ok(count)
        }
        _ => Err(format!("不支持的导出格式: {format}")),
    }
}

#[tauri::command]
pub async fn duplicate_object(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    kind: String,
    source: String,
    target: String,
) -> Result<(), String> {
    ensure_table(&source)?;
    let target = target.trim();
    if target.is_empty() {
        return Err("新名称不能为空".to_string());
    }
    if target == source {
        return Err("新名称不能与原名称相同".to_string());
    }
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    match kind.as_str() {
        "table" => driver.duplicate_table(&source, target).await,
        "view" => driver.duplicate_view(&source, target).await,
        _ => Err("当前只支持复制表或视图".to_string()),
    }
}

#[tauri::command]
pub async fn snapshot_view_to_table(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    source: String,
    target: String,
    with_data: Option<bool>,
) -> Result<(), String> {
    ensure_table(&source)?;
    let target = target.trim();
    if target.is_empty() {
        return Err("新表名不能为空".to_string());
    }
    if target == source {
        return Err("新表名不能与视图名相同".to_string());
    }
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver
        .snapshot_view(&source, target, with_data.unwrap_or(true))
        .await
}

fn table_maintenance_sql(driver: &str, table: &str, action: &str) -> Result<String, String> {
    if table.trim().is_empty() {
        return Err("表名不能为空".to_string());
    }
    let identifier = export_identifier(table, driver == "mysql");
    match action {
        "empty" => Ok(format!("DELETE FROM {identifier}")),
        // SQLite has no TRUNCATE; DELETE FROM is the equivalent (the command
        // also clears the sqlite_sequence entry).
        "truncate" if driver == "sqlite" => Ok(format!("DELETE FROM {identifier}")),
        // PostgreSQL needs RESTART IDENTITY to reset sequences like Navicat.
        "truncate" if driver == "postgres" => {
            Ok(format!("TRUNCATE TABLE {identifier} RESTART IDENTITY"))
        }
        "truncate" => Ok(format!("TRUNCATE TABLE {identifier}")),
        _ => Err("不支持的表维护操作".to_string()),
    }
}

#[tauri::command]
pub async fn table_maintenance(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    action: String,
) -> Result<u64, String> {
    ensure_table(&table)?;
    ensure_writable(&app, &connection_id)?;
    let driver_kind = connection_driver(&app, &connection_id)?;
    let sql = table_maintenance_sql(&driver_kind, &table, &action)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let result = driver.execute(&sql, 1).await?;

    if driver_kind == "sqlite" && action == "truncate" {
        // Reset AUTOINCREMENT counters when sqlite_sequence exists; ignore
        // databases that never used AUTOINCREMENT.
        let literal = table.replace('\'', "''");
        let _ = driver
            .execute(
                &format!("DELETE FROM sqlite_sequence WHERE name = '{literal}'"),
                1,
            )
            .await;
    }
    Ok(result.rows_affected)
}

fn connection_driver(app: &AppHandle, connection_id: &str) -> Result<String, String> {
    storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .map(|item| item.driver)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))
}

#[tauri::command]
pub fn write_text_file(path: String, contents: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    std::fs::write(&path, contents).map_err(|error| format!("写入文件失败: {error}"))
}

#[tauri::command]
pub async fn import_csv(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    path: String,
    has_header: Option<bool>,
) -> Result<u64, String> {
    ensure_table(&table)?;
    if path.trim().is_empty() {
        return Err("CSV 文件路径不能为空".to_string());
    }

    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(has_header.unwrap_or(true))
        .flexible(true)
        .from_path(&path)
        .map_err(|error| format!("打开 CSV 失败: {error}"))?;

    let headers: Vec<String> = if has_header.unwrap_or(true) {
        reader
            .headers()
            .map_err(|error| format!("读取 CSV 表头失败: {error}"))?
            .iter()
            .map(|value| value.trim().to_string())
            .collect()
    } else {
        driver
            .list_columns(&table)
            .await?
            .into_iter()
            .filter(|column| !column.auto_increment)
            .map(|column| column.name)
            .collect()
    };

    if headers.is_empty() {
        return Err("CSV 没有可导入的列".to_string());
    }

    let mut inserted = 0u64;
    for record in reader.records() {
        let record = record.map_err(|error| format!("读取 CSV 行失败: {error}"))?;
        let mut values = Vec::new();

        for (index, header) in headers.iter().enumerate() {
            if header.is_empty() {
                continue;
            }
            let cell = record.get(index).unwrap_or_default();
            let value = if cell.is_empty() {
                Value::Null
            } else {
                Value::String(cell.to_string())
            };
            values.push(ColumnValue {
                column: header.clone(),
                value,
            });
        }

        if values.is_empty() {
            continue;
        }

        driver.insert_row(&table, &values).await?;
        inserted += 1;
    }

    Ok(inserted)
}

#[tauri::command]
pub async fn import_json(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    path: String,
) -> Result<u64, String> {
    ensure_table(&table)?;
    if path.trim().is_empty() {
        return Err("JSON 文件路径不能为空".to_string());
    }

    ensure_writable(&app, &connection_id)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取 JSON 文件失败: {error}"))?;
    let parsed: Value =
        serde_json::from_str(&text).map_err(|error| format!("解析 JSON 失败: {error}"))?;
    let rows = match parsed {
        Value::Array(rows) => rows,
        Value::Object(mut map) => map
            .remove("rows")
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| "JSON 需要是对象数组，或包含 rows 数组".to_string())?,
        _ => return Err("JSON 需要是对象数组，或包含 rows 数组".to_string()),
    };

    let driver = driver_for(&app, &state, &connection_id).await?;
    let columns = driver.list_columns(&table).await?;
    let allowed: HashSet<String> = columns
        .iter()
        .map(|column| column.name.clone())
        .collect();

    let mut inserted = 0u64;
    for (index, row) in rows.into_iter().enumerate() {
        let object = row
            .as_object()
            .ok_or_else(|| format!("第 {} 条记录不是 JSON 对象", index + 1))?;
        let mut values = Vec::new();
        for (column, value) in object {
            if !allowed.contains(column) {
                continue;
            }
            let value = match value {
                Value::Null => Value::Null,
                Value::String(_) | Value::Number(_) | Value::Bool(_) => value.clone(),
                other => Value::String(other.to_string()),
            };
            values.push(ColumnValue {
                column: column.clone(),
                value,
            });
        }
        if values.is_empty() {
            continue;
        }
        driver.insert_row(&table, &values).await?;
        inserted += 1;
    }

    Ok(inserted)
}

fn ensure_table(table: &str) -> Result<(), String> {
    if table.trim().is_empty() {
        return Err("表名不能为空".to_string());
    }
    Ok(())
}

fn csv_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn export_identifier(name: &str, mysql: bool) -> String {
    if mysql {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn sql_literal(value: &Value, mysql: bool) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(flag) => {
            if *flag {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Value::Number(number) => number.to_string(),
        Value::String(text) => format!("'{}'", escape_sql_string(text, mysql)),
        other => format!("'{}'", escape_sql_string(&other.to_string(), mysql)),
    }
}

fn escape_sql_string(text: &str, mysql: bool) -> String {
    let text = if mysql {
        text.replace('\\', "\\\\").replace('\0', "\\0")
    } else {
        text.replace('\0', "")
    };
    text.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sql_literals() {
        assert_eq!(sql_literal(&Value::Null, false), "NULL");
        assert_eq!(sql_literal(&Value::Bool(true), false), "TRUE");
        assert_eq!(sql_literal(&Value::from(42), false), "42");
        assert_eq!(
            sql_literal(&Value::String("O'Reilly".to_string()), false),
            "'O''Reilly'"
        );
        assert_eq!(
            sql_literal(&Value::String("a\\b".to_string()), true),
            "'a\\\\b'"
        );
    }

    #[test]
    fn quotes_export_identifiers() {
        assert_eq!(export_identifier("a`b", true), "`a``b`");
        assert_eq!(export_identifier("a\"b", false), "\"a\"\"b\"");
    }

    #[test]
    fn builds_table_maintenance_sql() {
        assert_eq!(
            table_maintenance_sql("mysql", "orders", "truncate").expect("truncate"),
            "TRUNCATE TABLE `orders`"
        );
        assert_eq!(
            table_maintenance_sql("postgres", "orders", "truncate").expect("pg truncate"),
            "TRUNCATE TABLE \"orders\" RESTART IDENTITY"
        );
        assert_eq!(
            table_maintenance_sql("postgres", "orders", "empty").expect("empty"),
            "DELETE FROM \"orders\""
        );
        assert_eq!(
            table_maintenance_sql("sqlite", "orders", "truncate").expect("sqlite truncate"),
            "DELETE FROM \"orders\""
        );
        assert!(table_maintenance_sql("mysql", "orders", "drop").is_err());
    }
}
