use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const MAX_ENTRIES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub connection_id: String,
    pub connection_name: String,
    pub sql: String,
    pub executed_at: String,
    pub elapsed_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}

fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法获取配置目录: {error}"))?;
    Ok(dir.join("history.json"))
}

pub fn load(app: &AppHandle) -> Result<Vec<HistoryEntry>, String> {
    let path = history_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path).map_err(|error| format!("读取执行历史失败: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&raw).map_err(|error| format!("解析执行历史失败: {error}"))
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    let path = history_path(app)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("清空执行历史失败: {error}"))?;
    }
    Ok(())
}

pub fn record(
    app: &AppHandle,
    connection_id: &str,
    connection_name: &str,
    sql: &str,
    elapsed_ms: u64,
    success: bool,
    error: Option<String>,
) -> Result<(), String> {
    let mut entries = load(app).unwrap_or_default();
    entries.insert(
        0,
        HistoryEntry {
            id: Uuid::new_v4().to_string(),
            connection_id: connection_id.to_string(),
            connection_name: connection_name.to_string(),
            sql: sql.to_string(),
            executed_at: chrono::Utc::now().to_rfc3339(),
            elapsed_ms,
            success,
            error,
        },
    );
    entries.truncate(MAX_ENTRIES);

    let path = history_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(&entries)
        .map_err(|error| format!("序列化执行历史失败: {error}"))?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, raw).map_err(|error| format!("写入执行历史失败: {error}"))?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("替换执行历史失败: {error}"))?;
    }

    fs::rename(&temp_path, &path).map_err(|error| format!("保存执行历史失败: {error}"))?;
    Ok(())
}
