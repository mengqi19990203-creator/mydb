use super::{AppState, driver_for};
use crate::models::{DatabaseDocsSnapshot, DocsFailure, DocsTable};
use tauri::{AppHandle, State};

const DOC_TABLE_LIMIT: usize = 500;

#[tauri::command]
pub async fn database_docs_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<DatabaseDocsSnapshot, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    let all = driver.list_tables().await?;
    let mut objects: Vec<_> = all
        .into_iter()
        .filter(|table| matches!(table.kind.as_str(), "table" | "view" | "matview"))
        .collect();
    objects.sort_by(|left, right| left.name.cmp(&right.name));
    let truncated = objects.len() > DOC_TABLE_LIMIT;
    objects.truncate(DOC_TABLE_LIMIT);

    let mut tables = Vec::new();
    let mut failures = Vec::new();
    for object in objects {
        match driver.table_structure(&object.name).await {
            Ok(structure) => tables.push(DocsTable {
                name: object.name,
                kind: object.kind,
                structure,
            }),
            Err(error) => failures.push(DocsFailure {
                name: object.name,
                error,
            }),
        }
    }

    Ok(DatabaseDocsSnapshot {
        tables,
        failures,
        truncated,
    })
}
