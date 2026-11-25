use crate::db::query::is_read_query;
use crate::db::{DatabaseDriver, DatabaseSession};
use crate::models::{
    ColumnInfo, ColumnValue, ConnectionConfig, DatabaseObjects, ForeignKeyInfo, IndexInfo,
    QueryResult, ScopeTableColumns, TableInfo, TableRows, TableStructure,
};
use crate::ssh::SshTunnel;
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use futures_util::TryStreamExt;
use serde_json::Value;
use sqlx::mysql::{MySqlArguments, MySqlConnectOptions, MySqlPoolOptions, MySqlRow, MySqlSslMode};
use sqlx::{Column, Connection, MySqlConnection, MySqlPool, Row, TypeInfo};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::time::Instant;
use tokio::sync::RwLock;

pub struct MySqlDriver {
    pool: MySqlPool,
    /// Current database for metadata and unqualified user SQL. Switching it
    /// does not reopen the connection.
    database: RwLock<String>,
    #[allow(dead_code)]
    tunnel: Option<SshTunnel>,
}

impl MySqlDriver {
    pub async fn connect(
        config: &ConnectionConfig,
        host: &str,
        port: u16,
        password: Option<&str>,
        tunnel: Option<SshTunnel>,
    ) -> Result<Self, String> {
        let ssl_mode = match config.ssl_mode.as_str() {
            "preferred" => MySqlSslMode::Preferred,
            "required" => MySqlSslMode::Required,
            "verify-ca" => MySqlSslMode::VerifyCa,
            "verify-full" => MySqlSslMode::VerifyIdentity,
            _ => MySqlSslMode::Disabled,
        };
        let needs_ca = matches!(config.ssl_mode.as_str(), "verify-ca" | "verify-full");
        if needs_ca && config.ssl_ca_path.trim().is_empty() {
            return Err("verify-ca / verify-full 必须选择 CA 证书文件".to_string());
        }
        let ca_path = config.ssl_ca_path.trim();
        if !ca_path.is_empty() && !std::path::Path::new(ca_path).is_file() {
            return Err(format!("CA 证书文件不存在: {ca_path}"));
        }
        let client_cert = config.ssl_client_cert_path.trim();
        let client_key = config.ssl_client_key_path.trim();
        if client_cert.is_empty() != client_key.is_empty() {
            return Err("客户端证书与私钥必须同时选择".to_string());
        }
        if !client_cert.is_empty() && !std::path::Path::new(client_cert).is_file() {
            return Err(format!("客户端证书文件不存在: {client_cert}"));
        }
        if !client_key.is_empty() && !std::path::Path::new(client_key).is_file() {
            return Err(format!("客户端私钥文件不存在: {client_key}"));
        }

        let database = if config.database.trim().is_empty() {
            "mysql".to_string()
        } else {
            config.database.trim().to_string()
        };

        let mut options = MySqlConnectOptions::new()
            .host(host)
            .port(if port == 0 { 3306 } else { port })
            .username(&config.username)
            .password(password.unwrap_or_default())
            .ssl_mode(ssl_mode)
            .database(&database);
        if !ca_path.is_empty() {
            options = options.ssl_ca(ca_path);
        }
        if !client_cert.is_empty() {
            options = options
                .ssl_client_cert(client_cert)
                .ssl_client_key(client_key);
        }

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|error| format!("连接 MySQL 失败: {error}"))?;

        Ok(Self {
            pool,
            database: RwLock::new(database),
            tunnel,
        })
    }

    async fn current_database(&self) -> String {
        self.database.read().await.clone()
    }
}

async fn execute_on_connection(
    connection: &mut MySqlConnection,
    sql: &str,
    max_rows: usize,
) -> Result<QueryResult, String> {
    let started = Instant::now();
    let sql = sql.trim();
    if sql.is_empty() {
        return Err("SQL 不能为空".to_string());
    }

    if is_read_query(sql) {
        let rows = sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .fetch_all(&mut *connection)
            .await
            .map_err(|error| format!("查询失败: {error}"))?;

        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let truncated = rows.len() > max_rows;
        let values = rows
            .iter()
            .take(max_rows)
            .map(|row| {
                (0..row.len())
                    .map(|index| value_to_json(row, index))
                    .collect()
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows: values,
            rows_affected: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
            truncated,
            is_query: true,
        })
    } else {
        let result = sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("执行失败: {error}"))?;

        Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: result.rows_affected(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            truncated: false,
            is_query: false,
        })
    }
}

struct MySqlSession {
    connection: sqlx::pool::PoolConnection<sqlx::MySql>,
}

#[async_trait]
impl DatabaseSession for MySqlSession {
    async fn begin(&mut self) -> Result<(), String> {
        sqlx::raw_sql("START TRANSACTION")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| format!("开启事务失败: {error}"))?;
        Ok(())
    }

    async fn execute(&mut self, sql: &str, max_rows: usize) -> Result<QueryResult, String> {
        execute_on_connection(&mut self.connection, sql, max_rows).await
    }

    async fn commit(&mut self) -> Result<(), String> {
        sqlx::raw_sql("COMMIT")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| format!("提交事务失败: {error}"))?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), String> {
        sqlx::raw_sql("ROLLBACK")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| format!("回滚事务失败: {error}"))?;
        Ok(())
    }

    async fn backend_id(&mut self) -> Result<Option<i64>, String> {
        let id: i64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
            .fetch_one(&mut *self.connection)
            .await
            .map_err(|error| format!("读取连接 ID 失败: {error}"))?;
        Ok(Some(id))
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    async fn test_connection(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| format!("连接测试失败: {error}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取数据库列表失败: {error}"))?;
        Ok(rows)
    }

    async fn context(&self) -> (Option<String>, Option<String>) {
        (Some(self.current_database().await), None)
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        let database = self.current_database().await;

        let rows = sqlx::query(
            "SELECT table_name, table_type \
             FROM information_schema.tables \
             WHERE table_schema = ? \
             ORDER BY table_type, table_name",
        )
        .bind(&database)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取表列表失败: {error}"))?;

        let mut objects: Vec<TableInfo> = rows
            .into_iter()
            .map(|row| {
                let name: String = row.try_get(0).unwrap_or_default();
                let raw_kind: String = row.try_get(1).unwrap_or_default();
                let kind = if raw_kind.eq_ignore_ascii_case("VIEW")
                    || raw_kind.eq_ignore_ascii_case("SYSTEM VIEW")
                {
                    "view"
                } else if raw_kind.eq_ignore_ascii_case("SEQUENCE") {
                    "sequence"
                } else {
                    "table"
                };
                TableInfo {
                    name,
                    kind: kind.to_string(),
                }
            })
            .collect();

        if let Ok(routines) = sqlx::query(
            "SELECT routine_name, routine_type \
             FROM information_schema.routines \
             WHERE routine_schema = ? \
             ORDER BY routine_type, routine_name",
        )
        .bind(&database)
        .fetch_all(&self.pool)
        .await
        {
            objects.extend(routines.into_iter().map(|row| {
                let name: String = row.try_get(0).unwrap_or_default();
                let raw_kind: String = row.try_get(1).unwrap_or_default();
                TableInfo {
                    name,
                    kind: if raw_kind.eq_ignore_ascii_case("PROCEDURE") {
                        "procedure".to_string()
                    } else {
                        "function".to_string()
                    },
                }
            }));
        }

        if let Ok(triggers) = sqlx::query(
            "SELECT DISTINCT trigger_name FROM information_schema.triggers \
             WHERE trigger_schema = ? \
             ORDER BY trigger_name",
        )
        .bind(&database)
        .fetch_all(&self.pool)
        .await
        {
            objects.extend(triggers.into_iter().map(|row| TableInfo {
                name: row.try_get(0).unwrap_or_default(),
                kind: "trigger".to_string(),
            }));
        }

        if let Ok(events) = sqlx::query(
            "SELECT event_name FROM information_schema.events \
             WHERE event_schema = ? \
             ORDER BY event_name",
        )
        .bind(&database)
        .fetch_all(&self.pool)
        .await
        {
            objects.extend(events.into_iter().map(|row| TableInfo {
                name: row.try_get(0).unwrap_or_default(),
                kind: "event".to_string(),
            }));
        }

        Ok(objects)
    }

    async fn list_database_objects(&self) -> Result<Vec<DatabaseObjects>, String> {
        let mut map: BTreeMap<String, Vec<TableInfo>> = BTreeMap::new();

        let relations = sqlx::query(
            "SELECT table_schema, table_name, table_type \
             FROM information_schema.tables \
             ORDER BY table_schema, table_type, table_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取全部数据库对象失败: {error}"))?;

        for row in relations {
            let database: String = row.try_get(0).unwrap_or_default();
            let name: String = row.try_get(1).unwrap_or_default();
            let raw_kind: String = row.try_get(2).unwrap_or_default();
            let kind = if raw_kind.eq_ignore_ascii_case("VIEW")
                || raw_kind.eq_ignore_ascii_case("SYSTEM VIEW")
            {
                "view"
            } else if raw_kind.eq_ignore_ascii_case("SEQUENCE") {
                "sequence"
            } else {
                "table"
            };
            map.entry(database).or_default().push(TableInfo {
                name,
                kind: kind.to_string(),
            });
        }

        if let Ok(routines) = sqlx::query(
            "SELECT routine_schema, routine_name, routine_type \
             FROM information_schema.routines \
             ORDER BY routine_schema, routine_type, routine_name",
        )
        .fetch_all(&self.pool)
        .await
        {
            for row in routines {
                let database: String = row.try_get(0).unwrap_or_default();
                let name: String = row.try_get(1).unwrap_or_default();
                let raw_kind: String = row.try_get(2).unwrap_or_default();
                map.entry(database).or_default().push(TableInfo {
                    name,
                    kind: if raw_kind.eq_ignore_ascii_case("PROCEDURE") {
                        "procedure".to_string()
                    } else {
                        "function".to_string()
                    },
                });
            }
        }

        if let Ok(triggers) = sqlx::query(
            "SELECT DISTINCT trigger_schema, trigger_name \
             FROM information_schema.triggers \
             ORDER BY trigger_schema, trigger_name",
        )
        .fetch_all(&self.pool)
        .await
        {
            for row in triggers {
                let database: String = row.try_get(0).unwrap_or_default();
                map.entry(database).or_default().push(TableInfo {
                    name: row.try_get(1).unwrap_or_default(),
                    kind: "trigger".to_string(),
                });
            }
        }

        if let Ok(events) = sqlx::query(
            "SELECT event_schema, event_name \
             FROM information_schema.events \
             ORDER BY event_schema, event_name",
        )
        .fetch_all(&self.pool)
        .await
        {
            for row in events {
                let database: String = row.try_get(0).unwrap_or_default();
                map.entry(database).or_default().push(TableInfo {
                    name: row.try_get(1).unwrap_or_default(),
                    kind: "event".to_string(),
                });
            }
        }

        Ok(map
            .into_iter()
            .map(|(database, objects)| DatabaseObjects { database, objects })
            .collect())
    }

    async fn switch_database(&self, database: &str) -> Result<(), String> {
        let database = database.trim();
        if database.is_empty() {
            return Err("数据库名不能为空".to_string());
        }

        let exists: Option<String> = sqlx::query_scalar(
            "SELECT schema_name FROM information_schema.schemata \
             WHERE schema_name = ?",
        )
        .bind(database)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("校验数据库失败: {error}"))?;

        if exists.is_none() {
            return Err(format!("数据库不存在: {database}"));
        }

        *self.database.write().await = database.to_string();
        Ok(())
    }

    async fn list_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKeyInfo>, String> {
        let database = self.current_database().await;
        mysql_foreign_keys(&self.pool, &database, table).await
    }

    async fn index_table(&self, index_name: &str) -> Result<Option<String>, String> {
        let database = self.current_database().await;
        sqlx::query_scalar(
            "SELECT table_name FROM information_schema.statistics \
             WHERE table_schema = ? AND index_name = ? LIMIT 1",
        )
        .bind(&database)
        .bind(index_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("读取索引所属表失败: {error}"))
    }

    async fn execute(&self, sql: &str, max_rows: usize) -> Result<QueryResult, String> {
        // Pin one pooled connection so `USE` and the user's SQL share a
        // session; switching databases no longer reconnects the pool.
        let database = self.current_database().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let use_database = format!("USE {}", quote_identifier(&database));
        sqlx::raw_sql(sqlx::AssertSqlSafe(use_database))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("切换当前数据库失败: {error}"))?;

        execute_on_connection(&mut connection, sql, max_rows).await
    }

    async fn open_session(&self) -> Result<Box<dyn DatabaseSession>, String> {
        let database = self.current_database().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let use_database = format!("USE {}", quote_identifier(&database));
        sqlx::raw_sql(sqlx::AssertSqlSafe(use_database))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("切换当前数据库失败: {error}"))?;
        Ok(Box::new(MySqlSession { connection }))
    }

    async fn cancel_backend_query(&self, backend_id: i64) -> Result<bool, String> {
        // KILL QUERY ends only the statement; the session itself stays open.
        // Do not escalate to KILL <id>: that would drop the connection and
        // any open transaction.
        let sql = format!("KILL QUERY {backend_id}");
        match sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
        {
            Ok(_) => Ok(true),
            Err(error) => Err(format!(
                "取消查询失败（可能需要 PROCESS 权限，或语句已结束）: {error}"
            )),
        }
    }

    async fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let database = self.current_database().await;
        let rows = sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_key, extra, column_default, column_comment \
             FROM information_schema.columns \
             WHERE table_schema = ? AND table_name = ? \
             ORDER BY ordinal_position",
        )
        .bind(&database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取字段信息失败: {error}"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let name: String = row.try_get(0).unwrap_or_default();
                let data_type: String = row.try_get(1).unwrap_or_default();
                let nullable: String = row.try_get(2).unwrap_or_default();
                let column_key: String = row.try_get(3).unwrap_or_default();
                let extra: String = row.try_get(4).unwrap_or_default();
                let default_value: Option<String> = row.try_get(5).ok().flatten();
                let comment: Option<String> = row.try_get(6).ok().flatten();

                ColumnInfo {
                    name,
                    data_type,
                    nullable: nullable.eq_ignore_ascii_case("YES"),
                    primary_key: column_key.eq_ignore_ascii_case("PRI"),
                    auto_increment: extra.to_ascii_lowercase().contains("auto_increment"),
                    default_value,
                    comment: comment.filter(|value| !value.trim().is_empty()),
                }
            })
            .collect())
    }

    async fn scope_columns(&self) -> Result<Vec<ScopeTableColumns>, String> {
        let database = self.current_database().await;
        let rows = sqlx::query(
            "SELECT c.table_name, t.table_type, c.column_name, c.column_type, \
                    c.is_nullable, c.column_key, c.extra, c.column_default \
             FROM information_schema.columns c \
             JOIN information_schema.tables t \
               ON t.table_schema = c.table_schema AND t.table_name = c.table_name \
             WHERE c.table_schema = ? AND t.table_type IN ('BASE TABLE', 'VIEW') \
             ORDER BY c.table_name, c.ordinal_position",
        )
        .bind(&database)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取补全元数据失败: {error}"))?;

        let mut grouped: BTreeMap<String, (String, Vec<ColumnInfo>)> = BTreeMap::new();
        for row in rows {
            let table_name: String = row.try_get(0).unwrap_or_default();
            let table_type: String = row.try_get(1).unwrap_or_default();
            let kind = if table_type.eq_ignore_ascii_case("VIEW") {
                "view"
            } else {
                "table"
            };
            let nullable: String = row.try_get(4).unwrap_or_default();
            let column_key: String = row.try_get(5).unwrap_or_default();
            let extra: String = row.try_get(6).unwrap_or_default();
            let default_value: Option<String> = row.try_get(7).ok().flatten();
            let column = ColumnInfo {
                name: row.try_get(2).unwrap_or_default(),
                data_type: row.try_get(3).unwrap_or_default(),
                nullable: nullable.eq_ignore_ascii_case("YES"),
                primary_key: column_key.eq_ignore_ascii_case("PRI"),
                auto_increment: extra.to_ascii_lowercase().contains("auto_increment"),
                default_value,
                comment: None,
            };
            grouped
                .entry(table_name)
                .or_insert_with(|| (kind.to_string(), Vec::new()))
                .1
                .push(column);
        }

        Ok(grouped
            .into_iter()
            .map(|(name, (kind, columns))| ScopeTableColumns {
                name,
                kind,
                columns,
            })
            .collect())
    }

    async fn table_structure(&self, table: &str) -> Result<TableStructure, String> {
        let database = self.current_database().await;
        let columns = self.list_columns(table).await?;
        let indexes = mysql_indexes(&self.pool, &database, table).await?;
        let foreign_keys = mysql_foreign_keys(&self.pool, &database, table).await?;

        let ddl_sql = format!(
            "SHOW CREATE TABLE {}",
            qualified_identifier(&database, table)
        );
        // SHOW statements are not reliably supported by the prepared
        // statement protocol (notably MariaDB); use the simple protocol.
        let rows = sqlx::raw_sql(sqlx::AssertSqlSafe(ddl_sql))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("读取 DDL 失败（数据库 {database} · 表 {table}）: {error}"))?;
        let ddl = rows
            .first()
            .and_then(|row| row_text(row, 1))
            .unwrap_or_default();

        Ok(TableStructure {
            columns,
            indexes,
            foreign_keys,
            ddl,
        })
    }

    async fn execute_transaction(&self, statements: &[String]) -> Result<u64, String> {
        if statements.is_empty() {
            return Ok(0);
        }
        let database = self.current_database().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let use_database = format!("USE {}", quote_identifier(&database));
        sqlx::raw_sql(sqlx::AssertSqlSafe(use_database))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("切换当前数据库失败: {error}"))?;

        let mut transaction = connection
            .begin()
            .await
            .map_err(|error| format!("开启事务失败: {error}"))?;
        let mut count = 0u64;
        for (index, statement) in statements.iter().enumerate() {
            sqlx::raw_sql(sqlx::AssertSqlSafe(statement.as_str()))
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    format!("第 {} 条语句执行失败（事务已回滚）: {error}", index + 1)
                })?;
            count += 1;
        }
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交事务失败: {error}"))?;
        Ok(count)
    }

    async fn read_rows(
        &self,
        table: &str,
        limit: u64,
        offset: u64,
        filter: Option<&str>,
        order_by: Option<&str>,
        order_dir: Option<&str>,
    ) -> Result<TableRows, String> {
        let database = self.current_database().await;
        let columns = self.list_columns(table).await?;
        let identifier = qualified_identifier(&database, table);
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());
        if filter.is_some_and(|value| value.contains(';')) {
            return Err("WHERE 条件不能包含分号".to_string());
        }
        let where_clause = filter
            .map(|value| format!(" WHERE {value}"))
            .unwrap_or_default();

        let count_sql = format!("SELECT COUNT(*) FROM {identifier}{where_clause}");
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql))
            .fetch_one(&self.pool)
            .await
            .map_err(|error| format!("读取总行数失败: {error}"))?;

        let order_clause = if let Some(column) = order_by {
            if !columns.iter().any(|item| item.name == column) {
                return Err(format!("排序字段不存在: {column}"));
            }
            let direction = if order_dir
                .map(|value| value.eq_ignore_ascii_case("desc"))
                .unwrap_or(false)
            {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY {} {direction}", quote_identifier(column))
        } else {
            String::new()
        };

        let sql =
            format!("SELECT * FROM {identifier}{where_clause}{order_clause} LIMIT ? OFFSET ?");
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("读取表数据失败: {error}"))?;

        let values = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|index| value_to_json(row, index))
                    .collect()
            })
            .collect();

        Ok(TableRows {
            columns,
            rows: values,
            total: total.max(0) as u64,
            offset,
            limit,
        })
    }

    async fn export_rows(
        &self,
        table: &str,
        limit: Option<u64>,
        sink: &mut (dyn FnMut(Vec<Value>) -> Result<(), String> + Send),
    ) -> Result<u64, String> {
        let database = self.current_database().await;
        let mut sql = format!("SELECT * FROM {}", qualified_identifier(&database, table));
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        let mut stream = sqlx::query(sqlx::AssertSqlSafe(sql)).fetch(&self.pool);
        let mut count = 0u64;
        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|error| format!("导出查询失败: {error}"))?
        {
            let values = (0..row.len())
                .map(|index| value_to_json(&row, index))
                .collect();
            sink(values)?;
            count += 1;
        }
        Ok(count)
    }

    async fn read_rows_sorted(
        &self,
        table: &str,
        limit: u64,
        offset: u64,
        filter: Option<&str>,
        sorts: &[crate::models::TableSort],
    ) -> Result<TableRows, String> {
        let database = self.current_database().await;
        let columns = self.list_columns(table).await?;
        let identifier = qualified_identifier(&database, table);
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());
        if filter.is_some_and(|value| value.contains(';')) {
            return Err("WHERE 条件不能包含分号".to_string());
        }
        let where_clause = filter
            .map(|value| format!(" WHERE {value}"))
            .unwrap_or_default();
        let count_sql = format!("SELECT COUNT(*) FROM {identifier}{where_clause}");
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql))
            .fetch_one(&self.pool)
            .await
            .map_err(|error| format!("读取总行数失败: {error}"))?;

        let mut order_columns: Vec<(String, String)> = Vec::new();
        for sort in sorts {
            if !columns.iter().any(|column| column.name == sort.column) {
                return Err(format!("排序字段不存在: {}", sort.column));
            }
            if order_columns
                .iter()
                .any(|(column, _)| column == &sort.column)
            {
                continue;
            }
            order_columns.push((
                sort.column.clone(),
                if sort.direction.eq_ignore_ascii_case("desc") {
                    "DESC".to_string()
                } else {
                    "ASC".to_string()
                },
            ));
        }
        if !order_columns.is_empty() {
            for column in columns.iter().filter(|column| column.primary_key) {
                if !order_columns.iter().any(|(name, _)| name == &column.name) {
                    order_columns.push((column.name.clone(), "ASC".to_string()));
                }
            }
        }
        let order_clause = if order_columns.is_empty() {
            String::new()
        } else {
            format!(
                " ORDER BY {}",
                order_columns
                    .iter()
                    .map(|(column, direction)| {
                        format!("{} {direction}", quote_identifier(column))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let sql =
            format!("SELECT * FROM {identifier}{where_clause}{order_clause} LIMIT ? OFFSET ?");
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("读取表数据失败: {error}"))?;
        let values = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|index| value_to_json(row, index))
                    .collect()
            })
            .collect();
        Ok(TableRows {
            columns,
            rows: values,
            total: total.max(0) as u64,
            offset,
            limit,
        })
    }

    async fn apply_table_changes(
        &self,
        table: &str,
        changes: &[crate::models::PendingTableChange],
    ) -> Result<usize, String> {
        let database = self.current_database().await;
        let table_ref = qualified_identifier(&database, table);
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| format!("开启批量事务失败: {error}"))?;
        let mut applied = 0usize;
        for (index, change) in changes.iter().enumerate() {
            let label = format!("第 {} 项（{}）", index + 1, change.kind);
            let result: Result<u64, String> = match change.kind.as_str() {
                "delete" => {
                    if change.keys.is_empty() {
                        Err("删除缺少主键条件".to_string())
                    } else {
                        let where_clause = change
                            .keys
                            .iter()
                            .map(|key| {
                                if key.value.is_null() {
                                    format!("{} IS NULL", quote_identifier(&key.column))
                                } else {
                                    format!("{} = ?", quote_identifier(&key.column))
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" AND ");
                        let sql = format!(
                            "DELETE FROM {table_ref} WHERE {where_clause}{}",
                            if change.single_row { " LIMIT 1" } else { "" }
                        );
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_mysql(query, &key.value);
                            }
                        }
                        query
                            .execute(&mut *transaction)
                            .await
                            .map(|result| result.rows_affected())
                            .map_err(|error| format!("删除失败: {error}"))
                    }
                }
                "update" => {
                    if change.keys.is_empty() {
                        Err("更新缺少主键条件".to_string())
                    } else if change.changes.is_empty() {
                        Err("更新没有字段变更".to_string())
                    } else {
                        let set_clause = change
                            .changes
                            .iter()
                            .map(|item| format!("{} = ?", quote_identifier(&item.column)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let where_clause = change
                            .keys
                            .iter()
                            .map(|key| {
                                if key.value.is_null() {
                                    format!("{} IS NULL", quote_identifier(&key.column))
                                } else {
                                    format!("{} = ?", quote_identifier(&key.column))
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" AND ");
                        let sql = format!(
                            "UPDATE {table_ref} SET {set_clause} WHERE {where_clause}{}",
                            if change.single_row { " LIMIT 1" } else { "" }
                        );
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for item in &change.changes {
                            query = bind_json_mysql(query, &item.value);
                        }
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_mysql(query, &key.value);
                            }
                        }
                        query
                            .execute(&mut *transaction)
                            .await
                            .map(|result| result.rows_affected())
                            .map_err(|error| format!("更新失败: {error}"))
                    }
                }
                "insert" => {
                    let sql = if change.values.is_empty() {
                        format!("INSERT INTO {table_ref} () VALUES ()")
                    } else {
                        let fields = change
                            .values
                            .iter()
                            .map(|item| quote_identifier(&item.column))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let placeholders = vec!["?"; change.values.len()].join(", ");
                        format!("INSERT INTO {table_ref} ({fields}) VALUES ({placeholders})")
                    };
                    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for item in &change.values {
                        query = bind_json_mysql(query, &item.value);
                    }
                    query
                        .execute(&mut *transaction)
                        .await
                        .map(|result| result.rows_affected())
                        .map_err(|error| format!("新增失败: {error}"))
                }
                other => Err(format!("未知变更类型: {other}")),
            };
            match result {
                Ok(rows) => {
                    let needs_one = matches!(change.kind.as_str(), "delete" | "update");
                    if needs_one && rows != 1 {
                        let _ = transaction.rollback().await;
                        return Err(format!(
                            "{label} 目标行已不存在或已变化（影响 {rows} 行），已回滚"
                        ));
                    }
                    applied += 1;
                }
                Err(error) => {
                    let _ = transaction.rollback().await;
                    return Err(format!("{label} {error}，已回滚"));
                }
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交批量事务失败: {error}"))?;
        Ok(applied)
    }

    async fn dump_insert_plan(&self, table: &str) -> Result<(Vec<String>, String), String> {
        let database = self.current_database().await;
        let generated: HashSet<String> = sqlx::query_scalar(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = ? AND table_name = ? \
               AND UPPER(extra) LIKE '%GENERATED%'",
        )
        .bind(&database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取生成列失败: {error}"))?
        .into_iter()
        .collect();
        let columns = self
            .list_columns(table)
            .await?
            .into_iter()
            .filter(|column| !generated.contains(&column.name))
            .map(|column| column.name)
            .collect();
        Ok((columns, String::new()))
    }

    async fn stream_sql_literal_rows(
        &self,
        table: &str,
        columns: &[String],
        sink: &mut (dyn FnMut(Vec<Option<String>>) -> Result<(), String> + Send),
    ) -> Result<u64, String> {
        if columns.is_empty() {
            return Ok(0);
        }
        let database = self.current_database().await;
        let metadata = self.list_columns(table).await?;
        let expressions = columns
            .iter()
            .map(|name| {
                metadata
                    .iter()
                    .find(|column| &column.name == name)
                    .map(mysql_dump_expression)
                    .unwrap_or_else(|| format!("QUOTE({})", quote_identifier(name)))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {expressions} FROM {}",
            qualified_identifier(&database, table)
        );
        let mut stream = sqlx::query(sqlx::AssertSqlSafe(sql)).fetch(&self.pool);
        let mut count = 0u64;
        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|error| format!("读取表数据失败: {error}"))?
        {
            let values = (0..columns.len())
                .map(|index| row.try_get::<Option<String>, _>(index).ok().flatten())
                .collect::<Vec<_>>();
            sink(values)?;
            count += 1;
        }
        Ok(count)
    }

    async fn dump_object_drop(&self, kind: &str, name: &str) -> Result<String, String> {
        let keyword = match kind {
            "table" => "TABLE",
            "view" => "VIEW",
            "procedure" => "PROCEDURE",
            "function" => "FUNCTION",
            "trigger" => "TRIGGER",
            "event" => "EVENT",
            "sequence" => "SEQUENCE",
            _ => return Err(format!("MySQL 不支持删除该对象类型: {kind}")),
        };
        Ok(format!(
            "DROP {keyword} IF EXISTS {};",
            quote_identifier(name)
        ))
    }

    async fn update_row(
        &self,
        table: &str,
        keys: &[ColumnValue],
        changes: &[ColumnValue],
        single_row: bool,
    ) -> Result<u64, String> {
        if keys.is_empty() {
            return Err("该表没有可用的行标识，无法安全编辑".to_string());
        }
        if changes.is_empty() {
            return Ok(0);
        }

        let schema = self.current_database().await;
        let set_clause = changes
            .iter()
            .map(|item| format!("{} = ?", quote_identifier(&item.column)))
            .collect::<Vec<_>>()
            .join(", ");
        let key_clause = keys
            .iter()
            .map(|item| {
                if item.value.is_null() {
                    format!("{} IS NULL", quote_identifier(&item.column))
                } else {
                    format!("{} = ?", quote_identifier(&item.column))
                }
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "UPDATE {} SET {set_clause} WHERE {key_clause}{}",
            qualified_identifier(&schema, table),
            if single_row { " LIMIT 1" } else { "" }
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for change in changes {
            query = bind_json_mysql(query, &change.value);
        }
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_mysql(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("更新行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn insert_row(&self, table: &str, values: &[ColumnValue]) -> Result<u64, String> {
        let database = self.current_database().await;
        let identifier = qualified_identifier(&database, table);
        let sql = if values.is_empty() {
            format!("INSERT INTO {identifier} () VALUES ()")
        } else {
            let fields = values
                .iter()
                .map(|item| quote_identifier(&item.column))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = vec!["?"; values.len()].join(", ");
            format!("INSERT INTO {identifier} ({fields}) VALUES ({placeholders})")
        };

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for value in values {
            query = bind_json_mysql(query, &value.value);
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("插入行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn delete_row(
        &self,
        table: &str,
        keys: &[ColumnValue],
        single_row: bool,
    ) -> Result<u64, String> {
        if keys.is_empty() {
            return Err("该表没有可用的行标识，无法安全删除".to_string());
        }

        let schema = self.current_database().await;
        let where_clause = keys
            .iter()
            .map(|item| {
                if item.value.is_null() {
                    format!("{} IS NULL", quote_identifier(&item.column))
                } else {
                    format!("{} = ?", quote_identifier(&item.column))
                }
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "DELETE FROM {} WHERE {where_clause}{}",
            qualified_identifier(&schema, table),
            if single_row { " LIMIT 1" } else { "" }
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_mysql(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn object_ddl(&self, kind: &str, name: &str) -> Result<String, String> {
        let database = self.current_database().await;
        let qualified = qualified_identifier(&database, name);
        let (query, value_index) = match kind {
            "table" => (format!("SHOW CREATE TABLE {qualified}"), 1),
            "view" => (format!("SHOW CREATE VIEW {qualified}"), 1),
            "procedure" => (format!("SHOW CREATE PROCEDURE {qualified}"), 2),
            "function" => (format!("SHOW CREATE FUNCTION {qualified}"), 2),
            "trigger" => (format!("SHOW CREATE TRIGGER {qualified}"), 2),
            "event" => (format!("SHOW CREATE EVENT {qualified}"), 3),
            "sequence" => (format!("SHOW CREATE SEQUENCE {qualified}"), 1),
            _ => return Err("MySQL 不支持读取该对象类型的 DDL".to_string()),
        };

        let rows = sqlx::raw_sql(sqlx::AssertSqlSafe(query))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("读取 DDL 失败（数据库 {database} · {name}）: {error}"))?;

        rows.first()
            .and_then(|row| row_text(row, value_index))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("没有可用的 DDL（数据库 {database} · {name}）"))
    }

    async fn create_database(
        &self,
        name: &str,
        charset: Option<&str>,
        collation: Option<&str>,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("数据库名称不能为空".to_string());
        }
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "information_schema" | "mysql" | "performance_schema" | "sys"
        ) {
            return Err("不能创建或覆盖系统数据库".to_string());
        }
        let charset = charset.unwrap_or("utf8mb4").trim().to_ascii_lowercase();
        const CHARSETS: &[&str] = &[
            "utf8mb4", "utf8", "utf8mb3", "latin1", "gbk", "big5", "binary",
        ];
        if !CHARSETS.contains(&charset.as_str()) {
            return Err("不支持的字符集".to_string());
        }
        let mut sql = format!(
            "CREATE DATABASE {} CHARACTER SET {charset}",
            quote_identifier(name)
        );
        if let Some(collation) = collation.map(str::trim).filter(|value| !value.is_empty()) {
            if !collation
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                return Err("排序规则只能包含字母、数字和下划线".to_string());
            }
            sql.push_str(&format!(" COLLATE {collation}"));
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("创建数据库失败: {error}"))?;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("数据库名称不能为空".to_string());
        }
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "information_schema" | "mysql" | "performance_schema" | "sys"
        ) {
            return Err("不能删除系统数据库".to_string());
        }
        if self.current_database().await == name {
            self.switch_database("mysql").await?;
        }
        let sql = format!("DROP DATABASE {}", quote_identifier(name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除数据库失败: {error}"))?;
        Ok(())
    }

    async fn save_object_definition(
        &self,
        kind: &str,
        name: &str,
        definition: &str,
    ) -> Result<(), String> {
        if kind != "view" {
            return Err("MySQL 仅支持在应用内修改视图定义".to_string());
        }
        // Refuse to overwrite a view that was dropped or renamed since the
        // dialog was opened.
        let _ = self.object_ddl("view", name).await?;
        let definition = definition.trim().trim_end_matches(';').trim();
        let upper = definition.to_ascii_uppercase();
        if !upper.starts_with("CREATE ") {
            return Err("视图定义必须以 CREATE ... VIEW 开头".to_string());
        }
        let sql = if upper.starts_with("CREATE OR REPLACE ") {
            definition.to_string()
        } else {
            format!(
                "CREATE OR REPLACE {}",
                definition["CREATE ".len()..].trim_start()
            )
        };
        self.execute(&sql, 1).await.map(|_| ())
    }

    async fn duplicate_table(&self, source: &str, target: &str) -> Result<(), String> {
        let database = self.current_database().await;
        let source = qualified_identifier(&database, source);
        let target = qualified_identifier(&database, target);

        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE {target} LIKE {source}"
        )))
        .execute(&self.pool)
        .await
        .map_err(|error| format!("创建表失败: {error}"))?;

        let copy_sql = format!("INSERT INTO {target} SELECT * FROM {source}");
        if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(copy_sql))
            .execute(&self.pool)
            .await
        {
            let _ = sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TABLE {target}")))
                .execute(&self.pool)
                .await;
            return Err(format!("复制数据失败: {error}"));
        }
        Ok(())
    }

    async fn duplicate_view(&self, source: &str, target_name: &str) -> Result<(), String> {
        let ddl = self.object_ddl("view", source).await?;
        let ddl = super::driver::strip_mysql_definer(&ddl);
        let database = self.current_database().await;
        let replacement = qualified_identifier(&database, target_name);
        let ddl = super::driver::rename_ddl_object(&ddl, &replacement)?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("创建视图失败: {error}"))?;
        Ok(())
    }

    async fn snapshot_view(
        &self,
        source: &str,
        target: &str,
        with_data: bool,
    ) -> Result<(), String> {
        let database = self.current_database().await;
        let source = qualified_identifier(&database, source);
        let target = qualified_identifier(&database, target);
        let filter = if with_data { "" } else { " WHERE 0" };
        let sql = format!("CREATE TABLE {target} AS SELECT * FROM {source}{filter}");
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("快照视图失败: {error}"))?;
        Ok(())
    }

    async fn rename_object(&self, kind: &str, name: &str, new_name: &str) -> Result<(), String> {
        if kind != "table" && kind != "view" {
            return Err("MySQL 仅支持重命名表或视图".to_string());
        }
        let database = self.current_database().await;
        let sql = format!(
            "RENAME TABLE {} TO {}",
            qualified_identifier(&database, name),
            qualified_identifier(&database, new_name)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("重命名失败: {error}"))?;
        Ok(())
    }

    async fn drop_object(&self, kind: &str, name: &str) -> Result<(), String> {
        let keyword = match kind {
            "table" => "TABLE",
            "view" => "VIEW",
            "procedure" => "PROCEDURE",
            "function" => "FUNCTION",
            "trigger" => "TRIGGER",
            "event" => "EVENT",
            "sequence" => "SEQUENCE",
            _ => return Err("MySQL 不支持删除该对象类型".to_string()),
        };
        let database = self.current_database().await;
        let sql = format!("DROP {keyword} {}", qualified_identifier(&database, name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除对象失败: {error}"))?;
        Ok(())
    }
}

async fn mysql_indexes(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, String> {
    let rows = sqlx::query(
        "SELECT index_name, CAST(non_unique AS SIGNED) AS non_unique, column_name \
         FROM information_schema.statistics \
         WHERE table_schema = ? AND table_name = ? \
         ORDER BY index_name, seq_in_index",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取索引失败: {error}"))?;

    let mut indexes: BTreeMap<String, IndexInfo> = BTreeMap::new();
    for row in rows {
        let name: String = row.try_get(0).unwrap_or_default();
        let non_unique: i64 = row.try_get(1).unwrap_or(1);
        let column: String = row.try_get(2).unwrap_or_default();

        let entry = indexes.entry(name.clone()).or_insert_with(|| IndexInfo {
            primary: name.eq_ignore_ascii_case("PRIMARY"),
            name: name.clone(),
            columns: Vec::new(),
            unique: non_unique == 0,
        });
        entry.columns.push(column);
    }

    Ok(indexes.into_values().collect())
}

async fn mysql_foreign_keys(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, String> {
    let rows = sqlx::query(
        "SELECT k.constraint_name, k.column_name, k.referenced_table_name, \
                k.referenced_column_name, r.update_rule, r.delete_rule \
         FROM information_schema.key_column_usage k \
         JOIN information_schema.referential_constraints r \
           ON r.constraint_schema = k.constraint_schema \
          AND r.constraint_name = k.constraint_name \
         WHERE k.table_schema = ? AND k.table_name = ? \
           AND k.referenced_table_name IS NOT NULL \
         ORDER BY k.constraint_name, k.ordinal_position",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取外键失败: {error}"))?;

    let mut foreign_keys: BTreeMap<String, ForeignKeyInfo> = BTreeMap::new();
    for row in rows {
        let name: String = row.try_get(0).unwrap_or_default();
        let column: String = row.try_get(1).unwrap_or_default();
        let referenced_table: String = row.try_get(2).unwrap_or_default();
        let referenced_column: Option<String> = row.try_get(3).ok();
        let on_update: String = row.try_get(4).unwrap_or_default();
        let on_delete: String = row.try_get(5).unwrap_or_default();

        let entry = foreign_keys
            .entry(name.clone())
            .or_insert_with(|| ForeignKeyInfo {
                name,
                columns: Vec::new(),
                referenced_table,
                referenced_columns: Vec::new(),
                on_delete,
                on_update,
            });
        entry.columns.push(column);
        if let Some(referenced_column) = referenced_column {
            entry.referenced_columns.push(referenced_column);
        }
    }

    Ok(foreign_keys.into_values().collect())
}

fn quote_identifier(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

fn mysql_dump_expression(column: &ColumnInfo) -> String {
    let name = quote_identifier(&column.name);
    let data_type = column.data_type.trim().to_ascii_lowercase();
    let base = data_type
        .split(|character: char| character == '(' || character.is_whitespace())
        .next()
        .unwrap_or("");
    if base == "bit" {
        return format!("CAST({name} AS UNSIGNED)");
    }
    if matches!(
        base,
        "binary"
            | "varbinary"
            | "blob"
            | "tinyblob"
            | "mediumblob"
            | "longblob"
            | "geometry"
            | "point"
            | "linestring"
            | "polygon"
            | "multipoint"
            | "multilinestring"
            | "multipolygon"
            | "geometrycollection"
    ) {
        return format!("CONCAT(\"X'\", HEX({name}), \"'\")");
    }
    if matches!(
        base,
        "tinyint"
            | "smallint"
            | "mediumint"
            | "int"
            | "integer"
            | "bigint"
            | "decimal"
            | "numeric"
            | "float"
            | "double"
            | "real"
            | "year"
            | "bool"
            | "boolean"
    ) {
        return format!("CAST({name} AS CHAR)");
    }
    format!("QUOTE({name})")
}

fn qualified_identifier(database: &str, name: &str) -> String {
    format!("{}.{}", quote_identifier(database), quote_identifier(name))
}

/// SHOW CREATE results can arrive as TEXT or BLOB and may use the server's
/// connection charset; fall back to a lossy UTF-8 conversion so DDL never
/// disappears because of a strict decode.
fn row_text(row: &MySqlRow, index: usize) -> Option<String> {
    if let Ok(Some(value)) = row.try_get::<Option<String>, _>(index) {
        return Some(value);
    }
    if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(index) {
        return Some(String::from_utf8_lossy(&bytes).to_string());
    }
    None
}

fn bind_json_mysql<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, MySqlArguments>,
    value: &'q Value,
) -> sqlx::query::Query<'q, sqlx::MySql, MySqlArguments> {
    match value {
        Value::Null => query.bind(Option::<String>::None),
        Value::Bool(value) => query.bind(*value),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                query.bind(value)
            } else if let Some(value) = number.as_f64() {
                query.bind(value)
            } else {
                query.bind(number.to_string())
            }
        }
        Value::String(value) => query.bind(value.as_str()),
        other => query.bind(other.to_string()),
    }
}

fn value_to_json(row: &MySqlRow, index: usize) -> Value {
    let type_name = row.column(index).type_info().name().to_ascii_uppercase();

    match type_name.as_str() {
        "BOOLEAN" => row
            .try_get::<Option<bool>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED"
        | "BIGINT UNSIGNED" | "YEAR" => row
            .try_get::<Option<u64>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "FLOAT" | "DOUBLE" => row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "DECIMAL" => row
            .try_get::<Option<BigDecimal>, _>(index)
            .ok()
            .flatten()
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
            row.try_get::<Option<String>, _>(index)
                .ok()
                .flatten()
                .map(Value::from)
                .unwrap_or(Value::Null)
        }
        "DATE" => row
            .try_get::<Option<NaiveDate>, _>(index)
            .ok()
            .flatten()
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "DATETIME" | "TIMESTAMP" => row
            .try_get::<Option<NaiveDateTime>, _>(index)
            .ok()
            .flatten()
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "TIME" => row
            .try_get::<Option<NaiveTime>, _>(index)
            .ok()
            .flatten()
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "JSON" => row
            .try_get::<Option<serde_json::Value>, _>(index)
            .ok()
            .flatten()
            .unwrap_or(Value::Null),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" | "GEOMETRY" => {
            row.try_get::<Option<Vec<u8>>, _>(index)
                .ok()
                .flatten()
                .map(|bytes| Value::String(format!("<BLOB {} bytes>", bytes.len())))
                .unwrap_or(Value::Null)
        }
        "BIT" => {
            if let Ok(Some(value)) = row.try_get::<Option<u64>, _>(index) {
                return Value::from(value);
            }
            row.try_get::<Option<Vec<u8>>, _>(index)
                .ok()
                .flatten()
                .map(|bytes| Value::String(format!("<BLOB {} bytes>", bytes.len())))
                .unwrap_or(Value::Null)
        }
        _ => {
            if let Ok(Some(value)) = row.try_get::<Option<i64>, _>(index) {
                return Value::from(value);
            }
            if let Ok(Some(value)) = row.try_get::<Option<u64>, _>(index) {
                return Value::from(value);
            }
            if let Ok(Some(value)) = row.try_get::<Option<f64>, _>(index) {
                return Value::from(value);
            }
            if let Ok(Some(value)) = row.try_get::<Option<String>, _>(index) {
                return Value::from(value);
            }
            Value::Null
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_identifiers_and_qualifies() {
        assert_eq!(quote_identifier("a`b"), "`a``b`");
        assert_eq!(qualified_identifier("my db", "t"), "`my db`.`t`");
    }

    #[test]
    fn builds_dump_literals_for_binary_numeric_and_text() {
        let column = |name: &str, data_type: &str| ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            nullable: true,
            primary_key: false,
            auto_increment: false,
            default_value: None,
            comment: None,
        };
        assert_eq!(
            mysql_dump_expression(&column("payload", "longblob")),
            "CONCAT(\"X'\", HEX(`payload`), \"'\")"
        );
        assert_eq!(
            mysql_dump_expression(&column("amount", "decimal(10,2)")),
            "CAST(`amount` AS CHAR)"
        );
        assert_eq!(
            mysql_dump_expression(&column("name", "varchar(255)")),
            "QUOTE(`name`)"
        );
        assert_eq!(
            mysql_dump_expression(&column("flags", "bit(8)")),
            "CAST(`flags` AS UNSIGNED)"
        );
    }

    /// Live regression for MySQL/MariaDB. Skips unless `MYDB_TEST_MYSQL_HOST`
    /// is set. Set `MYDB_TEST_MYSQL_DB` / `MYDB_TEST_MYSQL_TABLE` to exercise
    /// a specific table, and either `MYDB_TEST_MYSQL_PASSWORD` or
    /// `MYDB_TEST_MYSQL_KEYCHAIN_ID` (the saved connection id) for auth.
    #[tokio::test]
    async fn live_structure_and_ddl() {
        let Ok(host) = std::env::var("MYDB_TEST_MYSQL_HOST") else {
            return;
        };
        let port: u16 = std::env::var("MYDB_TEST_MYSQL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3306);
        let username = std::env::var("MYDB_TEST_MYSQL_USER").unwrap_or_else(|_| "root".to_string());
        let database =
            std::env::var("MYDB_TEST_MYSQL_CONFIG_DB").unwrap_or_else(|_| "mysql".to_string());
        let target_db = std::env::var("MYDB_TEST_MYSQL_DB").unwrap_or_else(|_| database.clone());
        let table = std::env::var("MYDB_TEST_MYSQL_TABLE").ok();

        let password = match std::env::var("MYDB_TEST_MYSQL_PASSWORD") {
            Ok(value) => Some(value),
            Err(_) => match std::env::var("MYDB_TEST_MYSQL_KEYCHAIN_ID") {
                Ok(connection_id) => {
                    keyring::Entry::new("mydb", &format!("connection:{connection_id}:db"))
                        .ok()
                        .and_then(|entry| entry.get_password().ok())
                }
                Err(_) => None,
            },
        };

        let config = ConnectionConfig {
            driver: "mysql".to_string(),
            host: host.clone(),
            port,
            username,
            database,
            ..Default::default()
        };
        let driver = MySqlDriver::connect(&config, &host, port, password.as_deref(), None)
            .await
            .expect("connect");
        driver
            .switch_database(&target_db)
            .await
            .expect("switch database");

        let table = match table {
            Some(table) => table,
            None => {
                let objects = driver.list_tables().await.expect("list tables");
                objects
                    .into_iter()
                    .find(|item| item.kind == "table")
                    .map(|item| item.name)
                    .expect("no table in target database")
            }
        };

        let structure = driver
            .table_structure(&table)
            .await
            .expect("table structure");
        assert!(!structure.columns.is_empty(), "structure has no columns");
        assert!(!structure.ddl.trim().is_empty(), "structure has no DDL");

        let ddl = driver
            .object_ddl("table", &table)
            .await
            .expect("object ddl");
        assert!(ddl.to_uppercase().contains("CREATE TABLE"));
    }
}
