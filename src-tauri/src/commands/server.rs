use super::{AppState, driver_for, ensure_writable};
use crate::models::{
    ServerMetric, ServerOverview, ServerProcess, ServerTopTable, ServerUser, ServerUserDetail,
};
use crate::storage;
use serde_json::Value;
use std::time::Instant;
use tauri::{AppHandle, State};

fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn bool_label(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "t" | "true" | "yes" | "y" | "1" => "是".to_string(),
        "f" | "false" | "no" | "n" | "0" | "" => "否".to_string(),
        other => other.to_string(),
    }
}

fn cell(result: &crate::models::QueryResult, row: usize, column: usize) -> String {
    result
        .rows
        .get(row)
        .and_then(|values| values.get(column))
        .map(text)
        .unwrap_or_default()
}

fn number_cell(result: &crate::models::QueryResult, row: usize, column: usize) -> Option<u64> {
    cell(result, row, column).trim().parse().ok()
}

fn driver_for_connection(app: &AppHandle, connection_id: &str) -> Result<String, String> {
    let config = storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    match config.driver.as_str() {
        "mysql" | "postgres" => Ok(config.driver),
        _ => Err("当前驱动不支持服务器活动面板".to_string()),
    }
}

fn percent(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator > 0.0 {
        Some((numerator / denominator * 1000.0).round() / 10.0)
    } else {
        None
    }
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "''")
}

fn escape_postgres_literal(value: &str) -> String {
    value.replace('\'', "''")
}

/// SHOW GRANTS on old MySQL versions could include an identified password
/// hash. The panel is read-only and must never expose hashes.
fn sanitize_grant(line: &str) -> Option<String> {
    let upper = line.to_ascii_uppercase();
    if upper.contains("IDENTIFIED BY PASSWORD") {
        if let Some(index) = upper.find("IDENTIFIED BY PASSWORD") {
            let mut sanitized = line[..index].to_string();
            sanitized.push_str("IDENTIFIED BY PASSWORD '<已隐藏>'");
            return Some(sanitized);
        }
    }
    Some(line.to_string())
}

#[tauri::command]
pub async fn server_overview(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ServerOverview, String> {
    let driver_kind = driver_for_connection(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let mut metrics = Vec::new();
    let sampled_at = chrono::Local::now().format("%H:%M:%S").to_string();
    let previous = state
        .server_samples
        .lock()
        .await
        .get(&connection_id)
        .copied();

    if driver_kind == "mysql" {
        let basic = driver
            .execute(
                "SELECT VERSION() AS version, USER() AS user, DATABASE() AS db",
                10,
            )
            .await?;
        let mut counter: Option<u64> = None;
        let mut cache_hit_percent = None;
        let mut running = None;

        if let Ok(status) = driver
            .execute(
                "SHOW GLOBAL STATUS WHERE Variable_name IN \
                 ('Uptime','Threads_connected','Threads_running','Questions', \
                  'Slow_queries','Innodb_buffer_pool_read_requests', \
                  'Innodb_buffer_pool_reads')",
                100,
            )
            .await
        {
            let mut buffer_requests = None;
            let mut buffer_reads = None;
            for row in 0..status.rows.len() {
                let label = cell(&status, row, 0);
                let value = cell(&status, row, 1);
                match label.as_str() {
                    "Uptime" => metrics.push(ServerMetric {
                        label: "运行时长（秒）".to_string(),
                        value,
                    }),
                    "Threads_connected" => metrics.push(ServerMetric {
                        label: "当前连接数".to_string(),
                        value,
                    }),
                    "Threads_running" => {
                        running = value.parse::<u64>().ok();
                        metrics.push(ServerMetric {
                            label: "运行中线程".to_string(),
                            value,
                        });
                    }
                    "Questions" => {
                        counter = value.parse::<u64>().ok();
                        metrics.push(ServerMetric {
                            label: "累计查询数".to_string(),
                            value,
                        });
                    }
                    "Slow_queries" => metrics.push(ServerMetric {
                        label: "慢查询数".to_string(),
                        value,
                    }),
                    "Innodb_buffer_pool_read_requests" => {
                        buffer_requests = value.parse::<f64>().ok();
                    }
                    "Innodb_buffer_pool_reads" => {
                        buffer_reads = value.parse::<f64>().ok();
                    }
                    _ => {}
                }
            }
            if let (Some(requests), Some(reads)) = (buffer_requests, buffer_reads) {
                cache_hit_percent = percent(requests - reads, requests);
            }
        }

        let max_connections = driver
            .execute(
                "SHOW GLOBAL VARIABLES WHERE Variable_name = 'max_connections'",
                10,
            )
            .await
            .ok()
            .and_then(|result| number_cell(&result, 0, 1));

        let size = driver
            .execute(
                "SELECT COALESCE(SUM(data_length + index_length), 0) AS bytes, \
                        COUNT(*) AS table_count \
                 FROM information_schema.tables \
                 WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE'",
                10,
            )
            .await
            .ok();
        let database_size_bytes = size.as_ref().and_then(|result| number_cell(result, 0, 0));
        let table_count = size.as_ref().and_then(|result| number_cell(result, 0, 1));

        let top_tables = driver
            .execute(
                "SELECT table_name, data_length, index_length \
                 FROM information_schema.tables \
                 WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' \
                 ORDER BY (data_length + index_length) DESC LIMIT 5",
                10,
            )
            .await
            .map(|result| {
                (0..result.rows.len())
                    .map(|row| {
                        let data_bytes = number_cell(&result, row, 1).unwrap_or(0);
                        let index_bytes = number_cell(&result, row, 2).unwrap_or(0);
                        ServerTopTable {
                            name: cell(&result, row, 0),
                            total_bytes: data_bytes + index_bytes,
                            data_bytes,
                            index_bytes,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let connections = metrics
            .iter()
            .find(|metric| metric.label == "当前连接数")
            .and_then(|metric| metric.value.parse::<f64>().ok());
        let connection_usage_percent = match (connections, max_connections) {
            (Some(current), Some(max)) => percent(current, max as f64),
            _ => None,
        };
        let now = Instant::now();
        let qps = previous.and_then(|(at, count)| {
            let seconds = now.duration_since(at).as_secs_f64();
            let current = counter?;
            if seconds >= 0.5 && current >= count {
                Some(((current - count) as f64 / seconds * 10.0).round() / 10.0)
            } else {
                None
            }
        });
        if let Some(current) = counter {
            state
                .server_samples
                .lock()
                .await
                .insert(connection_id.clone(), (now, current));
        }

        return Ok(ServerOverview {
            driver: driver_kind,
            version: cell(&basic, 0, 0),
            current_user: cell(&basic, 0, 1),
            current_database: cell(&basic, 0, 2),
            metrics,
            sampled_at,
            max_connections,
            connection_usage_percent,
            database_size_bytes,
            table_count,
            qps,
            cache_hit_percent,
            active_sessions: running,
            idle_sessions: None,
            top_tables,
        });
    }

    let basic = driver
        .execute(
            "SELECT version() AS version, current_user AS user, current_database() AS db, \
                    EXTRACT(EPOCH FROM (now() - pg_postmaster_start_time()))::bigint AS uptime, \
                    current_setting('max_connections')::bigint AS max_connections, \
                    (SELECT count(*) FROM pg_stat_activity) AS connections, \
                    (SELECT count(*) FROM pg_stat_activity WHERE state = 'active') AS active, \
                    (SELECT count(*) FROM pg_stat_activity WHERE state = 'idle') AS idle",
            10,
        )
        .await?;
    metrics.push(ServerMetric {
        label: "运行时长（秒）".to_string(),
        value: cell(&basic, 0, 3),
    });
    metrics.push(ServerMetric {
        label: "当前连接数".to_string(),
        value: cell(&basic, 0, 5),
    });
    let max_connections = number_cell(&basic, 0, 4);
    let active_sessions = number_cell(&basic, 0, 6);
    let idle_sessions = number_cell(&basic, 0, 7);
    let connections = number_cell(&basic, 0, 5);
    let connection_usage_percent = match (connections, max_connections) {
        (Some(current), Some(max)) => percent(current as f64, max as f64),
        _ => None,
    };

    let mut counter: Option<u64> = None;
    let mut database_size_bytes = None;
    let mut cache_hit_percent = None;
    if let Ok(stats) = driver
        .execute(
            "SELECT pg_database_size(current_database()) AS bytes, \
                    COALESCE(blks_hit, 0) AS hit, COALESCE(blks_read, 0) AS read, \
                    COALESCE(xact_commit, 0) + COALESCE(xact_rollback, 0) AS xacts \
             FROM pg_stat_database WHERE datname = current_database()",
            10,
        )
        .await
    {
        database_size_bytes = number_cell(&stats, 0, 0);
        let hit = number_cell(&stats, 0, 1).map(|value| value as f64);
        let read = number_cell(&stats, 0, 2).map(|value| value as f64);
        if let (Some(hit), Some(read)) = (hit, read) {
            cache_hit_percent = percent(hit, hit + read);
        }
        counter = number_cell(&stats, 0, 3);
    }

    let table_count = driver
        .execute(
            "SELECT count(*) FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'p') \
               AND n.nspname NOT IN ('pg_catalog', 'information_schema')",
            10,
        )
        .await
        .ok()
        .and_then(|result| number_cell(&result, 0, 0));

    let top_tables = driver
        .execute(
            "SELECT c.relname, pg_total_relation_size(c.oid), \
                    pg_table_size(c.oid), pg_indexes_size(c.oid) \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'p') \
               AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 5",
            10,
        )
        .await
        .map(|result| {
            (0..result.rows.len())
                .map(|row| ServerTopTable {
                    name: cell(&result, row, 0),
                    total_bytes: number_cell(&result, row, 1).unwrap_or(0),
                    data_bytes: number_cell(&result, row, 2).unwrap_or(0),
                    index_bytes: number_cell(&result, row, 3).unwrap_or(0),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let now = Instant::now();
    let qps = previous.and_then(|(at, count)| {
        let seconds = now.duration_since(at).as_secs_f64();
        let current = counter?;
        if seconds >= 0.5 && current >= count {
            Some(((current - count) as f64 / seconds * 10.0).round() / 10.0)
        } else {
            None
        }
    });
    if let Some(current) = counter {
        state
            .server_samples
            .lock()
            .await
            .insert(connection_id.clone(), (now, current));
    }

    Ok(ServerOverview {
        driver: driver_kind,
        version: cell(&basic, 0, 0),
        current_user: cell(&basic, 0, 1),
        current_database: cell(&basic, 0, 2),
        metrics,
        sampled_at,
        max_connections,
        connection_usage_percent,
        database_size_bytes,
        table_count,
        qps,
        cache_hit_percent,
        active_sessions,
        idle_sessions,
        top_tables,
    })
}

#[tauri::command]
pub async fn server_users(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<ServerUser>, String> {
    let driver_kind = driver_for_connection(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;

    if driver_kind == "mysql" {
        let rich = driver
            .execute(
                "SELECT User, Host, plugin, account_locked, password_expired, \
                        password_last_changed \
                 FROM mysql.user ORDER BY User, Host LIMIT 500",
                500,
            )
            .await;
        let result = match rich {
            Ok(result) => result,
            Err(_) => driver
                .execute(
                    "SELECT User, Host FROM mysql.user ORDER BY User, Host LIMIT 500",
                    500,
                )
                .await
                .map_err(|error| format!("当前账号无权查看用户列表: {error}"))?,
        };
        return Ok((0..result.rows.len())
            .map(|row| {
                let mut attributes = Vec::new();
                if result.columns.len() > 2 {
                    attributes.push(ServerMetric {
                        label: "插件".to_string(),
                        value: cell(&result, row, 2),
                    });
                }
                if result.columns.len() > 3 {
                    attributes.push(ServerMetric {
                        label: "账号锁定".to_string(),
                        value: bool_label(&cell(&result, row, 3)),
                    });
                }
                if result.columns.len() > 4 {
                    attributes.push(ServerMetric {
                        label: "密码过期".to_string(),
                        value: bool_label(&cell(&result, row, 4)),
                    });
                }
                if result.columns.len() > 5 {
                    attributes.push(ServerMetric {
                        label: "密码最后修改".to_string(),
                        value: cell(&result, row, 5),
                    });
                }
                ServerUser {
                    name: cell(&result, row, 0),
                    host: cell(&result, row, 1),
                    attributes,
                }
            })
            .collect());
    }

    let result = driver
        .execute(
            "SELECT rolname, rolsuper, rolcanlogin, rolcreatedb, rolcreaterole, \
                    rolconnlimit, rolvaliduntil::text, \
                    has_database_privilege(rolname, current_database(), 'CONNECT'), \
                    has_database_privilege(rolname, current_database(), 'CREATE'), \
                    has_database_privilege(rolname, current_database(), 'TEMPORARY') \
             FROM pg_roles ORDER BY rolname LIMIT 500",
            500,
        )
        .await
        .map_err(|error| format!("当前账号无权查看角色列表: {error}"))?;
    Ok((0..result.rows.len())
        .map(|row| ServerUser {
            name: cell(&result, row, 0),
            host: String::new(),
            attributes: vec![
                ServerMetric {
                    label: "超级用户".to_string(),
                    value: bool_label(&cell(&result, row, 1)),
                },
                ServerMetric {
                    label: "可登录".to_string(),
                    value: bool_label(&cell(&result, row, 2)),
                },
                ServerMetric {
                    label: "可建库".to_string(),
                    value: bool_label(&cell(&result, row, 3)),
                },
                ServerMetric {
                    label: "可建角色".to_string(),
                    value: bool_label(&cell(&result, row, 4)),
                },
                ServerMetric {
                    label: "连接限制".to_string(),
                    value: cell(&result, row, 5),
                },
                ServerMetric {
                    label: "有效期".to_string(),
                    value: cell(&result, row, 6),
                },
                ServerMetric {
                    label: "当前库 CONNECT".to_string(),
                    value: bool_label(&cell(&result, row, 7)),
                },
                ServerMetric {
                    label: "当前库 CREATE".to_string(),
                    value: bool_label(&cell(&result, row, 8)),
                },
                ServerMetric {
                    label: "当前库 TEMPORARY".to_string(),
                    value: bool_label(&cell(&result, row, 9)),
                },
            ],
        })
        .collect())
}

#[tauri::command]
pub async fn server_user_grants(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
    host: String,
) -> Result<ServerUserDetail, String> {
    let driver_kind = driver_for_connection(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("用户 / 角色名称不能为空".to_string());
    }

    if driver_kind == "mysql" {
        let account_name = escape_sql_literal(&name);
        let account_host = escape_sql_literal(if host.trim().is_empty() { "%" } else { &host });
        let result = driver
            .execute(
                &format!("SHOW GRANTS FOR '{account_name}'@'{account_host}'"),
                200,
            )
            .await
            .map_err(|error| format!("读取授权失败: {error}"))?;
        let grants = (0..result.rows.len())
            .filter_map(|row| sanitize_grant(&cell(&result, row, 0)))
            .collect();
        return Ok(ServerUserDetail {
            name,
            host: host.trim().to_string(),
            grants,
            memberships: Vec::new(),
            privileges: Vec::new(),
            notes: vec![
                "只读查看；授权与回收请使用 SQL 或专用工具。".to_string(),
                "不显示密码散列或认证字符串。".to_string(),
            ],
        });
    }

    let role = escape_postgres_literal(&name);
    let memberships = driver
        .execute(
            &format!(
                "SELECT g.rolname FROM pg_auth_members m \
                 JOIN pg_roles r ON r.oid = m.member \
                 JOIN pg_roles g ON g.oid = m.roleid \
                 WHERE r.rolname = '{role}' ORDER BY g.rolname LIMIT 200"
            ),
            200,
        )
        .await
        .map(|result| {
            (0..result.rows.len())
                .map(|row| cell(&result, row, 0))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let privileges = driver
        .execute(
            &format!(
                "SELECT has_database_privilege('{role}', current_database(), 'CONNECT'), \
                        has_database_privilege('{role}', current_database(), 'CREATE'), \
                        has_database_privilege('{role}', current_database(), 'TEMPORARY')"
            ),
            10,
        )
        .await
        .map(|result| {
            vec![
                ServerMetric {
                    label: "CONNECT".to_string(),
                    value: bool_label(&cell(&result, 0, 0)),
                },
                ServerMetric {
                    label: "CREATE".to_string(),
                    value: bool_label(&cell(&result, 0, 1)),
                },
                ServerMetric {
                    label: "TEMPORARY".to_string(),
                    value: bool_label(&cell(&result, 0, 2)),
                },
            ]
        })
        .unwrap_or_default();

    Ok(ServerUserDetail {
        name,
        host: String::new(),
        grants: Vec::new(),
        memberships,
        privileges,
        notes: vec![
            "只读查看；授权与回收请使用 SQL 或专用工具。".to_string(),
            "角色成员关系最多显示 200 条，权限只检查当前数据库。".to_string(),
        ],
    })
}

#[tauri::command]
pub async fn server_processes(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<ServerProcess>, String> {
    let driver_kind = driver_for_connection(&app, &connection_id)?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    if driver_kind == "mysql" {
        let result = driver.execute("SHOW FULL PROCESSLIST", 500).await?;
        return Ok((0..result.rows.len())
            .map(|row| ServerProcess {
                id: cell(&result, row, 0),
                user: cell(&result, row, 1),
                database: cell(&result, row, 3),
                state: format!("{} {}", cell(&result, row, 4), cell(&result, row, 6))
                    .trim()
                    .to_string(),
                time_seconds: cell(&result, row, 5).parse().unwrap_or(0),
                info: cell(&result, row, 7),
            })
            .collect());
    }

    let result = driver
        .execute(
            "SELECT pid, usename, datname, state, \
                    EXTRACT(EPOCH FROM (now() - query_start()))::bigint AS seconds, query \
             FROM pg_stat_activity \
             WHERE pid <> pg_backend_pid() \
             ORDER BY query_start NULLS LAST",
            500,
        )
        .await?;
    Ok((0..result.rows.len())
        .map(|row| ServerProcess {
            id: cell(&result, row, 0),
            user: cell(&result, row, 1),
            database: cell(&result, row, 2),
            state: cell(&result, row, 3),
            time_seconds: cell(&result, row, 4).parse().unwrap_or(0),
            info: cell(&result, row, 5),
        })
        .collect())
}

#[tauri::command]
pub async fn terminate_server_process(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    process_id: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let driver_kind = driver_for_connection(&app, &connection_id)?;
    let pid: i64 = process_id
        .trim()
        .parse()
        .map_err(|_| "进程 / 会话 ID 必须是整数".to_string())?;
    let driver = driver_for(&app, &state, &connection_id).await?;
    let sql = if driver_kind == "mysql" {
        format!("KILL {pid}")
    } else {
        format!("SELECT pg_terminate_backend({pid})")
    };
    driver.execute(&sql, 1).await?;
    Ok(())
}
