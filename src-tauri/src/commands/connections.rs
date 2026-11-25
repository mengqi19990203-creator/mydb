use super::AppState;
use super::close_connection_transactions;
use super::query_window_uses_connection;
use crate::db::{DatabaseDriver, MySqlDriver, PostgresDriver, SqliteDriver};
use crate::keychain;
use crate::models::{ConnectionConfig, ConnectionGroup, DatabaseObjects};
use crate::ssh::SshTunnel;
use crate::storage;
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[tauri::command]
pub async fn list_connections(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ConnectionConfig>, String> {
    let mut connections = storage::load_connections(&app)?;
    let passwords = state.passwords.lock().await;
    let ssh_passwords = state.ssh_passwords.lock().await;
    let ssh_key_passphrases = state.ssh_key_passphrases.lock().await;

    for config in &mut connections {
        config.has_password = config.save_password || passwords.contains_key(&config.id);
        config.has_ssh_password =
            config.save_ssh_password || ssh_passwords.contains_key(&config.id);
        config.has_ssh_key_passphrase =
            config.save_ssh_key_passphrase || ssh_key_passphrases.contains_key(&config.id);
    }

    Ok(connections)
}

#[tauri::command]
pub async fn save_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    mut config: ConnectionConfig,
    password: Option<String>,
    ssh_password: Option<String>,
    ssh_key_passphrase: Option<String>,
) -> Result<ConnectionConfig, String> {
    validate_config(&mut config)?;

    if config.id.trim().is_empty() {
        config.id = Uuid::new_v4().to_string();
    }

    let existing = storage::load_connections(&app)?
        .into_iter()
        .find(|item| item.id == config.id);

    apply_password(
        &state,
        &config.id,
        "db",
        password.as_deref(),
        config.save_password,
        existing
            .as_ref()
            .map(|item| item.save_password)
            .unwrap_or(false),
    )
    .await?;

    apply_password(
        &state,
        &config.id,
        "ssh",
        ssh_password.as_deref(),
        config.save_ssh_password,
        existing
            .as_ref()
            .map(|item| item.save_ssh_password)
            .unwrap_or(false),
    )
    .await?;

    apply_password(
        &state,
        &config.id,
        "ssh-key",
        ssh_key_passphrase.as_deref(),
        config.save_ssh_key_passphrase,
        existing
            .as_ref()
            .map(|item| item.save_ssh_key_passphrase)
            .unwrap_or(false),
    )
    .await?;

    let mut connections = storage::load_connections(&app)?;
    if let Some(current) = connections.iter_mut().find(|item| item.id == config.id) {
        *current = config.clone();
    } else {
        connections.push(config.clone());
    }
    storage::save_connections(&app, &connections)?;

    config.has_password =
        config.save_password || state.passwords.lock().await.contains_key(&config.id);
    config.has_ssh_password =
        config.save_ssh_password || state.ssh_passwords.lock().await.contains_key(&config.id);
    config.has_ssh_key_passphrase = config.save_ssh_key_passphrase
        || state
            .ssh_key_passphrases
            .lock()
            .await
            .contains_key(&config.id);

    // Drop the cached pool/tunnel/Redis connection so the next request reconnects.
    state.drivers.lock().await.remove(&config.id);
    state.redis_clients.lock().await.remove(&config.id);
    state.redis_tunnels.lock().await.remove(&config.id);
    state.redis_databases.lock().await.remove(&config.id);
    close_connection_transactions(&state, &config.id).await;

    Ok(config)
}

#[tauri::command]
pub async fn clear_connection_secret(
    state: State<'_, AppState>,
    id: String,
    kind: String,
) -> Result<(), String> {
    if !matches!(kind.as_str(), "db" | "ssh" | "ssh-key") {
        return Err("未知的凭据类型".to_string());
    }
    match kind.as_str() {
        "db" => {
            state.passwords.lock().await.remove(&id);
        }
        "ssh" => {
            state.ssh_passwords.lock().await.remove(&id);
        }
        "ssh-key" => {
            state.ssh_key_passphrases.lock().await.remove(&id);
        }
        _ => {}
    }
    keychain::delete(&id, &kind);
    Ok(())
}

#[tauri::command]
pub async fn delete_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut connections = storage::load_connections(&app)?;
    connections.retain(|item| item.id != id);
    storage::save_connections(&app, &connections)?;

    state.drivers.lock().await.remove(&id);
    state.redis_clients.lock().await.remove(&id);
    state.redis_tunnels.lock().await.remove(&id);
    state.redis_databases.lock().await.remove(&id);
    state.passwords.lock().await.remove(&id);
    state.ssh_passwords.lock().await.remove(&id);
    state.ssh_key_passphrases.lock().await.remove(&id);
    close_connection_transactions(&state, &id).await;
    keychain::delete(&id, "db");
    keychain::delete(&id, "ssh");
    keychain::delete(&id, "ssh-key");

    Ok(())
}

pub(crate) fn connection_read_only(app: &AppHandle, connection_id: &str) -> Result<bool, String> {
    Ok(storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .map(|item| item.read_only)
        .unwrap_or(false))
}

pub(crate) fn ensure_writable(app: &AppHandle, connection_id: &str) -> Result<(), String> {
    if connection_read_only(app, connection_id)? {
        Err("当前连接为只读模式，已阻止写入".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn close_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.drivers.lock().await.remove(&id);
    state.redis_clients.lock().await.remove(&id);
    state.redis_tunnels.lock().await.remove(&id);
    close_connection_transactions(&state, &id).await;
    Ok(())
}

#[tauri::command]
pub async fn set_connection_read_only(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    read_only: bool,
) -> Result<ConnectionConfig, String> {
    let mut connections = storage::load_connections(&app)?;
    let config = connections
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("连接不存在: {id}"))?;
    config.read_only = read_only;
    let saved = config.clone();
    storage::save_connections(&app, &connections)?;
    state.drivers.lock().await.remove(&id);
    state.redis_clients.lock().await.remove(&id);
    state.redis_tunnels.lock().await.remove(&id);
    close_connection_transactions(&state, &id).await;
    Ok(saved)
}

#[tauri::command]
pub async fn duplicate_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    new_name: Option<String>,
    copy_secrets: Option<bool>,
) -> Result<ConnectionConfig, String> {
    let connections = storage::load_connections(&app)?;
    let source = connections
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| format!("连接不存在: {id}"))?;

    let mut config = source.clone();
    config.id = Uuid::new_v4().to_string();
    config.name = new_name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{} 副本", source.name));
    config.has_password = false;
    config.has_ssh_password = false;
    config.has_ssh_key_passphrase = false;
    if connections.iter().any(|item| item.name == config.name) {
        return Err("连接名称已存在".to_string());
    }

    if copy_secrets.unwrap_or(false) {
        copy_secret(&state, &source, &config.id, "db").await?;
        copy_secret(&state, &source, &config.id, "ssh").await?;
        copy_secret(&state, &source, &config.id, "ssh-key").await?;
        let session_db = state.passwords.lock().await.get(&source.id).cloned();
        let session_ssh = state.ssh_passwords.lock().await.get(&source.id).cloned();
        let session_key = state
            .ssh_key_passphrases
            .lock()
            .await
            .get(&source.id)
            .cloned();
        if let Some(password) = session_db {
            state
                .passwords
                .lock()
                .await
                .insert(config.id.clone(), password);
        }
        if let Some(password) = session_ssh {
            state
                .ssh_passwords
                .lock()
                .await
                .insert(config.id.clone(), password);
        }
        if let Some(passphrase) = session_key {
            state
                .ssh_key_passphrases
                .lock()
                .await
                .insert(config.id.clone(), passphrase);
        }
        config.save_password =
            source.save_password && keychain::load(&source.id, "db").ok().flatten().is_some();
        config.save_ssh_password =
            source.save_ssh_password && keychain::load(&source.id, "ssh").ok().flatten().is_some();
        config.save_ssh_key_passphrase = source.save_ssh_key_passphrase
            && keychain::load(&source.id, "ssh-key")
                .ok()
                .flatten()
                .is_some();
    } else {
        config.save_password = false;
        config.save_ssh_password = false;
        config.save_ssh_key_passphrase = false;
    }

    let mut connections = storage::load_connections(&app)?;
    connections.push(config.clone());
    storage::save_connections(&app, &connections)?;
    Ok(config)
}

async fn copy_secret(
    state: &AppState,
    source: &ConnectionConfig,
    target_id: &str,
    kind: &str,
) -> Result<(), String> {
    let saved = keychain::load(&source.id, kind).ok().flatten();
    let session = match kind {
        "db" => state.passwords.lock().await.get(&source.id).cloned(),
        "ssh" => state.ssh_passwords.lock().await.get(&source.id).cloned(),
        "ssh-key" => state
            .ssh_key_passphrases
            .lock()
            .await
            .get(&source.id)
            .cloned(),
        _ => None,
    };
    if let Some(password) = saved.or(session) {
        match kind {
            "db" => {
                state
                    .passwords
                    .lock()
                    .await
                    .insert(target_id.to_string(), password.clone());
            }
            "ssh" => {
                state
                    .ssh_passwords
                    .lock()
                    .await
                    .insert(target_id.to_string(), password.clone());
            }
            "ssh-key" => {
                state
                    .ssh_key_passphrases
                    .lock()
                    .await
                    .insert(target_id.to_string(), password.clone());
            }
            _ => {}
        }
        if let Some(stored) = keychain::load(&source.id, kind).ok().flatten() {
            if stored == password {
                let _ = keychain::store(target_id, kind, &password);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionInfoFormat {
    HostPort,
    Url,
    UrlWithPassword,
    Jdbc,
    Dsn,
    Cli,
}

#[tauri::command]
pub async fn connection_info(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    format: ConnectionInfoFormat,
    _include_password: Option<bool>,
) -> Result<String, String> {
    let connections = storage::load_connections(&app)?;
    let config = connections
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| format!("连接不存在: {id}"))?;

    let with_password = matches!(format, ConnectionInfoFormat::UrlWithPassword);
    let password = if with_password {
        Some(
            super::password_for(&state, &config, "db")
                .await?
                .ok_or_else(|| "没有可用的密码，无法生成含密码的连接信息".to_string())?,
        )
    } else {
        None
    };

    let user = encode_component(&config.username);
    let host = config.host.clone();
    let port = config.port;
    let database = if config.database.is_empty() {
        match config.driver.as_str() {
            "mysql" => "mysql".to_string(),
            "postgres" => "postgres".to_string(),
            other => other.to_string(),
        }
    } else {
        config.database.clone()
    };
    let dsn_database = database.clone();
    let file = config.file.clone();

    let mut text = match format {
        ConnectionInfoFormat::HostPort => match config.driver.as_str() {
            "sqlite" => file.clone(),
            "redis" => format!("{}:{}/{}", host, port, database),
            _ => format!("{}@{}:{}/{}", user, host, port, database),
        },
        ConnectionInfoFormat::Url => match config.driver.as_str() {
            "sqlite" => format!("sqlite:///{}", file.trim_start_matches('/')),
            "redis" => format!("redis://{}:{}/{}", host, port, database),
            "postgres" => format!("postgres://{}@{}:{}/{}", user, host, port, database),
            _ => format!("mysql://{}@{}:{}/{}", user, host, port, database),
        },
        ConnectionInfoFormat::UrlWithPassword => {
            let password = encode_component(password.as_deref().unwrap_or(""));
            match config.driver.as_str() {
                "sqlite" => return Err("SQLite 不支持含密码的连接信息".to_string()),
                "redis" => format!(
                    "{}://:{}@{}:{}/{}",
                    if config.ssl_mode == "required" {
                        "rediss"
                    } else {
                        "redis"
                    },
                    password,
                    host,
                    port,
                    database
                ),
                "postgres" => format!(
                    "postgres://{}:{}@{}:{}/{}",
                    user, password, host, port, database
                ),
                _ => format!(
                    "mysql://{}:{}@{}:{}/{}",
                    user, password, host, port, database
                ),
            }
        }
        ConnectionInfoFormat::Jdbc => match config.driver.as_str() {
            "sqlite" => format!("jdbc:sqlite:{}", file),
            "postgres" => format!("jdbc:postgresql://{}:{}/{}", host, port, database),
            "redis" => return Err("Redis 不支持 JDBC 连接串".to_string()),
            _ => format!("jdbc:mysql://{}:{}/{}", host, port, database),
        },
        ConnectionInfoFormat::Dsn => match config.driver.as_str() {
            "sqlite" => format!("Data Source={}", file),
            "postgres" => format!(
                "host={} port={} user={} dbname={} sslmode={}",
                host, port, config.username, dsn_database, config.ssl_mode
            ),
            "redis" => return Err("Redis 不支持 DSN 连接串".to_string()),
            _ => format!(
                "host={} port={} user={} dbname={}",
                host, port, config.username, dsn_database
            ),
        },
        ConnectionInfoFormat::Cli => match config.driver.as_str() {
            "sqlite" => format!("sqlite3 {}", file),
            "postgres" => format!(
                "psql \"host={} port={} user={} dbname={}\"",
                host, port, config.username, dsn_database
            ),
            "redis" => format!(
                "redis-cli{} -h {} -p {} -n {}",
                if config.ssl_mode == "required" {
                    " --tls"
                } else {
                    ""
                },
                host,
                port,
                database
            ),
            _ => format!(
                "mysql -h {} -P {} -u {} -p {}",
                host, port, config.username, dsn_database
            ),
        },
    };

    if config.ssh_enabled {
        text.push_str(&format!(
            "\n# via ssh {}@{}:{}",
            config.ssh_username, config.ssh_host, config.ssh_port
        ));
    }
    Ok(text)
}

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[tauri::command]
pub async fn list_connection_groups(app: AppHandle) -> Result<Vec<ConnectionGroup>, String> {
    storage::load_groups(&app)
}

#[tauri::command]
pub async fn save_connection_group(
    app: AppHandle,
    group: ConnectionGroup,
) -> Result<Vec<ConnectionGroup>, String> {
    let name = group.name.trim().to_string();
    if name.is_empty() {
        return Err("分组名不能为空".to_string());
    }
    let mut groups = storage::load_groups(&app)?;
    match groups.iter_mut().find(|item| item.name == name) {
        Some(existing) => {
            if !group.color.trim().is_empty() {
                existing.color = group.color;
            }
            existing.order = group.order;
            existing.collapsed = group.collapsed;
        }
        None => groups.push(ConnectionGroup {
            name,
            color: if group.color.trim().is_empty() {
                "#58a6ff".to_string()
            } else {
                group.color
            },
            order: group.order,
            collapsed: group.collapsed,
        }),
    }
    storage::save_groups(&app, &groups)?;
    Ok(groups)
}

#[tauri::command]
pub async fn rename_connection_group(
    app: AppHandle,
    old_name: String,
    new_name: String,
) -> Result<Vec<ConnectionGroup>, String> {
    let old_name = old_name.trim().to_string();
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("分组名不能为空".to_string());
    }
    if old_name == new_name {
        return storage::load_groups(&app);
    }
    let mut groups = storage::load_groups(&app)?;
    if groups.iter().any(|item| item.name == new_name) {
        return Err("分组名已存在".to_string());
    }
    if let Some(group) = groups.iter_mut().find(|item| item.name == old_name) {
        group.name = new_name.clone();
    }
    let mut connections = storage::load_connections(&app)?;
    for config in &mut connections {
        if config.group == old_name {
            config.group = new_name.clone();
        }
    }
    storage::save_connections(&app, &connections)?;
    storage::save_groups(&app, &groups)?;
    Ok(groups)
}

#[tauri::command]
pub async fn delete_connection_group(
    app: AppHandle,
    name: String,
) -> Result<Vec<ConnectionGroup>, String> {
    let mut groups = storage::load_groups(&app)?;
    groups.retain(|item| item.name != name);
    let mut connections = storage::load_connections(&app)?;
    for config in &mut connections {
        if config.group == name {
            config.group.clear();
        }
    }
    storage::save_connections(&app, &connections)?;
    storage::save_groups(&app, &groups)?;
    Ok(groups)
}

#[tauri::command]
pub async fn move_connection_to_group(
    app: AppHandle,
    id: String,
    group: Option<String>,
) -> Result<(), String> {
    let mut connections = storage::load_connections(&app)?;
    let config = connections
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("连接不存在: {id}"))?;
    config.group = group.unwrap_or_default().trim().to_string();
    storage::save_connections(&app, &connections)?;
    Ok(())
}

#[tauri::command]
pub async fn list_databases(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.list_databases().await
}

#[tauri::command]
pub async fn switch_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<(), String> {
    let database = database.trim().to_string();
    if database.is_empty() {
        return Err("数据库名不能为空".to_string());
    }

    let connections = storage::load_connections(&app)?;
    let mut config = connections
        .iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))?
        .clone();

    let driver = driver_for(&app, &state, &connection_id).await?;
    let (current_database, _) = driver.context().await;
    if current_database.as_deref() != Some(database.as_str())
        && query_window_uses_connection(&state, &connection_id)
    {
        return Err("独立查询窗口正在使用该连接，请先关闭查询窗口再切换数据库 / 模式".to_string());
    }

    config.database = database.clone();
    // MySQL keeps the existing pool and only changes the active schema.
    if config.driver == "mysql" {
        return driver.switch_database(&database).await;
    }

    let password = password_for(&state, &config, "db").await?;
    let (host, port, tunnel) = connection_target(&state, &config, None, None).await?;

    let driver: Arc<dyn DatabaseDriver> = match config.driver.as_str() {
        "postgres" => {
            let driver =
                PostgresDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?;
            driver.test_connection().await?;
            Arc::new(driver)
        }
        _ => return Err("当前连接不支持切换数据库".to_string()),
    };

    state.drivers.lock().await.insert(connection_id, driver);

    Ok(())
}

#[tauri::command]
pub async fn list_database_objects(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<DatabaseObjects>, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.list_database_objects().await
}

#[tauri::command]
pub async fn list_schemas(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    driver.list_schemas().await
}

#[tauri::command]
pub async fn switch_schema(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
) -> Result<(), String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    let (_, current_schema) = driver.context().await;
    if current_schema.as_deref() != Some(schema.as_str())
        && query_window_uses_connection(&state, &connection_id)
    {
        return Err("独立查询窗口正在使用该连接，请先关闭查询窗口再切换数据库 / 模式".to_string());
    }
    driver.switch_schema(&schema).await
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    mut config: ConnectionConfig,
    password: Option<String>,
    ssh_password: Option<String>,
    ssh_key_passphrase: Option<String>,
) -> Result<(), String> {
    validate_config(&mut config)?;

    if config.driver == "sqlite" {
        if !std::path::Path::new(&config.file).exists() {
            return Err("数据库文件不存在".to_string());
        }
        let driver = SqliteDriver::connect(&config.file).await?;
        return driver.test_connection().await;
    }

    if config.driver == "redis" {
        let password = match password {
            Some(value) if !value.is_empty() => Some(value),
            _ => password_for(&state, &config, "db").await?,
        };
        let (host, port, tunnel) =
            connection_target(&state, &config, ssh_password, ssh_key_passphrase).await?;
        let result =
            crate::redis_client::test_connection(&config, &host, port, password.as_deref()).await;
        drop(tunnel);
        return result;
    }

    let password = match password {
        Some(value) if !value.is_empty() => Some(value),
        _ => password_for(&state, &config, "db").await?,
    };
    let (host, port, tunnel) =
        connection_target(&state, &config, ssh_password, ssh_key_passphrase).await?;

    if config.driver == "postgres" {
        let driver =
            PostgresDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?;
        return driver.test_connection().await;
    }

    let driver = MySqlDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?;
    driver.test_connection().await
}

pub(crate) async fn driver_for(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
) -> Result<Arc<dyn DatabaseDriver>, String> {
    {
        let drivers = state.drivers.lock().await;
        if let Some(driver) = drivers.get(connection_id) {
            return Ok(driver.clone());
        }
    }

    let connections = storage::load_connections(app)?;
    let config = connections
        .iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))?
        .clone();

    let driver: Arc<dyn DatabaseDriver> = match config.driver.as_str() {
        "sqlite" => Arc::new(SqliteDriver::connect(&config.file).await?),
        "mysql" => {
            let password = password_for(state, &config, "db").await?;
            let (host, port, tunnel) = connection_target(state, &config, None, None).await?;
            Arc::new(MySqlDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?)
        }
        "postgres" => {
            let password = password_for(state, &config, "db").await?;
            let (host, port, tunnel) = connection_target(state, &config, None, None).await?;
            Arc::new(
                PostgresDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?,
            )
        }
        other => return Err(format!("暂不支持的数据库类型: {other}")),
    };

    state
        .drivers
        .lock()
        .await
        .insert(connection_id.to_string(), driver.clone());
    Ok(driver)
}

fn validate_config(config: &mut ConnectionConfig) -> Result<(), String> {
    config.name = config.name.trim().to_string();
    config.group = config.group.trim().to_string();
    config.file = config.file.trim().to_string();
    config.host = config.host.trim().to_string();
    config.username = config.username.trim().to_string();
    config.database = config.database.trim().to_string();
    config.ssh_host = config.ssh_host.trim().to_string();
    config.ssh_username = config.ssh_username.trim().to_string();
    config.ssh_key_path = config.ssh_key_path.trim().to_string();
    config.ssl_ca_path = config.ssl_ca_path.trim().to_string();
    config.ssl_client_cert_path = config.ssl_client_cert_path.trim().to_string();
    config.ssl_client_key_path = config.ssl_client_key_path.trim().to_string();
    for path in [
        &mut config.ssl_ca_path,
        &mut config.ssl_client_cert_path,
        &mut config.ssl_client_key_path,
    ] {
        if let Some(relative) = path.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                *path = std::path::PathBuf::from(home)
                    .join(relative)
                    .to_string_lossy()
                    .to_string();
            }
        }
    }
    if matches!(config.ssl_mode.as_str(), "verify-ca" | "verify-full")
        && config.ssl_ca_path.is_empty()
    {
        return Err("verify-ca / verify-full 必须选择 CA 证书文件".to_string());
    }
    if config.ssl_client_cert_path.is_empty() != config.ssl_client_key_path.is_empty() {
        return Err("客户端证书与私钥必须同时选择".to_string());
    }

    if let Some(relative) = config.ssh_key_path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            config.ssh_key_path = std::path::PathBuf::from(home)
                .join(relative)
                .to_string_lossy()
                .to_string();
        }
    }

    if config.name.is_empty() {
        return Err("连接名称不能为空".to_string());
    }
    if config.driver.trim().is_empty() {
        config.driver = "sqlite".to_string();
    }
    if config.color.trim().is_empty() {
        config.color = "#58a6ff".to_string();
    }

    match config.driver.as_str() {
        "sqlite" => {
            if config.file.is_empty() {
                return Err("SQLite 文件路径不能为空".to_string());
            }
        }
        "mysql" => {
            if config.host.is_empty() {
                return Err("主机不能为空".to_string());
            }
            if config.username.is_empty() {
                return Err("用户名不能为空".to_string());
            }
            if config.database.is_empty() {
                config.database = "mysql".to_string();
            }
            if config.port == 0 {
                config.port = 3306;
            }
            if config.ssl_mode.is_empty() {
                config.ssl_mode = "disabled".to_string();
            }
        }
        "postgres" => {
            if config.host.is_empty() {
                return Err("主机不能为空".to_string());
            }
            if config.username.is_empty() {
                return Err("用户名不能为空".to_string());
            }
            if config.database.is_empty() {
                config.database = "postgres".to_string();
            }
            if config.port == 0 {
                config.port = 5432;
            }
            if config.ssl_mode.is_empty() {
                config.ssl_mode = "disabled".to_string();
            }
        }
        "redis" => {
            if config.host.is_empty() {
                return Err("主机不能为空".to_string());
            }
            if config.port == 0 {
                config.port = 6379;
            }
            config.database = config.database.trim().to_string();
            if config.database.is_empty() {
                config.database = "0".to_string();
            } else {
                let db = config
                    .database
                    .parse::<i64>()
                    .map_err(|_| "Redis 数据库编号必须是整数".to_string())?;
                if db < 0 {
                    return Err("Redis 数据库编号不能为负数".to_string());
                }
                config.database = db.to_string();
            }
            config.ssl_mode = match config.ssl_mode.trim().to_ascii_lowercase().as_str() {
                "required" | "preferred" => "required".to_string(),
                _ => "disabled".to_string(),
            };
        }
        other => return Err(format!("暂不支持的数据库类型: {other}")),
    }

    if config.ssh_enabled {
        if config.ssh_host.is_empty() {
            return Err("SSH 主机不能为空".to_string());
        }
        if config.ssh_username.is_empty() {
            return Err("SSH 用户名不能为空".to_string());
        }
        if config.ssh_port == 0 {
            config.ssh_port = 22;
        }
        if config.ssh_auth == "key" && config.ssh_key_path.is_empty() {
            return Err("SSH 私钥路径不能为空".to_string());
        }
    }

    Ok(())
}

pub(crate) async fn connection_target(
    state: &AppState,
    config: &ConnectionConfig,
    ssh_password_override: Option<String>,
    ssh_key_passphrase_override: Option<String>,
) -> Result<(String, u16, Option<SshTunnel>), String> {
    if !config.ssh_enabled {
        return Ok((config.host.clone(), config.port, None));
    }

    let credential = if config.ssh_auth == "key" {
        match ssh_key_passphrase_override {
            Some(value) if !value.is_empty() => Some(value),
            _ => password_for(state, config, "ssh-key").await?,
        }
    } else {
        match ssh_password_override {
            Some(value) if !value.is_empty() => Some(value),
            _ => password_for(state, config, "ssh").await?,
        }
    };
    let tunnel = SshTunnel::open(config, &config.host, config.port, credential.as_deref()).await?;
    let local_port = tunnel.local_port;

    Ok(("127.0.0.1".to_string(), local_port, Some(tunnel)))
}

pub(crate) async fn password_for(
    state: &AppState,
    config: &ConnectionConfig,
    kind: &str,
) -> Result<Option<String>, String> {
    let session_password = match kind {
        "db" => state.passwords.lock().await.get(&config.id).cloned(),
        "ssh" => state.ssh_passwords.lock().await.get(&config.id).cloned(),
        "ssh-key" => state
            .ssh_key_passphrases
            .lock()
            .await
            .get(&config.id)
            .cloned(),
        _ => None,
    };
    if session_password.is_some() {
        return Ok(session_password);
    }

    let save_flag = match kind {
        "db" => config.save_password,
        "ssh" => config.save_ssh_password,
        "ssh-key" => config.save_ssh_key_passphrase,
        _ => false,
    };
    if save_flag && !config.id.is_empty() {
        return keychain::load(&config.id, kind);
    }

    Ok(None)
}

async fn apply_password(
    state: &AppState,
    connection_id: &str,
    kind: &str,
    password: Option<&str>,
    save_password: bool,
    was_saved: bool,
) -> Result<(), String> {
    if let Some(value) = password {
        if !value.is_empty() {
            match kind {
                "db" => {
                    state
                        .passwords
                        .lock()
                        .await
                        .insert(connection_id.to_string(), value.to_string());
                }
                "ssh" => {
                    state
                        .ssh_passwords
                        .lock()
                        .await
                        .insert(connection_id.to_string(), value.to_string());
                }
                "ssh-key" => {
                    state
                        .ssh_key_passphrases
                        .lock()
                        .await
                        .insert(connection_id.to_string(), value.to_string());
                }
                _ => {}
            }

            if save_password {
                keychain::store(connection_id, kind, value)?;
            } else {
                keychain::delete(connection_id, kind);
            }
            return Ok(());
        }
    }

    if !save_password {
        keychain::delete(connection_id, kind);
    } else if !was_saved {
        // The user kept "save password" checked while editing and left the field blank.
        // Keep whatever is already in the keychain.
    }

    Ok(())
}
