use super::{AppState, driver_for};
use crate::models::{QueryResult, ScopeSearchHit};
use crate::storage;
use serde_json::Value;
use tauri::{AppHandle, State};

const LIMIT: usize = 200;

fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn cell(result: &QueryResult, row: usize, column: usize) -> String {
    result
        .rows
        .get(row)
        .and_then(|values| values.get(column))
        .map(text)
        .unwrap_or_default()
}

fn escape(pattern: &str) -> String {
    pattern.replace('\'', "''")
}

#[tauri::command]
pub async fn search_scope_objects(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    query: String,
) -> Result<Vec<ScopeSearchHit>, String> {
    let query = query.trim();
    if query.len() < 2 {
        return Ok(Vec::new());
    }
    let config = storage::load_connections(&app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持对象搜索".to_string());
    }
    let driver = driver_for(&app, &state, &connection_id).await?;
    let pattern = escape(query);
    let mut hits = Vec::new();

    if config.driver == "mysql" {
        let objects = driver
            .execute(
                &format!(
                    "SELECT table_schema, table_name, table_type \
                     FROM information_schema.tables \
                     WHERE table_name LIKE '%{pattern}%' \
                       AND table_schema NOT IN ('information_schema','mysql','performance_schema','sys') \
                     ORDER BY table_schema, table_name LIMIT 120"
                ),
                120,
            )
            .await?;
        for row in 0..objects.rows.len() {
            let raw_kind = cell(&objects, row, 2);
            hits.push(ScopeSearchHit {
                database: Some(cell(&objects, row, 0)),
                schema: None,
                name: cell(&objects, row, 1),
                kind: if raw_kind.eq_ignore_ascii_case("VIEW") {
                    "view".to_string()
                } else {
                    "table".to_string()
                },
                column: None,
                owner: None,
            });
        }
        let routines = driver
            .execute(
                &format!(
                    "SELECT routine_schema, routine_name, routine_type \
                     FROM information_schema.routines \
                     WHERE routine_name LIKE '%{pattern}%' \
                       AND routine_schema NOT IN ('information_schema','mysql','performance_schema','sys') \
                     LIMIT 60"
                ),
                60,
            )
            .await?;
        for row in 0..routines.rows.len() {
            hits.push(ScopeSearchHit {
                database: Some(cell(&routines, row, 0)),
                schema: None,
                name: cell(&routines, row, 1),
                kind: if cell(&routines, row, 2).eq_ignore_ascii_case("PROCEDURE") {
                    "procedure".to_string()
                } else {
                    "function".to_string()
                },
                column: None,
                owner: None,
            });
        }
        let columns = driver
            .execute(
                &format!(
                    "SELECT table_schema, table_name, column_name \
                     FROM information_schema.columns \
                     WHERE column_name LIKE '%{pattern}%' \
                       AND table_schema NOT IN ('information_schema','mysql','performance_schema','sys') \
                     ORDER BY table_schema, table_name, ordinal_position LIMIT 120"
                ),
                120,
            )
            .await?;
        for row in 0..columns.rows.len() {
            hits.push(ScopeSearchHit {
                database: Some(cell(&columns, row, 0)),
                schema: None,
                name: cell(&columns, row, 2),
                kind: "column".to_string(),
                column: Some(cell(&columns, row, 2)),
                owner: Some(cell(&columns, row, 1)),
            });
        }
    } else if config.driver == "postgres" {
        let objects = driver
            .execute(
                &format!(
                    "SELECT n.nspname, c.relname, c.relkind::text \
                     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE c.relname ILIKE '%{pattern}%' \
                       AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema' \
                     ORDER BY n.nspname, c.relname LIMIT 120"
                ),
                120,
            )
            .await?;
        for row in 0..objects.rows.len() {
            let kind = match cell(&objects, row, 2).as_str() {
                "v" => "view",
                "m" => "matview",
                "S" => "sequence",
                _ => "table",
            };
            hits.push(ScopeSearchHit {
                database: None,
                schema: Some(cell(&objects, row, 0)),
                name: cell(&objects, row, 1),
                kind: kind.to_string(),
                column: None,
                owner: None,
            });
        }
        let routines = driver
            .execute(
                &format!(
                    "SELECT n.nspname, p.proname, p.prokind::text \
                     FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                     WHERE p.proname ILIKE '%{pattern}%' \
                       AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema' \
                     LIMIT 60"
                ),
                60,
            )
            .await?;
        for row in 0..routines.rows.len() {
            hits.push(ScopeSearchHit {
                database: None,
                schema: Some(cell(&routines, row, 0)),
                name: cell(&routines, row, 1),
                kind: if cell(&routines, row, 2) == "p" {
                    "procedure".to_string()
                } else {
                    "function".to_string()
                },
                column: None,
                owner: None,
            });
        }
        let columns = driver
            .execute(
                &format!(
                    "SELECT n.nspname, c.relname, a.attname \
                     FROM pg_attribute a \
                     JOIN pg_class c ON c.oid = a.attrelid \
                     JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE a.attname ILIKE '%{pattern}%' AND a.attnum > 0 AND NOT a.attisdropped \
                       AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema' \
                     ORDER BY n.nspname, c.relname, a.attnum LIMIT 120"
                ),
                120,
            )
            .await?;
        for row in 0..columns.rows.len() {
            hits.push(ScopeSearchHit {
                database: None,
                schema: Some(cell(&columns, row, 0)),
                name: cell(&columns, row, 2),
                kind: "column".to_string(),
                column: Some(cell(&columns, row, 2)),
                owner: Some(cell(&columns, row, 1)),
            });
        }
    } else {
        let tables = driver.list_tables().await?;
        for table in tables
            .iter()
            .filter(|table| table.name.to_lowercase().contains(&query.to_lowercase()))
            .take(60)
        {
            hits.push(ScopeSearchHit {
                database: None,
                schema: None,
                name: table.name.clone(),
                kind: table.kind.clone(),
                column: None,
                owner: None,
            });
        }
        for table in tables
            .iter()
            .filter(|table| matches!(table.kind.as_str(), "table" | "view"))
            .take(40)
        {
            let Ok(columns) = driver.list_columns(&table.name).await else {
                continue;
            };
            for column in columns
                .into_iter()
                .filter(|column| column.name.to_lowercase().contains(&query.to_lowercase()))
            {
                hits.push(ScopeSearchHit {
                    database: None,
                    schema: None,
                    name: column.name.clone(),
                    kind: "column".to_string(),
                    column: Some(column.name),
                    owner: Some(table.name.clone()),
                });
            }
        }
    }

    hits.truncate(LIMIT);
    Ok(hits)
}
