use super::{AppState, connection_target, driver_for, password_for};
use crate::db::{DatabaseDriver, MySqlDriver, PostgresDriver, SqliteDriver};
use crate::models::{ConnectionConfig, TableInfo};
use crate::storage;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionContext {
    pub database: Option<String>,
    pub schema: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ResolvedTarget {
    pub driver: Arc<dyn DatabaseDriver>,
    pub database: Option<String>,
    pub schema: Option<String>,
}

fn config_for(app: &AppHandle, connection_id: &str) -> Result<ConnectionConfig, String> {
    storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))
}

/// Builds a fresh, uncached driver for `connection_id` with an optional
/// database / schema override. Same-connection sync needs this so comparing
/// a target database does not change the source connection's current scope.
pub(crate) async fn isolated_driver_for(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
    database: Option<&str>,
    schema: Option<&str>,
) -> Result<Arc<dyn DatabaseDriver>, String> {
    let mut config = config_for(app, connection_id)?;
    let database_override = database.map(str::trim).filter(|value| !value.is_empty());
    let schema_override = schema.map(str::trim).filter(|value| !value.is_empty());

    match config.driver.as_str() {
        "sqlite" => {
            if database_override.is_some() || schema_override.is_some() {
                return Err("SQLite 不支持切换目标数据库 / 模式".to_string());
            }
            Ok(Arc::new(SqliteDriver::connect(&config.file).await?))
        }
        "mysql" => {
            if let Some(database) = database_override {
                config.database = database.to_string();
            }
            let password = password_for(state, &config, "db").await?;
            let (host, port, tunnel) = connection_target(state, &config, None).await?;
            Ok(Arc::new(
                MySqlDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?,
            ))
        }
        "postgres" => {
            if let Some(database) = database_override {
                config.database = database.to_string();
            }
            let password = password_for(state, &config, "db").await?;
            let (host, port, tunnel) = connection_target(state, &config, None).await?;
            let driver =
                PostgresDriver::connect(&config, &host, port, password.as_deref(), tunnel).await?;
            if let Some(schema) = schema_override {
                driver.switch_schema(schema).await?;
            }
            Ok(Arc::new(driver))
        }
        other => Err(format!("暂不支持的数据库类型: {other}")),
    }
}

fn resolve_scope(
    driver: &str,
    source_database: Option<&str>,
    source_schema: Option<&str>,
    target_database: Option<&str>,
    target_schema: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
    if driver == "sqlite" {
        return Err("SQLite 同一连接不能作为目标，请选择另一条 SQLite 连接".to_string());
    }
    if driver == "redis" {
        return Err("Redis 不支持结构 / 数据同步".to_string());
    }

    let source_database = source_database.map(str::to_string);
    let source_schema = source_schema.map(str::to_string);
    let database = target_database
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| source_database.clone());
    let schema = target_schema
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| source_schema.clone());

    Ok((database, schema))
}

/// Resolves the source scope of a comparison. The cached driver is reused
/// when the selected database / schema matches its current scope; otherwise
/// a fresh driver is opened so the connection's visible scope is untouched.
pub(crate) async fn resolve_source(
    app: &AppHandle,
    state: &AppState,
    source_config: &ConnectionConfig,
    source_connection_id: &str,
    source_database: Option<&str>,
    source_schema: Option<&str>,
) -> Result<ResolvedTarget, String> {
    let cached = driver_for(app, state, source_connection_id).await?;
    let (current_database, current_schema) = cached.context().await;

    let database_override = source_database
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let schema_override = source_schema
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if database_override.is_none() && schema_override.is_none() {
        return Ok(ResolvedTarget {
            driver: cached,
            database: current_database,
            schema: current_schema,
        });
    }
    if source_config.driver == "sqlite" {
        return Err("SQLite 不支持选择源数据库 / 模式".to_string());
    }
    if source_config.driver == "redis" {
        return Err("Redis 不支持结构 / 数据同步".to_string());
    }

    let database = database_override
        .map(str::to_string)
        .or_else(|| current_database.clone());
    let schema = if source_config.driver == "mysql" {
        None
    } else {
        schema_override
            .map(str::to_string)
            .or_else(|| current_schema.clone())
    };

    if database == current_database && schema == current_schema {
        return Ok(ResolvedTarget {
            driver: cached,
            database,
            schema,
        });
    }

    let driver = isolated_driver_for(
        app,
        state,
        &source_config.id,
        database.as_deref(),
        schema.as_deref(),
    )
    .await?;
    Ok(ResolvedTarget {
        driver,
        database,
        schema,
    })
}

/// Resolves the destination of a schema / data comparison. A different
/// connection uses its cached driver and current scope; the same connection
/// uses an isolated driver pointed at the requested database / schema.
pub(crate) async fn resolve_target(
    app: &AppHandle,
    state: &AppState,
    source_config: &ConnectionConfig,
    source_driver: &Arc<dyn DatabaseDriver>,
    target_connection_id: &str,
    target_database: Option<&str>,
    target_schema: Option<&str>,
) -> Result<ResolvedTarget, String> {
    if target_connection_id != source_config.id {
        let has_database = target_database
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let has_schema = target_schema
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if has_database || has_schema {
            let driver = isolated_driver_for(
                app,
                state,
                target_connection_id,
                target_database,
                target_schema,
            )
            .await?;
            let (database, schema) = driver.context().await;
            return Ok(ResolvedTarget {
                driver,
                database,
                schema,
            });
        }
        let driver = driver_for(app, state, target_connection_id).await?;
        let (database, schema) = driver.context().await;
        return Ok(ResolvedTarget {
            driver,
            database,
            schema,
        });
    }

    let (source_database, source_schema) = source_driver.context().await;
    let (database, schema) = resolve_scope(
        &source_config.driver,
        source_database.as_deref(),
        source_schema.as_deref(),
        target_database,
        target_schema,
    )?;

    if database == source_database && schema == source_schema {
        // Same scope is valid when the table pair differs; reuse the source
        // driver instead of opening a second connection to the same scope.
        return Ok(ResolvedTarget {
            driver: source_driver.clone(),
            database,
            schema,
        });
    }

    let driver = isolated_driver_for(
        app,
        state,
        &source_config.id,
        database.as_deref(),
        schema.as_deref(),
    )
    .await?;
    Ok(ResolvedTarget {
        driver,
        database,
        schema,
    })
}

pub(crate) fn context_label(
    name: &str,
    driver: &str,
    database: Option<&str>,
    schema: Option<&str>,
) -> String {
    match driver {
        "mysql" => database
            .filter(|value| !value.is_empty())
            .map(|database| format!("{name} · {database}"))
            .unwrap_or_else(|| name.to_string()),
        "postgres" => match (
            database.filter(|value| !value.is_empty()),
            schema.filter(|value| !value.is_empty()),
        ) {
            (Some(database), Some(schema)) => format!("{name} · {database} · {schema}"),
            (Some(database), None) => format!("{name} · {database}"),
            (None, Some(schema)) => format!("{name} · {schema}"),
            (None, None) => name.to_string(),
        },
        _ => name.to_string(),
    }
}

#[tauri::command]
pub async fn get_connection_context(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ConnectionContext, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    let (database, schema) = driver.context().await;
    Ok(ConnectionContext { database, schema })
}

#[tauri::command]
pub async fn list_context_tables(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
    schema: Option<String>,
) -> Result<Vec<TableInfo>, String> {
    let has_database = database
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_schema = schema
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let driver = if has_database || has_schema {
        isolated_driver_for(
            &app,
            &state,
            &connection_id,
            database.as_deref(),
            schema.as_deref(),
        )
        .await?
    } else {
        driver_for(&app, &state, &connection_id).await?
    };
    driver.list_tables().await
}

#[tauri::command]
pub async fn list_context_schemas(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: Option<String>,
) -> Result<Vec<String>, String> {
    let has_database = database
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let driver = if has_database {
        isolated_driver_for(&app, &state, &connection_id, database.as_deref(), None).await?
    } else {
        driver_for(&app, &state, &connection_id).await?
    };
    driver.list_schemas().await
}

#[cfg(test)]
mod tests {
    use super::{context_label, resolve_scope};

    #[test]
    fn labels_include_database_and_schema() {
        assert_eq!(
            context_label("prod", "mysql", Some("app"), None),
            "prod · app"
        );
        assert_eq!(
            context_label("pg", "postgres", Some("app"), Some("public")),
            "pg · app · public"
        );
        assert_eq!(context_label("local", "sqlite", None, None), "local");
        assert_eq!(context_label("prod", "mysql", None, None), "prod");
    }

    #[test]
    fn resolves_same_instance_targets() {
        assert_eq!(
            resolve_scope("mysql", Some("a"), None, Some("b"), None).expect("mysql target"),
            (Some("b".to_string()), None)
        );
        assert_eq!(
            resolve_scope("mysql", Some("a"), None, Some("a"), None)
                .expect("same mysql scope"),
            (Some("a".to_string()), None)
        );
        assert_eq!(
            resolve_scope("mysql", Some("a"), None, None, None)
                .expect("default mysql scope"),
            (Some("a".to_string()), None)
        );
        assert_eq!(
            resolve_scope(
                "postgres",
                Some("app"),
                Some("public"),
                Some("app"),
                Some("archive"),
            )
            .expect("postgres schema target"),
            (
                Some("app".to_string()),
                Some("archive".to_string())
            )
        );
        assert_eq!(
            resolve_scope(
                "postgres",
                Some("app"),
                Some("public"),
                Some("other"),
                None,
            )
            .expect("postgres database target"),
            (Some("other".to_string()), Some("public".to_string()))
        );
        assert_eq!(
            resolve_scope(
                "postgres",
                Some("app"),
                Some("public"),
                Some("app"),
                Some("public")
            )
            .expect("same postgres scope"),
            (Some("app".to_string()), Some("public".to_string()))
        );
        assert!(resolve_scope("sqlite", None, None, Some("x"), None).is_err());
    }
}