use crate::models::{ConnectionConfig, ConnectionGroup};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法获取配置目录: {error}"))
}

pub fn connections_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("connections.json"))
}

pub fn load_connections(app: &AppHandle) -> Result<Vec<ConnectionConfig>, String> {
    let path = connections_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path).map_err(|error| format!("读取连接配置失败: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&raw).map_err(|error| format!("解析连接配置失败: {error}"))
}

pub fn save_connections(app: &AppHandle, connections: &[ConnectionConfig]) -> Result<(), String> {
    let path = connections_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败: {error}"))?;
    }

    let raw = serde_json::to_string_pretty(connections)
        .map_err(|error| format!("序列化连接配置失败: {error}"))?;
    let temp_path = path.with_extension("json.tmp");

    fs::write(&temp_path, raw).map_err(|error| format!("写入连接配置失败: {error}"))?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("替换连接配置失败: {error}"))?;
    }

    fs::rename(&temp_path, &path).map_err(|error| format!("保存连接配置失败: {error}"))?;
    Ok(())
}

pub fn groups_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("groups.json"))
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct GroupsFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    groups: Vec<ConnectionGroup>,
}

pub fn load_groups(app: &AppHandle) -> Result<Vec<ConnectionGroup>, String> {
    let path = groups_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path).map_err(|error| format!("读取分组配置失败: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let file: GroupsFile =
        serde_json::from_str(&raw).map_err(|error| format!("解析分组配置失败: {error}"))?;
    Ok(file.groups)
}

pub fn save_groups(app: &AppHandle, groups: &[ConnectionGroup]) -> Result<(), String> {
    let path = groups_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败: {error}"))?;
    }

    let file = GroupsFile {
        version: 1,
        groups: groups.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&file)
        .map_err(|error| format!("序列化分组配置失败: {error}"))?;
    let temp_path = path.with_extension("json.tmp");

    fs::write(&temp_path, raw).map_err(|error| format!("写入分组配置失败: {error}"))?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("替换分组配置失败: {error}"))?;
    }

    fs::rename(&temp_path, &path).map_err(|error| format!("保存分组配置失败: {error}"))?;
    Ok(())
}
