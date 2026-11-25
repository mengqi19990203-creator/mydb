use super::{AppState, driver_for, ensure_writable};
use crate::storage;
use tauri::{AppHandle, State};

fn validate_name(name: &str, driver: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if name.contains(';') || name.contains('\0') {
        return Err("名称不能包含分号或空字符".to_string());
    }
    let limit = if driver == "postgres" { 63 } else { 64 };
    if name.chars().count() > limit {
        return Err(format!("名称不能超过 {limit} 个字符"));
    }
    Ok(name.to_string())
}

fn driver_kind(app: &AppHandle, connection_id: &str) -> Result<String, String> {
    storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .map(|item| item.driver)
        .ok_or_else(|| "连接不存在".to_string())
}

#[tauri::command]
pub async fn create_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
    charset: Option<String>,
    collation: Option<String>,
) -> Result<(), String> {
    let kind = driver_kind(&app, &connection_id)?;
    if kind != "mysql" && kind != "postgres" {
        return Err("当前驱动不支持创建数据库".to_string());
    }
    let name = validate_name(&name, &kind)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver
        .create_database(&name, charset.as_deref(), collation.as_deref())
        .await
}

#[tauri::command]
pub async fn create_schema(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
) -> Result<(), String> {
    let kind = driver_kind(&app, &connection_id)?;
    if kind != "postgres" {
        return Err("当前驱动不支持创建模式".to_string());
    }
    let name = validate_name(&name, &kind)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.create_schema(&name).await
}

#[tauri::command]
pub async fn drop_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
) -> Result<(), String> {
    let kind = driver_kind(&app, &connection_id)?;
    if kind != "mysql" && kind != "postgres" {
        return Err("当前驱动不支持删除数据库".to_string());
    }
    let name = validate_name(&name, &kind)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.drop_database(&name).await
}

#[tauri::command]
pub async fn drop_schema(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
) -> Result<(), String> {
    let kind = driver_kind(&app, &connection_id)?;
    if kind != "postgres" {
        return Err("当前驱动不支持删除模式".to_string());
    }
    let name = validate_name(&name, &kind)?;
    ensure_writable(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.drop_schema(&name).await
}
