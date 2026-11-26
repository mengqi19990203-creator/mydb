use super::{
    AppState, context_label, driver_for, ensure_writable, isolated_driver_for, resolve_source,
    resolve_target,
};
use crate::models::ColumnInfo;
use crate::storage;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, State};

const DEFAULT_MAX_ROWS: u64 = 5_000;
const HARD_MAX_ROWS: u64 = 50_000;
const MAX_CHANGE_DETAILS: usize = 200;
const KEY_SEPARATOR: &str = "\u{1f}";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowChange {
    /// added | updated | removed
    pub status: String,
    pub key: String,
    pub changed_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataDiff {
    pub source_label: String,
    pub target_label: String,
    pub source_table: String,
    pub target_table: String,
    pub key_columns: Vec<String>,
    pub compared_columns: Vec<String>,
    pub skipped_columns: Vec<String>,
    pub source_only_columns: Vec<String>,
    pub target_only_columns: Vec<String>,
    pub insert_count: usize,
    pub update_count: usize,
    pub delete_count: usize,
    pub same_count: usize,
    pub source_total: u64,
    pub target_total: u64,
    pub truncated: bool,
    pub changes: Vec<RowChange>,
    pub changes_truncated: bool,
    pub statements: Vec<String>,
    pub sql: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct RowsSnapshot {
    columns: Vec<ColumnInfo>,
    rows: Vec<Vec<Value>>,
    total: u64,
}

#[tauri::command]
pub async fn compare_table_data(
    app: AppHandle,
    state: State<'_, AppState>,
    source_connection_id: String,
    target_connection_id: String,
    source_table: String,
    target_table: String,
    source_database: Option<String>,
    source_schema: Option<String>,
    target_database: Option<String>,
    target_schema: Option<String>,
    max_rows: Option<u64>,
) -> Result<TableDataDiff, String> {
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
        return Err("只支持同一类型数据库之间对比".to_string());
    }
    if source_config.driver == "redis" {
        return Err("Redis 不支持数据对比".to_string());
    }

    let source_table = source_table.trim().to_string();
    let target_table = target_table.trim().to_string();
    if source_table.is_empty() || target_table.is_empty() {
        return Err("请选择源表和目标表".to_string());
    }

    let resolved_source = resolve_source(
        &app,
        &state,
        source_config,
        &source_connection_id,
        source_database.as_deref(),
        source_schema.as_deref(),
    )
    .await?;
    let resolved_target = resolve_target(
        &app,
        &state,
        source_config,
        &resolved_source.driver,
        &target_connection_id,
        target_database.as_deref(),
        target_schema.as_deref(),
    )
    .await?;

    let source_label = context_label(
        &source_config.name,
        &source_config.driver,
        resolved_source.database.as_deref(),
        resolved_source.schema.as_deref(),
    );
    let target_label = context_label(
        &target_config.name,
        &source_config.driver,
        resolved_target.database.as_deref(),
        resolved_target.schema.as_deref(),
    );

    let source_tables = resolved_source.driver.list_tables().await?;
    if !source_tables
        .iter()
        .any(|table| table.name == source_table && table.kind == "table")
    {
        return Err(format!("源对象 {source_table} 不是源库 / 模式下的表"));
    }
    let target_tables = resolved_target.driver.list_tables().await?;
    if !target_tables
        .iter()
        .any(|table| table.name == target_table && table.kind == "table")
    {
        return Err(format!("目标对象 {target_table} 不是目标库 / 模式下的表"));
    }

    let source_columns = resolved_source
        .driver
        .list_columns(&source_table)
        .await
        .map_err(|error| format!("读取源表字段失败: {error}"))?;
    let target_columns = resolved_target
        .driver
        .list_columns(&target_table)
        .await
        .map_err(|error| format!("读取目标表字段失败: {error}"))?;

    let key_columns = primary_keys(&source_columns)
        .map_err(|_| format!("源表 {source_table} 没有主键，无法逐行比对"))?;
    let target_keys = primary_keys(&target_columns)
        .map_err(|_| format!("目标表 {target_table} 没有主键，无法逐行比对"))?;
    if key_columns != target_keys {
        return Err(format!(
            "源表与目标表的主键不一致：源 [{}]，目标 [{}]",
            key_columns.join(", "),
            target_keys.join(", ")
        ));
    }

    let limit = max_rows
        .unwrap_or(DEFAULT_MAX_ROWS)
        .clamp(100, HARD_MAX_ROWS);
    let order_by = if key_columns.len() == 1 {
        Some(key_columns[0].as_str())
    } else {
        None
    };

    let source_rows = resolved_source
        .driver
        .read_rows(&source_table, limit, 0, None, order_by, Some("asc"))
        .await
        .map_err(|error| format!("读取源表数据失败: {error}"))?;
    let target_rows = resolved_target
        .driver
        .read_rows(&target_table, limit, 0, None, order_by, Some("asc"))
        .await
        .map_err(|error| format!("读取目标表数据失败: {error}"))?;

    let source = RowsSnapshot {
        columns: source_rows.columns,
        rows: source_rows.rows,
        total: source_rows.total,
    };
    let target = RowsSnapshot {
        columns: target_rows.columns,
        rows: target_rows.rows,
        total: target_rows.total,
    };

    build_data_diff(
        &source_label,
        &target_label,
        &source_table,
        &target_table,
        &source_config.driver,
        &source,
        &target,
    )
}

#[tauri::command]
pub async fn apply_table_data_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    target_connection_id: String,
    target_database: Option<String>,
    target_schema: Option<String>,
    statements: Vec<String>,
) -> Result<u64, String> {
    ensure_writable(&app, &target_connection_id)?;
    if statements.is_empty() {
        return Ok(0);
    }
    let has_database = target_database
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_schema = target_schema
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let driver = if has_database || has_schema {
        isolated_driver_for(
            &app,
            &state,
            &target_connection_id,
            target_database.as_deref(),
            target_schema.as_deref(),
        )
        .await?
    } else {
        driver_for(&app, &state, &target_connection_id).await?
    };
    driver.execute_transaction(&statements).await
}

fn primary_keys(columns: &[ColumnInfo]) -> Result<Vec<String>, String> {
    let keys: Vec<String> = columns
        .iter()
        .filter(|column| column.primary_key)
        .map(|column| column.name.clone())
        .collect();
    if keys.is_empty() {
        Err("表没有主键".to_string())
    } else {
        Ok(keys)
    }
}

fn index_columns(columns: &[ColumnInfo]) -> HashMap<&str, usize> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| (column.name.as_str(), index))
        .collect()
}

fn build_data_diff(
    source_label: &str,
    target_label: &str,
    source_table: &str,
    target_table: &str,
    driver: &str,
    source: &RowsSnapshot,
    target: &RowsSnapshot,
) -> Result<TableDataDiff, String> {
    let source_index = index_columns(&source.columns);
    let target_index = index_columns(&target.columns);
    let key_columns = primary_keys(&source.columns)?;
    let target_keys = primary_keys(&target.columns)?;
    if key_columns != target_keys {
        return Err("源表与目标表的主键不一致".to_string());
    }
    for key in &key_columns {
        let column = &source.columns[source_index[key.as_str()]];
        if unsupported_type(&column.data_type) {
            return Err(format!(
                "主键字段 {} 的类型 {} 暂不支持数据同步",
                column.name, column.data_type
            ));
        }
    }

    let mut compared_columns = Vec::new();
    let mut skipped_columns = Vec::new();
    let mut source_only_columns = Vec::new();
    let mut target_only_columns = Vec::new();
    let mut warnings = Vec::new();

    for column in &source.columns {
        match target_index.get(column.name.as_str()) {
            Some(&target_position) => {
                if unsupported_type(&column.data_type)
                    || unsupported_type(&target.columns[target_position].data_type)
                {
                    skipped_columns.push(column.name.clone());
                } else {
                    compared_columns.push(column.name.clone());
                }
            }
            None => source_only_columns.push(column.name.clone()),
        }
    }
    for column in &target.columns {
        if !source_index.contains_key(column.name.as_str()) {
            target_only_columns.push(column.name.clone());
        }
    }

    if compared_columns.is_empty() {
        return Err("两张表没有可同步的公共字段".to_string());
    }

    if !skipped_columns.is_empty() {
        warnings.push(format!(
            "二进制 / 数组字段暂不参与比较和写入：{}（若这些字段为 NOT NULL 且无默认值，新增行可能失败）",
            skipped_columns.join(", ")
        ));
    }
    if !source_only_columns.is_empty() {
        warnings.push(format!(
            "源表字段在目标表中不存在，已跳过：{}",
            source_only_columns.join(", ")
        ));
    }
    if !target_only_columns.is_empty() {
        warnings.push(format!(
            "目标表多出的字段不会写入：{}",
            target_only_columns.join(", ")
        ));
    }

    let target_key_auto_increment = key_columns
        .iter()
        .any(|key| target.columns[target_index[key.as_str()]].auto_increment);
    if target_key_auto_increment {
        warnings.push(
            "目标表主键包含自增字段：脚本会显式写入主键值，可能造成自增值 / 序列不同步".to_string(),
        );
    }

    let source_key_indices: Vec<usize> = key_columns
        .iter()
        .map(|name| source_index[name.as_str()])
        .collect();
    let target_key_indices: Vec<usize> = key_columns
        .iter()
        .map(|name| target_index[name.as_str()])
        .collect();

    let source_positions: HashMap<String, usize> = source
        .rows
        .iter()
        .enumerate()
        .map(|(position, row)| (row_key(row, &source_key_indices), position))
        .collect();
    let target_positions: HashMap<String, usize> = target
        .rows
        .iter()
        .enumerate()
        .map(|(position, row)| (row_key(row, &target_key_indices), position))
        .collect();

    let source_truncated = source.total > source.rows.len() as u64;
    let target_truncated = target.total > target.rows.len() as u64;
    let truncated = source_truncated || target_truncated;
    if source_truncated {
        warnings.push(format!(
            "源表共有 {} 行，本次只读取前 {} 行",
            source.total,
            source.rows.len()
        ));
    }
    if target_truncated {
        warnings.push(format!(
            "目标表共有 {} 行，本次只读取前 {} 行",
            target.total,
            target.rows.len()
        ));
    }
    if truncated {
        warnings.push(
            "比较范围已被截断，建议调高每侧行数上限，或先通过 WHERE / 视图缩小范围".to_string(),
        );
        if key_columns.len() > 1 {
            warnings.push(
                "复合主键无法按主键排序读取，截断范围内的结果可能不稳定，建议调高上限或缩小范围"
                    .to_string(),
            );
        }
    }

    let mut changes = Vec::new();
    let mut changes_truncated = false;
    let mut delete_statements = Vec::new();
    let mut update_statements = Vec::new();
    let mut insert_statements = Vec::new();
    let mut same_count = 0usize;

    // Deletes first: removing target-only rows frees unique values that
    // inserts may reuse.
    if source_truncated {
        let skipped = target
            .rows
            .iter()
            .filter(|row| !source_positions.contains_key(&row_key(row, &target_key_indices)))
            .count();
        if skipped > 0 {
            warnings.push(format!(
                "源表读取被截断，已跳过 {skipped} 条 DELETE，避免误删未读取到的源行"
            ));
        }
    } else {
        for row in &target.rows {
            if source_positions.contains_key(&row_key(row, &target_key_indices)) {
                continue;
            }
            let key = display_key(row, &key_columns, &target_key_indices);
            delete_statements.push(format!(
                "DELETE FROM {} {};",
                quote_identifier(target_table, driver),
                where_clause(&key_columns, &target_key_indices, row, driver)
            ));
            push_change(
                &mut changes,
                &mut changes_truncated,
                RowChange {
                    status: "removed".to_string(),
                    key,
                    changed_columns: Vec::new(),
                },
            );
        }
    }

    for source_row in &source.rows {
        let key = row_key(source_row, &source_key_indices);
        let Some(&target_position) = target_positions.get(&key) else {
            if target_truncated {
                continue;
            }
            insert_statements.push(insert_statement(
                source_row,
                target_table,
                &source_index,
                &compared_columns,
                driver,
            ));
            push_change(
                &mut changes,
                &mut changes_truncated,
                RowChange {
                    status: "added".to_string(),
                    key: display_key(source_row, &key_columns, &source_key_indices),
                    changed_columns: Vec::new(),
                },
            );
            continue;
        };
        let target_row = &target.rows[target_position];

        let mut changed_columns = Vec::new();
        let mut assignments = Vec::new();
        for name in &compared_columns {
            if key_columns.contains(name) {
                continue;
            }
            let source_value = &source_row[source_index[name.as_str()]];
            let target_value = &target_row[target_index[name.as_str()]];
            if canonical_value(source_value) != canonical_value(target_value) {
                changed_columns.push(name.clone());
                assignments.push(format!(
                    "{} = {}",
                    quote_identifier(name, driver),
                    sql_literal(source_value, driver)
                ));
            }
        }

        if assignments.is_empty() {
            same_count += 1;
            continue;
        }

        update_statements.push(format!(
            "UPDATE {} SET {} {};",
            quote_identifier(target_table, driver),
            assignments.join(", "),
            where_clause(&key_columns, &source_key_indices, source_row, driver)
        ));
        push_change(
            &mut changes,
            &mut changes_truncated,
            RowChange {
                status: "updated".to_string(),
                key: display_key(source_row, &key_columns, &source_key_indices),
                changed_columns,
            },
        );
    }

    if target_truncated && !target_positions.is_empty() {
        let skipped = source
            .rows
            .iter()
            .filter(|row| !target_positions.contains_key(&row_key(row, &source_key_indices)))
            .count();
        if skipped > 0 {
            warnings.push(format!(
                "目标表读取被截断，已跳过 {skipped} 条 INSERT，避免与未读取到的目标行冲突"
            ));
        }
    }

    let mut statements = delete_statements;
    let delete_count = statements.len();
    statements.extend(update_statements);
    let update_count = statements.len() - delete_count;
    statements.extend(insert_statements);
    let insert_count = statements.len() - delete_count - update_count;

    if delete_count > 0 {
        warnings.push("DELETE 语句会永久删除目标行，执行前请确认".to_string());
    }
    if !statements.is_empty() {
        warnings.push(
            "脚本使用未限定的表名，请在目标连接的当前数据库 / 模式下执行；语句按 删除 → 更新 → 新增 排序，并在一个事务中执行，唯一约束或外键可能导致整批回滚"
                .to_string(),
        );
    }
    if changes_truncated {
        warnings.push(format!(
            "行级差异只展示前 {MAX_CHANGE_DETAILS} 条，完整内容见 SQL 预览"
        ));
    }

    let mut sql = String::new();
    sql.push_str(&format!(
        "-- mydb 数据同步：{} · {} → {} · {}\n",
        source_label, source_table, target_label, target_table
    ));
    sql.push_str(&format!(
        "-- 新增 {} 行 · 更新 {} 行 · 删除 {} 行 · 一致 {} 行\n",
        insert_count, update_count, delete_count, same_count
    ));
    if statements.is_empty() {
        sql.push_str("-- 两边数据一致，没有需要执行的语句\n");
    } else {
        sql.push_str("-- 以下语句在目标连接上按一个事务执行\nBEGIN;\n");
        for statement in &statements {
            sql.push_str(statement);
            sql.push('\n');
        }
        sql.push_str("COMMIT;\n");
    }

    Ok(TableDataDiff {
        source_label: source_label.to_string(),
        target_label: target_label.to_string(),
        source_table: source_table.to_string(),
        target_table: target_table.to_string(),
        key_columns,
        compared_columns,
        skipped_columns,
        source_only_columns,
        target_only_columns,
        insert_count,
        update_count,
        delete_count,
        same_count,
        source_total: source.total,
        target_total: target.total,
        truncated,
        changes,
        changes_truncated,
        statements,
        sql,
        warnings,
    })
}

fn insert_statement(
    row: &[Value],
    target_table: &str,
    source_index: &HashMap<&str, usize>,
    compared_columns: &[String],
    driver: &str,
) -> String {
    let columns = compared_columns
        .iter()
        .map(|name| quote_identifier(name, driver))
        .collect::<Vec<_>>()
        .join(", ");
    let values = compared_columns
        .iter()
        .map(|name| sql_literal(&row[source_index[name.as_str()]], driver))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {} ({columns}) VALUES ({values});",
        quote_identifier(target_table, driver)
    )
}

fn where_clause(
    key_columns: &[String],
    key_indices: &[usize],
    row: &[Value],
    driver: &str,
) -> String {
    let conditions = key_columns
        .iter()
        .zip(key_indices)
        .map(|(name, index)| {
            format!(
                "{} = {}",
                quote_identifier(name, driver),
                sql_literal(row.get(*index).unwrap_or(&Value::Null), driver)
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    format!("WHERE {conditions}")
}

fn push_change(changes: &mut Vec<RowChange>, truncated: &mut bool, change: RowChange) {
    if changes.len() < MAX_CHANGE_DETAILS {
        changes.push(change);
    } else {
        *truncated = true;
    }
}

fn row_key(row: &[Value], indices: &[usize]) -> String {
    indices
        .iter()
        .map(|index| canonical_value(row.get(*index).unwrap_or(&Value::Null)))
        .collect::<Vec<_>>()
        .join(KEY_SEPARATOR)
}

fn display_key(row: &[Value], key_columns: &[String], indices: &[usize]) -> String {
    key_columns
        .iter()
        .zip(indices)
        .map(|(name, index)| {
            format!(
                "{}={}",
                name,
                display_value(row.get(*index).unwrap_or(&Value::Null))
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_value(value: &Value) -> String {
    let text = match value {
        Value::Null => "NULL".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    if text.chars().count() > 40 {
        let mut shortened: String = text.chars().take(40).collect();
        shortened.push('…');
        shortened
    } else {
        text
    }
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                integer.to_string()
            } else if let Some(integer) = number.as_u64() {
                integer.to_string()
            } else if let Some(float) = number.as_f64() {
                if float.is_finite()
                    && float.fract() == 0.0
                    && float.abs() < 9_007_199_254_740_992.0
                {
                    format!("{}", float as i64)
                } else {
                    format!("{float}")
                }
            } else {
                number.to_string()
            }
        }
        Value::String(text) => format!("\"{text}\""),
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(canonical_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(map) => {
            // serde_json may be compiled with `preserve_order` (enabled by the
            // BSON dependency), so never rely on map iteration order here.
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, item)| format!(
                        "{}:{}",
                        canonical_value(&Value::String(key.clone())),
                        canonical_value(item)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn unsupported_type(data_type: &str) -> bool {
    let upper = data_type.trim().to_ascii_uppercase();
    upper.contains("BLOB")
        || upper.contains("BINARY")
        || upper.contains("BYTEA")
        || upper.contains("GEOMETRY")
        || upper.contains("GEOGRAPHY")
        || upper.ends_with("[]")
}

fn quote_identifier(name: &str, driver: &str) -> String {
    if driver == "mysql" {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn sql_literal(value: &Value, driver: &str) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(flag) => {
            let text = if driver == "sqlite" {
                if *flag { "1" } else { "0" }
            } else if *flag {
                "TRUE"
            } else {
                "FALSE"
            };
            text.to_string()
        }
        Value::Number(number) => number.to_string(),
        Value::String(text) => quote_string(text, driver),
        other => quote_string(&serde_json::to_string(other).unwrap_or_default(), driver),
    }
}

fn quote_string(value: &str, driver: &str) -> String {
    if driver == "mysql" {
        let mut escaped = String::with_capacity(value.len() + 2);
        for character in value.chars() {
            match character {
                '\\' => escaped.push_str("\\\\"),
                '\'' => escaped.push_str("''"),
                '\0' => escaped.push_str("\\0"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\u{1a}' => escaped.push_str("\\Z"),
                other => escaped.push(other),
            }
        }
        format!("'{escaped}'")
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn column(
        name: &str,
        data_type: &str,
        nullable: bool,
        primary_key: bool,
        auto_increment: bool,
    ) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            nullable,
            primary_key,
            auto_increment,
            default_value: None,
            comment: None,
        }
    }

    fn snapshot(columns: Vec<ColumnInfo>, rows: Vec<Vec<Value>>, total: u64) -> RowsSnapshot {
        RowsSnapshot {
            columns,
            rows,
            total,
        }
    }

    #[test]
    fn diffs_inserted_updated_and_deleted_rows() {
        let columns = vec![
            column("id", "INTEGER", false, true, false),
            column("name", "TEXT", true, false, false),
            column("price", "REAL", true, false, false),
        ];
        let source = snapshot(
            columns.clone(),
            vec![
                vec![json!(1), json!("a"), json!(1.5)],
                vec![json!(2), json!("bb"), json!(2.0)],
                vec![json!(3), json!("new"), Value::Null],
            ],
            3,
        );
        let target = snapshot(
            columns,
            vec![
                vec![json!(1), json!("a"), json!(1.5)],
                vec![json!(2), json!("b"), json!(2.0)],
                vec![json!(4), json!("x"), json!(4.0)],
            ],
            3,
        );

        let diff = build_data_diff("src", "dst", "items", "items", "sqlite", &source, &target)
            .expect("diff");

        assert_eq!(diff.insert_count, 1);
        assert_eq!(diff.update_count, 1);
        assert_eq!(diff.delete_count, 1);
        assert_eq!(diff.same_count, 1);
        assert_eq!(diff.key_columns, vec!["id".to_string()]);
        assert_eq!(
            diff.compared_columns,
            vec!["id".to_string(), "name".to_string(), "price".to_string()]
        );

        assert!(diff.statements[0].starts_with("DELETE FROM \"items\""));
        assert!(diff.statements[0].contains("\"id\" = 4"));
        assert!(diff.statements[1].starts_with("UPDATE \"items\" SET \"name\" = 'bb'"));
        assert!(diff.statements[2].starts_with("INSERT INTO \"items\""));
        assert!(diff.sql.contains("BEGIN;"));
        assert!(diff.sql.contains("COMMIT;"));

        let removed = diff
            .changes
            .iter()
            .find(|change| change.status == "removed")
            .expect("removed change");
        assert_eq!(removed.key, "id=4");
        let updated = diff
            .changes
            .iter()
            .find(|change| change.status == "updated")
            .expect("updated change");
        assert_eq!(updated.changed_columns, vec!["name".to_string()]);
    }

    #[tokio::test]
    async fn applies_generated_sql_to_a_target_sqlite_database() {
        use crate::db::DatabaseDriver;
        use crate::db::sqlite::SqliteDriver;

        let source_dir = tempfile::tempdir().expect("source dir");
        let target_dir = tempfile::tempdir().expect("target dir");
        let source = SqliteDriver::connect(
            source_dir
                .path()
                .join("source.db")
                .to_str()
                .expect("source path"),
        )
        .await
        .expect("connect source");
        let target = SqliteDriver::connect(
            target_dir
                .path()
                .join("target.db")
                .to_str()
                .expect("target path"),
        )
        .await
        .expect("connect target");

        let create = "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)";
        source.execute(create, 1000).await.expect("source table");
        target.execute(create, 1000).await.expect("target table");
        source
            .execute(
                "INSERT INTO items VALUES (1, 'a', 1.5), (2, 'bb', 2.0), (4, 'new', NULL)",
                1000,
            )
            .await
            .expect("source rows");
        target
            .execute(
                "INSERT INTO items VALUES (1, 'a', 1.5), (2, 'b', 2.0), (3, 'old', 9.0)",
                1000,
            )
            .await
            .expect("target rows");

        let source_rows = source
            .read_rows("items", 5000, 0, None, Some("id"), Some("asc"))
            .await
            .expect("source read");
        let target_rows = target
            .read_rows("items", 5000, 0, None, Some("id"), Some("asc"))
            .await
            .expect("target read");
        let source_snapshot = RowsSnapshot {
            columns: source_rows.columns,
            rows: source_rows.rows,
            total: source_rows.total,
        };
        let target_snapshot = RowsSnapshot {
            columns: target_rows.columns,
            rows: target_rows.rows,
            total: target_rows.total,
        };

        let diff = build_data_diff(
            "src",
            "dst",
            "items",
            "items",
            "sqlite",
            &source_snapshot,
            &target_snapshot,
        )
        .expect("diff");
        assert_eq!(diff.insert_count, 1);
        assert_eq!(diff.update_count, 1);
        assert_eq!(diff.delete_count, 1);

        let applied = target
            .execute_transaction(&diff.statements)
            .await
            .expect("apply");
        assert_eq!(applied, 3);

        let after = target
            .read_rows("items", 5000, 0, None, Some("id"), Some("asc"))
            .await
            .expect("read after");
        assert_eq!(after.total, 3);
        let keys: Vec<Value> = after.rows.iter().map(|row| row[0].clone()).collect();
        assert_eq!(keys, vec![json!(1), json!(2), json!(4)]);
        assert_eq!(after.rows[1][1], json!("bb"));
    }

    #[test]
    fn requires_primary_key_on_both_sides() {
        let columns = vec![column("id", "INTEGER", false, false, false)];
        let rows = snapshot(columns.clone(), Vec::new(), 0);
        let error = build_data_diff("src", "dst", "a", "b", "sqlite", &rows, &rows)
            .expect_err("missing primary key");
        assert!(error.contains("主键"));
    }

    #[test]
    fn suppresses_deletes_when_source_is_truncated() {
        let columns = vec![
            column("id", "INTEGER", false, true, false),
            column("name", "TEXT", true, false, false),
        ];
        let source = snapshot(columns.clone(), vec![vec![json!(1), json!("a")]], 10);
        let target = snapshot(
            columns,
            vec![vec![json!(1), json!("a")], vec![json!(2), json!("b")]],
            2,
        );

        let diff = build_data_diff("src", "dst", "items", "items", "sqlite", &source, &target)
            .expect("diff");
        assert!(diff.truncated);
        assert_eq!(diff.delete_count, 0);
        assert!(diff.statements.is_empty());
        assert!(
            diff.warnings
                .iter()
                .any(|warning| warning.contains("跳过 1 条 DELETE"))
        );
    }

    #[test]
    fn skips_binary_columns_and_reports_column_differences() {
        let source_columns = vec![
            column("id", "INTEGER", false, true, false),
            column("payload", "BLOB", true, false, false),
            column("note", "TEXT", true, false, false),
        ];
        let target_columns = vec![
            column("id", "INTEGER", false, true, false),
            column("payload", "BLOB", true, false, false),
            column("extra", "TEXT", true, false, false),
        ];
        let source = snapshot(
            source_columns,
            vec![vec![json!(1), json!("<BLOB 2 bytes>"), json!("hi")]],
            1,
        );
        let target = snapshot(target_columns, Vec::new(), 0);

        let diff = build_data_diff("src", "dst", "items", "items", "sqlite", &source, &target)
            .expect("diff");
        assert_eq!(diff.compared_columns, vec!["id".to_string()]);
        assert_eq!(diff.skipped_columns, vec!["payload".to_string()]);
        assert_eq!(diff.source_only_columns, vec!["note".to_string()]);
        assert_eq!(diff.target_only_columns, vec!["extra".to_string()]);
        assert!(
            diff.statements
                .iter()
                .all(|statement| !statement.contains("payload"))
        );
        assert!(!diff.statements.is_empty());
        assert!(
            diff.warnings
                .iter()
                .any(|warning| warning.contains("BLOB") || warning.contains("二进制"))
        );
    }

    #[test]
    fn canonical_values_treat_integral_floats_as_integers() {
        assert_eq!(canonical_value(&json!(1)), canonical_value(&json!(1.0)));
        assert_eq!(canonical_value(&json!(1.5)), "1.5");
        assert_eq!(
            canonical_value(&json!({"b": 1, "a": [true, null]})),
            "{\"a\":[true,null],\"b\":1}"
        );
    }

    #[test]
    fn escapes_literals_per_driver() {
        assert_eq!(quote_string("O'Reilly", "postgres"), "'O''Reilly'");
        assert_eq!(quote_string("O'Reilly\\x", "mysql"), "'O''Reilly\\\\x'");
        assert_eq!(sql_literal(&json!(true), "sqlite"), "1");
        assert_eq!(sql_literal(&json!(true), "postgres"), "TRUE");
        assert_eq!(sql_literal(&json!({"a": 1}), "postgres"), "'{\"a\":1}'");
    }

    #[test]
    fn uses_mysql_backticks() {
        assert_eq!(quote_identifier("order", "mysql"), "`order`");
        assert_eq!(quote_identifier("order", "postgres"), "\"order\"");
    }
}
