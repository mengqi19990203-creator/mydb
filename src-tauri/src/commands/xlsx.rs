use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};
use serde_json::Value;

use crate::models::XlsxExportSummary;

/// Excel refuses sheet names longer than 31 characters and containing any of
/// `[]:*?/\`. Keep the readable part and fall back to `Sheet1`.
pub fn safe_sheet_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|character| !matches!(character, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('\'').trim();
    let mut result: String = cleaned.chars().take(31).collect();
    if result.trim().is_empty() {
        result = "Sheet1".to_string();
    }
    result
}

/// Estimate a useful column width from the header and a sample of values.
pub fn estimate_width(header: &str, values: impl Iterator<Item = String>) -> f64 {
    let mut widest = header.chars().count();
    for value in values.take(200) {
        let length = value.chars().count();
        if length > widest {
            widest = length;
        }
        if widest >= 60 {
            break;
        }
    }
    (widest as f64 + 2.0).clamp(8.0, 60.0)
}

fn truncate_cell(text: &str) -> String {
    if text.chars().count() <= 32_767 {
        return text.to_string();
    }
    text.chars().take(32_767).collect()
}

/// Write one JSON value as a native XLSX cell.
///
/// Strings always use `write_string`, so values beginning with `=`, `+`, `-`
/// or `@` are stored as text and are never parsed as formulas by the
/// spreadsheet application.
pub fn write_cell(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: &Value,
) -> Result<(), XlsxError> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(flag) => worksheet.write_boolean(row, col, *flag).map(|_| ()),
        Value::Number(number) => {
            if let Some(value) = number.as_f64() {
                if value.is_finite() {
                    return worksheet.write_number(row, col, value).map(|_| ());
                }
            }
            worksheet
                .write_string(row, col, truncate_cell(&number.to_string()))
                .map(|_| ())
        }
        Value::String(text) => worksheet
            .write_string(row, col, truncate_cell(text))
            .map(|_| ()),
        other => worksheet
            .write_string(row, col, truncate_cell(&other.to_string()))
            .map(|_| ()),
    }
}

pub fn write_header(
    worksheet: &mut Worksheet,
    columns: &[String],
    bold: &Format,
) -> Result<(), XlsxError> {
    for (index, column) in columns.iter().enumerate() {
        worksheet.write_string_with_format(0, index as u16, column, bold)?;
    }
    Ok(())
}

/// Build a workbook from already-materialised rows (query result / current
/// table page). Rows are capped by the caller.
pub fn build_workbook(
    columns: &[String],
    rows: &[Vec<Value>],
    sheet_name: &str,
    with_header: bool,
) -> Result<Vec<u8>, String> {
    if columns.is_empty() {
        return Err("没有可导出的列".to_string());
    }
    let mut workbook = Workbook::new();
    let bold = Format::new().set_bold();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(&safe_sheet_name(sheet_name))
        .map_err(|error| format!("设置工作表名称失败: {error}"))?;

    let data_start = if with_header {
        write_header(worksheet, columns, &bold)
            .map_err(|error| format!("写入 XLSX 表头失败: {error}"))?;
        1u32
    } else {
        0
    };

    for (row_index, row) in rows.iter().enumerate() {
        let row_number = data_start + row_index as u32;
        for (column_index, value) in row.iter().enumerate() {
            if column_index >= columns.len() {
                break;
            }
            write_cell(worksheet, row_number, column_index as u16, value)
                .map_err(|error| format!("写入 XLSX 单元格失败: {error}"))?;
        }
    }

    if with_header && !rows.is_empty() {
        worksheet
            .set_freeze_panes(1, 0)
            .map_err(|error| format!("冻结表头失败: {error}"))?;
        worksheet
            .autofilter(0, 0, rows.len() as u32, (columns.len() - 1) as u16)
            .map_err(|error| format!("设置筛选失败: {error}"))?;
    }

    for (index, column) in columns.iter().enumerate() {
        let sample = rows.iter().take(200).map(|row| match row.get(index) {
            Some(Value::Null) | None => String::new(),
            Some(Value::String(text)) => text.clone(),
            Some(other) => other.to_string(),
        });
        let width = estimate_width(column, sample);
        worksheet
            .set_column_width(index as u16, width)
            .map_err(|error| format!("设置列宽失败: {error}"))?;
    }

    workbook
        .save_to_buffer()
        .map_err(|error| format!("生成 XLSX 失败: {error}"))
}

/// Write bytes through a temporary file in the destination directory, then
/// rename over the target. A failed write leaves no partial target file.
pub fn write_bytes_atomic(path: &str, contents: &[u8]) -> Result<u64, String> {
    let path = std::path::Path::new(path);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let file_name = path
        .file_name()
        .ok_or_else(|| "导出路径缺少文件名".to_string())?;
    let parent = parent.unwrap_or_else(|| std::path::Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name.to_string_lossy(),
        std::process::id(),
        stamp
    ));

    std::fs::write(&temp, contents).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!("写入文件失败: {error}")
    })?;

    if let Err(error) = std::fs::rename(&temp, path) {
        #[cfg(windows)]
        {
            if path.exists() {
                let _ = std::fs::remove_file(path);
                if std::fs::rename(&temp, path).is_ok() {
                    return Ok(contents.len() as u64);
                }
            }
        }
        let _ = std::fs::remove_file(&temp);
        return Err(format!("写入文件失败: {error}"));
    }
    Ok(contents.len() as u64)
}

/// Result-set / table-page XLSX export. Runs on a blocking thread because
/// workbook serialisation is CPU-bound.
#[tauri::command]
pub async fn export_query_result_xlsx(
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    path: String,
    sheet_name: Option<String>,
    with_header: Option<bool>,
) -> Result<XlsxExportSummary, String> {
    if path.trim().is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    let sheet_name = sheet_name.unwrap_or_else(|| "QueryResult".to_string());
    let with_header = with_header.unwrap_or(true);
    let row_count = rows.len() as u64;

    tokio::task::spawn_blocking(move || {
        let buffer = build_workbook(&columns, &rows, &sheet_name, with_header)?;
        let bytes = write_bytes_atomic(&path, &buffer)?;
        Ok(XlsxExportSummary {
            path,
            rows: row_count,
            sheet_name: safe_sheet_name(&sheet_name),
            bytes,
        })
    })
    .await
    .map_err(|error| format!("XLSX 导出任务失败: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_sheet_names() {
        assert_eq!(safe_sheet_name("orders[2026]:x/y"), "orders2026xy");
        assert_eq!(safe_sheet_name("   "), "Sheet1");
        assert_eq!(safe_sheet_name(&"a".repeat(40)), "a".repeat(31));
    }

    #[test]
    fn builds_formula_safe_text_cells() {
        let columns = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec![Value::String("=1+1".to_string()), Value::from(42)]];
        let buffer = build_workbook(&columns, &rows, "Sheet1", true).expect("workbook");
        assert!(buffer.starts_with(b"PK"), "xlsx is a zip archive");
        assert!(buffer.len() > 1000);
    }

    #[test]
    fn writes_bytes_atomically() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("out.xlsx");
        write_bytes_atomic(path.to_string_lossy().as_ref(), b"first").expect("write");
        write_bytes_atomic(path.to_string_lossy().as_ref(), b"second").expect("overwrite");
        assert_eq!(std::fs::read(&path).expect("read"), b"second");
        assert_eq!(
            std::fs::read_dir(dir.path()).expect("read dir").count(),
            1,
            "temporary files should be cleaned up"
        );
    }
}
