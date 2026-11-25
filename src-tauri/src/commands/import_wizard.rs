use super::{AppState, driver_for, ensure_writable};
use crate::models::{
    ColumnValue, ImportFailure, ImportMapping, ImportOptions, ImportPreview, ImportProgress,
    ImportSummary, PendingTableChange,
};
use serde_json::Value;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

fn delimiter_char(value: Option<&str>) -> Option<u8> {
    match value {
        None | Some("") => None,
        Some(",") => Some(b','),
        Some(";") => Some(b';'),
        Some("\t") | Some("tab") => Some(b'\t'),
        Some("|") => Some(b'|'),
        Some(_) => None,
    }
}

fn detect_delimiter(line: &str) -> u8 {
    let mut best = (b',', 0usize);
    for candidate in [b',', b';', b'\t', b'|'] {
        let count = line
            .as_bytes()
            .iter()
            .filter(|byte| **byte == candidate)
            .count();
        if count > best.1 {
            best = (candidate, count);
        }
    }
    best.0
}

fn delimiter_label(value: u8) -> String {
    match value {
        b'\t' => "\\t".to_string(),
        other => (other as char).to_string(),
    }
}

pub(crate) struct TabularFile {
    pub columns: Vec<String>,
    pub rows: Vec<(usize, Vec<String>)>,
    pub delimiter: u8,
}

pub(crate) fn read_tabular(
    path: &str,
    format: &str,
    options: &ImportOptions,
) -> Result<TabularFile, String> {
    match format {
        "json" => read_json(path),
        _ => read_csv(path, options),
    }
}

fn read_csv(path: &str, options: &ImportOptions) -> Result<TabularFile, String> {
    let file = File::open(path).map_err(|error| format!("打开 CSV 失败: {error}"))?;
    let mut content = Vec::new();
    use std::io::Read;
    BufReader::new(file)
        .take(64 * 1024)
        .read_to_end(&mut content)
        .map_err(|error| format!("读取 CSV 失败: {error}"))?;
    let content = String::from_utf8_lossy(&content);
    let first_line = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let delimiter = delimiter_char(options.delimiter.as_deref())
        .unwrap_or_else(|| detect_delimiter(first_line));

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .flexible(true)
        .from_path(path)
        .map_err(|error| format!("打开 CSV 失败: {error}"))?;
    let mut records: Vec<(usize, Vec<String>)> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| format!("读取 CSV 行失败: {error}"))?;
        let line = record
            .position()
            .map(|position| position.line() as usize)
            .unwrap_or(0);
        records.push((line, record.iter().map(|value| value.to_string()).collect()));
    }
    if records.is_empty() {
        return Err("CSV 文件没有数据".to_string());
    }

    let width = records.iter().map(|(_, row)| row.len()).max().unwrap_or(0);
    let (columns, data) = if options.has_header {
        let header = records.remove(0).1;
        let mut columns = header;
        while columns.len() < width {
            columns.push(format!("column_{}", columns.len() + 1));
        }
        (columns, records)
    } else {
        (
            (0..width)
                .map(|index| format!("column_{}", index + 1))
                .collect(),
            records,
        )
    };

    let rows = data
        .into_iter()
        .map(|(line, mut values)| {
            while values.len() < columns.len() {
                values.push(String::new());
            }
            values.truncate(columns.len());
            (line, values)
        })
        .collect();
    Ok(TabularFile {
        columns,
        rows,
        delimiter,
    })
}

fn json_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

fn read_json(path: &str) -> Result<TabularFile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("读取 JSON 失败（仅支持 UTF-8）: {error}"))?;
    if text.len() > 50 * 1024 * 1024 {
        return Err("JSON 文件超过 50 MB，请拆分后再导入".to_string());
    }
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
    if rows.is_empty() {
        return Err("JSON 文件没有数据".to_string());
    }
    let mut columns: Vec<String> = Vec::new();
    for row in rows.iter().take(200) {
        if let Some(object) = row.as_object() {
            for key in object.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
    }
    if columns.is_empty() {
        return Err("JSON 数组里没有对象字段".to_string());
    }
    let data = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let object = row.as_object().cloned().unwrap_or_default();
            (
                index + 1,
                columns
                    .iter()
                    .map(|column| object.get(column).map(json_cell).unwrap_or_default())
                    .collect(),
            )
        })
        .collect();
    Ok(TabularFile {
        columns,
        rows: data,
        delimiter: b',',
    })
}

#[tauri::command]
pub async fn preview_import_file(
    path: String,
    format: String,
    options: ImportOptions,
) -> Result<ImportPreview, String> {
    if path.trim().is_empty() {
        return Err("请选择导入文件".to_string());
    }
    let tabular = read_tabular(&path, &format, &options)?;
    let rows = tabular
        .rows
        .iter()
        .take(20)
        .map(|(_, row)| row.clone())
        .collect();
    Ok(ImportPreview {
        columns: tabular.columns,
        rows,
        detected_delimiter: delimiter_label(tabular.delimiter),
        format,
    })
}

#[tauri::command]
pub async fn cancel_import(state: State<'_, AppState>, operation_id: String) -> Result<(), String> {
    if let Some(flag) = state.import_cancels.lock().await.get(&operation_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn execute_import_file(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    table: String,
    path: String,
    format: String,
    options: ImportOptions,
    mappings: Vec<ImportMapping>,
    operation_id: Option<String>,
) -> Result<ImportSummary, String> {
    ensure_writable(&app, &connection_id)?;
    if table.trim().is_empty() {
        return Err("表名不能为空".to_string());
    }
    if !matches!(options.error_mode.as_str(), "rollback" | "stop" | "skip") {
        return Err("错误策略无效".to_string());
    }
    let started = Instant::now();
    let tabular = read_tabular(&path, &format, &options)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let table_columns = driver.list_columns(&table).await?;

    let mut targets = std::collections::HashSet::new();
    let mut active: Vec<(usize, String)> = Vec::new();
    for mapping in &mappings {
        let Some(target) = mapping
            .target
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        if !table_columns.iter().any(|column| &column.name == target) {
            return Err(format!("目标字段不存在: {target}"));
        }
        if !targets.insert(target.clone()) {
            return Err(format!("字段 {target} 被映射了多次"));
        }
        let source_index = tabular
            .columns
            .iter()
            .position(|column| column == &mapping.source)
            .ok_or_else(|| format!("文件列不存在: {}", mapping.source))?;
        active.push((source_index, target.clone()));
    }
    if active.is_empty() {
        return Err("请至少映射一个字段".to_string());
    }
    for column in &table_columns {
        if column.auto_increment || column.nullable || column.default_value.is_some() {
            continue;
        }
        if !targets.contains(&column.name) {
            return Err(format!(
                "字段 {} 为 NOT NULL 且没有默认值，必须映射",
                column.name
            ));
        }
    }

    let operation_id = operation_id
        .clone()
        .unwrap_or_else(|| format!("import-{}", uuid::Uuid::new_v4()));
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .import_cancels
        .lock()
        .await
        .insert(operation_id.clone(), cancel.clone());
    let total = tabular.rows.len();
    let progress = |processed: usize, inserted: usize, failed: usize, message: &str| {
        let _ = app.emit(
            "import:progress",
            ImportProgress {
                operation_id: Some(operation_id.clone()),
                processed,
                inserted,
                failed,
                total: Some(total),
                message: message.to_string(),
            },
        );
    };

    let mut prepared: Vec<(usize, Vec<ColumnValue>)> = Vec::new();
    for (line, row) in &tabular.rows {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut values = Vec::new();
        for (source, target) in &active {
            let raw = row.get(*source).cloned().unwrap_or_default();
            let value = if raw.is_empty() && options.empty_as_null {
                Value::Null
            } else {
                Value::String(raw)
            };
            values.push(ColumnValue {
                column: target.clone(),
                value,
            });
        }
        prepared.push((*line, values));
    }
    progress(prepared.len(), 0, 0, "parsed");

    let mut summary = ImportSummary {
        inserted: 0,
        failed: 0,
        skipped: 0,
        elapsed_ms: 0,
        cancelled: false,
        rolled_back: false,
        failures: Vec::new(),
    };

    if cancel.load(Ordering::Relaxed) || prepared.len() < total {
        summary.cancelled = true;
        summary.rolled_back = options.error_mode == "rollback";
        summary.elapsed_ms = started.elapsed().as_millis() as u64;
        state.import_cancels.lock().await.remove(&operation_id);
        progress(0, 0, 0, "cancelled");
        return Ok(summary);
    }

    if options.error_mode == "rollback" {
        let changes: Vec<PendingTableChange> = prepared
            .iter()
            .map(|(_, values)| PendingTableChange {
                kind: "insert".to_string(),
                keys: Vec::new(),
                changes: Vec::new(),
                values: values.clone(),
                single_row: false,
            })
            .collect();
        match driver.apply_table_changes(&table, &changes).await {
            Ok(inserted) => {
                summary.inserted = inserted;
                progress(inserted, inserted, 0, "done");
            }
            Err(error) => {
                summary.rolled_back = true;
                summary.failed = 1;
                let row = error
                    .split("第 ")
                    .nth(1)
                    .and_then(|rest| rest.split(' ').next())
                    .and_then(|number| number.parse::<usize>().ok())
                    .and_then(|index| prepared.get(index.saturating_sub(1)))
                    .map(|(line, _)| *line)
                    .unwrap_or(0);
                summary.failures.push(ImportFailure {
                    row,
                    message: error,
                });
            }
        }
    } else {
        for (index, (line, values)) in prepared.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                summary.cancelled = true;
                break;
            }
            match driver.insert_row(&table, values).await {
                Ok(_) => summary.inserted += 1,
                Err(error) => {
                    summary.failed += 1;
                    if summary.failures.len() < 100 {
                        summary.failures.push(ImportFailure {
                            row: *line,
                            message: error.clone(),
                        });
                    }
                    if options.error_mode == "stop" {
                        summary.skipped = total.saturating_sub(index + 1);
                        break;
                    }
                }
            }
            if (index + 1) % 50 == 0 {
                progress(index + 1, summary.inserted, summary.failed, "inserting");
            }
        }
        progress(
            summary.inserted + summary.failed,
            summary.inserted,
            summary.failed,
            "done",
        );
    }

    state.import_cancels.lock().await.remove(&operation_id);
    summary.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(summary)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_csv_with_header_and_semicolon() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("users.csv");
        std::fs::write(&path, "id;name\n1;Ana\n2;Ben\n").expect("write");
        let options = ImportOptions {
            delimiter: None,
            has_header: true,
            error_mode: "rollback".to_string(),
            empty_as_null: true,
        };
        let table = read_tabular(path.to_string_lossy().as_ref(), "csv", &options).expect("csv");
        assert_eq!(table.columns, vec!["id", "name"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[1].1, vec!["2", "Ben"]);
        assert_eq!(table.delimiter, b';');
    }

    #[test]
    fn unions_json_object_columns_and_stringifies_nested_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("rows.json");
        std::fs::write(&path, r#"[{"id":1,"meta":{"a":2}},{"id":2,"name":"x"}]"#).expect("write");
        let options = ImportOptions::default();
        let table = read_tabular(path.to_string_lossy().as_ref(), "json", &options).expect("json");
        assert_eq!(table.columns, vec!["id", "meta", "name"]);
        assert_eq!(table.rows[0].1[1], "{\"a\":2}");
        assert_eq!(table.rows[1].1[1], "");
    }
}
