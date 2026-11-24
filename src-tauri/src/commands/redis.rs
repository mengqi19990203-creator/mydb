use super::{AppState, connection_read_only};
use crate::redis_client::{
    self, RedisCommandResult, RedisDatabases, RedisKeyDetail, RedisScanResult,
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
    let ssh_password = super::password_for(state, config, "ssh").await?;
    let (host, port, tunnel) = super::connection_target(state, config, ssh_password).await?;
    let desired = match state
        .redis_databases
        .lock()
        .await
        .get(connection_id)
        .copied()
    {
        Some(database) => database,
        None => redis_client::database_index(config)?,
    };
    let mut connection = redis_client::connect(config, &host, port, password.as_deref()).await?;
    let database = redis_client::ensure_database(&mut connection, desired).await?;
    if let Some(tunnel) = tunnel {
        state
            .redis_tunnels
            .lock()
            .await
            .insert(connection_id.to_string(), tunnel);
    }
    state
        .redis_clients
        .lock()
        .await
        .insert(connection_id.to_string(), connection.clone());
    state
        .redis_databases
        .lock()
        .await
        .insert(connection_id.to_string(), database);
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
pub async fn redis_databases(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<RedisDatabases, String> {
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    redis_client::list_databases(&mut connection).await
}

#[tauri::command]
pub async fn redis_select_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: i64,
) -> Result<i64, String> {
    let mut connection = redis_connection(&app, &state, &connection_id).await?;
    let selected = redis_client::switch_database(&mut connection, database).await?;
    state
        .redis_databases
        .lock()
        .await
        .insert(connection_id, selected);
    Ok(selected)
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
