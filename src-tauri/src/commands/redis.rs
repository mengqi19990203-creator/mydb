use super::{AppState, connection_read_only};
use crate::redis_client::{
    self, RedisCommandResult, RedisKeyDetail, RedisScanResult,
};
use crate::storage;
use ::redis::aio::MultiplexedConnection;
use tauri::{AppHandle, State};

const READ_COMMANDS: &[&str] = &[
    "GET", "MGET", "STRLEN", "EXISTS", "TYPE", "TTL", "PTTL", "KEYS", "SCAN", "HGET",
    "HMGET", "HGETALL", "HKEYS", "HVALS", "HLEN", "HEXISTS", "LRANGE", "LLEN", "LINDEX",
    "SMEMBERS", "SCARD", "SISMEMBER", "SRANDMEMBER", "ZRANGE", "ZREVRANGE",
    "ZRANGEBYSCORE", "ZCARD", "ZSCORE", "ZRANK", "ZCOUNT", "XLEN", "XRANGE",
    "XREVRANGE", "XINFO", "PING", "ECHO", "DBSIZE", "INFO", "TIME", "COMMAND",
    "MEMORY", "OBJECT",
];

fn ensure_read_only_redis(command: &str) -> Result<(), String> {
    let first = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_uppercase();
    if READ_COMMANDS.contains(&first.as_str()) {
        Ok(())
    } else {
        Err(format!("只读模式不允许 Redis 写命令：{first}"))
    }
}

async fn redis_connection(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
) -> Result<MultiplexedConnection, String> {
    {
        let clients = state.redis_clients.lock().await;
        if let Some(connection) = clients.get(connection_id) {
            return Ok(connection.clone());
        }
    }

    let connections = storage::load_connections(app)?;
    let config = connections
        .iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))?;

    if config.driver != "redis" {
        return Err("当前连接不是 Redis".to_string());
    }

    let password = super::password_for(state, config, "db").await?;
    let connection = redis_client::connect(config, password.as_deref()).await?;
    state
        .redis_clients
        .lock()
        .await
        .insert(connection_id.to_string(), connection.clone());
    Ok(connection)
}

#[tauri::command]
pub async fn redis_ping(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::ping(&mut connection).await
}

#[tauri::command]
pub async fn redis_scan_keys(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<RedisScanResult, String> {
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::scan_keys(&mut connection, pattern.as_deref(), limit).await
}

#[tauri::command]
pub async fn redis_key_detail(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
) -> Result<RedisKeyDetail, String> {
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::key_detail(&mut connection, &key).await
}

#[tauri::command]
pub async fn redis_delete_key(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
) -> Result<u64, String> {
    if connection_read_only(&app, &connection_id)? {
        return Err("当前连接为只读模式，已阻止写入".to_string());
    }
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::delete_key(&mut connection, &key).await
}

#[tauri::command]
pub async fn redis_command(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    command: String,
) -> Result<RedisCommandResult, String> {
    if connection_read_only(&app, &connection_id)? {
        ensure_read_only_redis(&command)?;
    }
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::run_command(&mut connection, &command).await
}
