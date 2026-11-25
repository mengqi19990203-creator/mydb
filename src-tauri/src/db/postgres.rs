use crate::db::query::is_read_query;
use crate::db::{DatabaseDriver, DatabaseSession};
use crate::models::{
    ColumnInfo, ColumnValue, ConnectionConfig, ForeignKeyInfo, IndexInfo, QueryResult,
    ScopeTableColumns, TableInfo, TableRows, TableStructure,
};
use crate::ssh::SshTunnel;
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures_util::TryStreamExt;
use serde_json::Value;
use sqlx::postgres::types::{Oid, PgInterval};
use sqlx::postgres::{
    PgArguments, PgConnectOptions, PgPoolOptions, PgRow, PgSslMode, PgValueFormat,
};
use sqlx::{Column, Connection, PgConnection, PgPool, Postgres, Row, TypeInfo, ValueRef};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;
use tokio::sync::RwLock;

/// PostgreSQL connection scoped to one database and one schema.
///
/// PostgreSQL cannot switch databases on an existing connection, so the
/// frontend rebuilds the driver through `switch_database`; schemas are cheap
/// to switch and are stored inside the driver behind a lock.
pub struct PostgresDriver {
    pool: PgPool,
    database: String,
    schema: RwLock<String>,
    #[allow(dead_code)]
    tunnel: Option<SshTunnel>,
}

impl PostgresDriver {
    pub async fn connect(
        config: &ConnectionConfig,
        host: &str,
        port: u16,
        password: Option<&str>,
        tunnel: Option<SshTunnel>,
    ) -> Result<Self, String> {
        let ssl_mode = match config.ssl_mode.as_str() {
            "preferred" => PgSslMode::Prefer,
            "required" => PgSslMode::Require,
            "verify-ca" => PgSslMode::VerifyCa,
            "verify-full" => PgSslMode::VerifyFull,
            _ => PgSslMode::Disable,
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
            "postgres"
        } else {
            config.database.trim()
        };

        let mut options = PgConnectOptions::new()
            .host(host)
            .port(if port == 0 { 5432 } else { port })
            .username(&config.username)
            .password(password.unwrap_or_default())
            .database(database)
            .ssl_mode(ssl_mode)
            .application_name("mydb");
        if !ca_path.is_empty() {
            options = options.ssl_root_cert(ca_path);
        }
        if !client_cert.is_empty() {
            options = options
                .ssl_client_cert(client_cert)
                .ssl_client_key(client_key);
        }

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|error| format!("连接 PostgreSQL 失败: {error}"))?;

        Ok(Self {
            pool,
            database: database.to_string(),
            schema: RwLock::new("public".to_string()),
            tunnel,
        })
    }

    async fn current_schema(&self) -> String {
        self.schema.read().await.clone()
    }
}

async fn execute_on_connection(
    connection: &mut PgConnection,
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

struct PgSession {
    connection: sqlx::pool::PoolConnection<sqlx::Postgres>,
}

#[async_trait]
impl DatabaseSession for PgSession {
    async fn begin(&mut self) -> Result<(), String> {
        sqlx::raw_sql("BEGIN")
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
        let id: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *self.connection)
            .await
            .map_err(|error| format!("读取后端进程 ID 失败: {error}"))?;
        Ok(Some(id as i64))
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn test_connection(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| format!("连接测试失败: {error}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT datname FROM pg_database \
             WHERE datallowconn \
             ORDER BY datname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取数据库列表失败: {error}"))?;
        Ok(rows)
    }

    async fn list_schemas(&self) -> Result<Vec<String>, String> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT nspname FROM pg_namespace \
             WHERE nspname !~ '^pg_' AND nspname <> 'information_schema' \
             ORDER BY nspname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取模式列表失败: {error}"))?;
        Ok(rows)
    }

    async fn switch_schema(&self, schema: &str) -> Result<(), String> {
        let schema = schema.trim();
        if schema.is_empty() {
            return Err("模式名不能为空".to_string());
        }

        let exists: Option<String> =
            sqlx::query_scalar("SELECT nspname FROM pg_namespace WHERE nspname = $1")
                .bind(schema)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| format!("校验模式失败: {error}"))?;

        if exists.is_none() {
            return Err(format!("模式不存在: {schema}"));
        }

        *self.schema.write().await = schema.to_string();
        Ok(())
    }

    async fn context(&self) -> (Option<String>, Option<String>) {
        (
            Some(self.database.clone()),
            Some(self.current_schema().await),
        )
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        let schema = self.current_schema().await;
        let mut objects = Vec::new();

        let relations = sqlx::query(
            "SELECT c.relname, c.relkind::text AS relkind \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relkind IN ('r', 'p', 'f', 'v', 'm', 'S') \
             ORDER BY c.relkind, c.relname",
        )
        .bind(&schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取对象列表失败: {error}"))?;

        for row in relations {
            let name: String = row.try_get(0).unwrap_or_default();
            let relkind: String = row.try_get(1).unwrap_or_default();
            let kind = match relkind.as_str() {
                "v" => "view",
                "m" => "matview",
                "S" => "sequence",
                _ => "table",
            };
            objects.push(TableInfo {
                name,
                kind: kind.to_string(),
            });
        }

        if let Ok(routines) = sqlx::query(
            "SELECT DISTINCT p.proname, p.prokind::text AS prokind \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             WHERE n.nspname = $1 AND p.prokind IN ('f', 'p') \
             ORDER BY p.prokind, p.proname",
        )
        .bind(&schema)
        .fetch_all(&self.pool)
        .await
        {
            objects.extend(routines.into_iter().map(|row| {
                let name: String = row.try_get(0).unwrap_or_default();
                let prokind: String = row.try_get(1).unwrap_or_default();
                TableInfo {
                    name,
                    kind: if prokind == "p" {
                        "procedure".to_string()
                    } else {
                        "function".to_string()
                    },
                }
            }));
        }

        if let Ok(triggers) = sqlx::query(
            "SELECT DISTINCT t.tgname \
             FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND NOT t.tgisinternal \
             ORDER BY t.tgname",
        )
        .bind(&schema)
        .fetch_all(&self.pool)
        .await
        {
            objects.extend(triggers.into_iter().map(|row| TableInfo {
                name: row.try_get(0).unwrap_or_default(),
                kind: "trigger".to_string(),
            }));
        }

        Ok(objects)
    }

    async fn list_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKeyInfo>, String> {
        let schema = self.current_schema().await;
        pg_foreign_keys(&self.pool, &schema, table).await
    }

    async fn index_table(&self, index_name: &str) -> Result<Option<String>, String> {
        let schema = self.current_schema().await;
        sqlx::query_scalar(
            "SELECT c.relname \
             FROM pg_index ix \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_class c ON c.oid = ix.indrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND i.relname = $2 \
             LIMIT 1",
        )
        .bind(&schema)
        .bind(index_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("读取索引所属表失败: {error}"))
    }

    async fn execute(&self, sql: &str, max_rows: usize) -> Result<QueryResult, String> {
        // Pin one pooled connection so `SET search_path` and the user's SQL
        // run in the same session; unqualified names then follow the schema
        // selected in the object tree.
        let schema = self.current_schema().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let set_schema = format!("SET search_path TO {}", quote_identifier(&schema));
        sqlx::raw_sql(sqlx::AssertSqlSafe(set_schema))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("设置当前模式失败: {error}"))?;

        execute_on_connection(&mut connection, sql, max_rows).await
    }

    async fn open_session(&self) -> Result<Box<dyn DatabaseSession>, String> {
        let schema = self.current_schema().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let set_schema = format!("SET search_path TO {}", quote_identifier(&schema));
        sqlx::raw_sql(sqlx::AssertSqlSafe(set_schema))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("设置当前模式失败: {error}"))?;
        Ok(Box::new(PgSession { connection }))
    }

    async fn cancel_backend_query(&self, backend_id: i64) -> Result<bool, String> {
        let pid =
            i32::try_from(backend_id).map_err(|_| format!("无效的后端进程 ID: {backend_id}"))?;
        sqlx::query_scalar::<_, bool>("SELECT pg_cancel_backend($1)")
            .bind(pid)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| format!("取消查询失败（可能需要同角色或超级用户权限）: {error}"))
    }

    async fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let schema = self.current_schema().await;
        let rows = sqlx::query(
            "SELECT a.attname, \
                    pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type, \
                    NOT a.attnotnull AS nullable, \
                    pg_get_expr(d.adbin, d.adrelid) AS default_value, \
                    col_description(a.attrelid, a.attnum) AS comment, \
                    (a.attidentity IN ('a', 'd') \
                     OR COALESCE(pg_get_expr(d.adbin, d.adrelid), '') LIKE 'nextval(%') \
                      AS auto_increment \
             FROM pg_attribute a \
             JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             WHERE n.nspname = $1 AND c.relname = $2 \
               AND c.relkind IN ('r', 'p', 'f', 'v', 'm') \
               AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY a.attnum",
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取字段信息失败: {error}"))?;

        let primary_keys = pg_primary_key_columns(&self.pool, &schema, table).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let name: String = row.try_get(0).unwrap_or_default();
                ColumnInfo {
                    primary_key: primary_keys.contains(&name),
                    name,
                    data_type: row.try_get(1).unwrap_or_default(),
                    nullable: row.try_get(2).unwrap_or(true),
                    default_value: row.try_get(3).ok().flatten(),
                    auto_increment: row.try_get(5).unwrap_or(false),
                    comment: row
                        .try_get::<Option<String>, _>(4)
                        .ok()
                        .flatten()
                        .filter(|value| !value.trim().is_empty()),
                }
            })
            .collect())
    }

    async fn scope_columns(&self) -> Result<Vec<ScopeTableColumns>, String> {
        let schema = self.current_schema().await;
        let rows = sqlx::query(
            "SELECT c.relname, c.relkind::text AS relkind, a.attname, \
                    pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type, \
                    NOT a.attnotnull AS nullable, \
                    pg_get_expr(d.adbin, d.adrelid) AS default_value, \
                    (a.attidentity IN ('a', 'd') \
                     OR COALESCE(pg_get_expr(d.adbin, d.adrelid), '') LIKE 'nextval(%') \
                      AS auto_increment, \
                    COALESCE(pk.is_pk, false) AS primary_key \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_attribute a ON a.attrelid = c.oid \
               AND a.attnum > 0 AND NOT a.attisdropped \
             LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             LEFT JOIN LATERAL ( \
                 SELECT true AS is_pk \
                 FROM pg_index i \
                 WHERE i.indrelid = c.oid AND i.indisprimary \
                   AND a.attnum = ANY(i.indkey) \
                 LIMIT 1 \
             ) pk ON true \
             WHERE n.nspname = $1 AND c.relkind IN ('r', 'p', 'f', 'v', 'm') \
             ORDER BY c.relname, a.attnum",
        )
        .bind(&schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取补全元数据失败: {error}"))?;

        let mut grouped: BTreeMap<String, (String, Vec<ColumnInfo>)> = BTreeMap::new();
        for row in rows {
            let table_name: String = row.try_get(0).unwrap_or_default();
            let relkind: String = row.try_get(1).unwrap_or_default();
            let kind = match relkind.as_str() {
                "v" => "view",
                "m" => "matview",
                _ => "table",
            };
            let column = ColumnInfo {
                name: row.try_get(2).unwrap_or_default(),
                data_type: row.try_get(3).unwrap_or_default(),
                nullable: row.try_get(4).unwrap_or(true),
                default_value: row.try_get(5).ok().flatten(),
                auto_increment: row.try_get(6).unwrap_or(false),
                primary_key: row.try_get(7).unwrap_or(false),
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
        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;
        let indexes = pg_indexes(&self.pool, &schema, table).await?;
        let foreign_keys = pg_foreign_keys(&self.pool, &schema, table).await?;

        let ddl_row = sqlx::query(
            "SELECT c.relkind::text AS relkind, pg_get_viewdef(c.oid, true) AS viewdef \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relname = $2 \
               AND c.relkind IN ('r', 'p', 'f', 'v', 'm') \
             LIMIT 1",
        )
        .bind(&schema)
        .bind(table)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("读取 DDL 失败: {error}"))?;

        let ddl = match ddl_row {
            Some(row) => {
                let relkind: String = row.try_get(0).unwrap_or_default();
                let viewdef: Option<String> = row.try_get(1).ok().flatten();
                match (relkind.as_str(), viewdef) {
                    ("v", Some(definition)) => {
                        let definition = definition.trim().trim_end_matches(';');
                        format!(
                            "CREATE VIEW {}.{} AS\n{definition};",
                            quote_identifier(&schema),
                            quote_identifier(table)
                        )
                    }
                    ("m", Some(definition)) => {
                        let definition = definition.trim().trim_end_matches(';');
                        format!(
                            "CREATE MATERIALIZED VIEW {}.{} AS\n{definition};",
                            quote_identifier(&schema),
                            quote_identifier(table)
                        )
                    }
                    _ => build_table_ddl(&schema, table, &columns, &indexes, &foreign_keys),
                }
            }
            None => String::new(),
        };

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
        let schema = self.current_schema().await;
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        let set_schema = format!("SET search_path TO {}", quote_identifier(&schema));
        sqlx::raw_sql(sqlx::AssertSqlSafe(set_schema))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("设置当前模式失败: {error}"))?;

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
        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;
        let identifier = qualified_identifier(&schema, table);
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
            format!("SELECT * FROM {identifier}{where_clause}{order_clause} LIMIT $1 OFFSET $2");
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
        let schema = self.current_schema().await;
        let mut sql = format!("SELECT * FROM {}", qualified_identifier(&schema, table));
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
        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;
        let identifier = qualified_identifier(&schema, table);
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
            format!("SELECT * FROM {identifier}{where_clause}{order_clause} LIMIT $1 OFFSET $2");
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
        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;
        let table_ref = qualified_identifier(&schema, table);
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
                        let mut parameter = 0usize;
                        let mut where_parts = Vec::new();
                        for key in &change.keys {
                            if key.value.is_null() {
                                where_parts
                                    .push(format!("{} IS NULL", quote_identifier(&key.column)));
                            } else {
                                parameter += 1;
                                where_parts.push(format!(
                                    "{} = ${}{}",
                                    quote_identifier(&key.column),
                                    parameter,
                                    cast_for(&columns, &key.column)
                                ));
                            }
                        }
                        let key_clause = where_parts.join(" AND ");
                        let where_clause = if change.single_row {
                            format!(
                                "ctid = (SELECT ctid FROM {table_ref} WHERE {key_clause} LIMIT 1)"
                            )
                        } else {
                            key_clause
                        };
                        let sql = format!("DELETE FROM {table_ref} WHERE {where_clause}");
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_postgres(query, &key.value);
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
                        let mut parameter = 0usize;
                        let mut set_parts = Vec::new();
                        for item in &change.changes {
                            parameter += 1;
                            set_parts.push(format!(
                                "{} = ${}{}",
                                quote_identifier(&item.column),
                                parameter,
                                cast_for(&columns, &item.column)
                            ));
                        }
                        let mut where_parts = Vec::new();
                        for key in &change.keys {
                            if key.value.is_null() {
                                where_parts
                                    .push(format!("{} IS NULL", quote_identifier(&key.column)));
                            } else {
                                parameter += 1;
                                where_parts.push(format!(
                                    "{} = ${}{}",
                                    quote_identifier(&key.column),
                                    parameter,
                                    cast_for(&columns, &key.column)
                                ));
                            }
                        }
                        let key_clause = where_parts.join(" AND ");
                        let where_clause = if change.single_row {
                            format!(
                                "ctid = (SELECT ctid FROM {table_ref} WHERE {key_clause} LIMIT 1)"
                            )
                        } else {
                            key_clause
                        };
                        let sql = format!(
                            "UPDATE {table_ref} SET {} WHERE {where_clause}",
                            set_parts.join(", ")
                        );
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for item in &change.changes {
                            query = bind_json_postgres(query, &item.value);
                        }
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_postgres(query, &key.value);
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
                        format!("INSERT INTO {table_ref} DEFAULT VALUES")
                    } else {
                        let fields = change
                            .values
                            .iter()
                            .map(|item| quote_identifier(&item.column))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let placeholders = change
                            .values
                            .iter()
                            .enumerate()
                            .map(|(index, item)| {
                                format!("${}{}", index + 1, cast_for(&columns, &item.column))
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("INSERT INTO {table_ref} ({fields}) VALUES ({placeholders})")
                    };
                    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for item in &change.values {
                        query = bind_json_postgres(query, &item.value);
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

    async fn dump_table_ddl(&self, table: &str) -> Result<String, String> {
        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;
        let indexes = pg_indexes(&self.pool, &schema, table).await?;
        Ok(build_table_ddl(&schema, table, &columns, &indexes, &[]))
    }

    fn dump_indexes_separately(&self) -> bool {
        true
    }

    async fn dump_index_statements(&self, table: &str) -> Result<Vec<String>, String> {
        let schema = self.current_schema().await;
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT pg_get_indexdef(i.indexrelid) \
             FROM pg_index i \
             JOIN pg_class c ON c.oid = i.indrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relname = $2 AND NOT i.indisprimary \
             ORDER BY i.indexrelid",
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取索引定义失败: {error}"))?;
        Ok(rows
            .into_iter()
            .map(|sql| {
                let trimmed = sql.trim().trim_end_matches(';').to_string();
                format!("{trimmed};")
            })
            .collect())
    }

    fn dump_foreign_keys_separately(&self) -> bool {
        true
    }

    async fn dump_insert_plan(&self, table: &str) -> Result<(Vec<String>, String), String> {
        let schema = self.current_schema().await;
        let rows = sqlx::query(
            "SELECT column_name, is_generated, identity_generation \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2",
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取字段生成信息失败: {error}"))?;
        let mut generated = HashSet::new();
        let mut identity_always = false;
        for row in rows {
            let name: String = row.try_get(0).unwrap_or_default();
            let is_generated: String = row.try_get(1).unwrap_or_default();
            let identity: Option<String> = row.try_get(2).ok().flatten();
            if is_generated.eq_ignore_ascii_case("ALWAYS") {
                generated.insert(name);
            }
            if identity
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("ALWAYS"))
            {
                identity_always = true;
            }
        }
        let columns = self
            .list_columns(table)
            .await?
            .into_iter()
            .filter(|column| !generated.contains(&column.name))
            .map(|column| column.name)
            .collect();
        let clause = if identity_always {
            " OVERRIDING SYSTEM VALUE".to_string()
        } else {
            String::new()
        };
        Ok((columns, clause))
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
        let schema = self.current_schema().await;
        let metadata = self.list_columns(table).await?;
        let expressions = columns
            .iter()
            .map(|name| {
                let identifier = quote_identifier(name);
                let is_bytea = metadata
                    .iter()
                    .find(|column| &column.name == name)
                    .map(|column| column.data_type.to_ascii_lowercase().contains("bytea"))
                    .unwrap_or(false);
                if is_bytea {
                    format!(
                        "CASE WHEN {identifier} IS NULL THEN NULL ELSE 'decode(' || \
                         quote_literal(encode({identifier}, 'hex')) || ', ''hex'')' END"
                    )
                } else {
                    format!("quote_literal({identifier}::text)")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {expressions} FROM {}",
            qualified_identifier(&schema, table)
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
        let schema = self.current_schema().await;
        if kind == "trigger" {
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT c.relname \
                 FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = $1 AND t.tgname = $2 AND NOT t.tgisinternal",
            )
            .bind(&schema)
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("查找触发器失败: {error}"))?;
            return match tables.as_slice() {
                [table] => Ok(format!(
                    "DROP TRIGGER IF EXISTS {} ON {};",
                    quote_identifier(name),
                    qualified_identifier(&schema, table)
                )),
                [] => Err(format!("触发器不存在: {name}")),
                _ => Err("同名触发器存在于多张表，无法生成删除语句".to_string()),
            };
        }
        let keyword = match kind {
            "table" => "TABLE",
            "view" => "VIEW",
            "matview" => "MATERIALIZED VIEW",
            "sequence" => "SEQUENCE",
            "function" | "procedure" => "ROUTINE",
            _ => return Err(format!("PostgreSQL 不支持删除该对象类型: {kind}")),
        };
        Ok(format!(
            "DROP {keyword} IF EXISTS {};",
            qualified_identifier(&schema, name)
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

        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;

        let set_clause = changes
            .iter()
            .enumerate()
            .map(|(index, item)| {
                format!(
                    "{} = ${}{}",
                    quote_identifier(&item.column),
                    index + 1,
                    cast_for(&columns, &item.column)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut parameter = changes.len();
        let key_clause = keys
            .iter()
            .map(|item| {
                if item.value.is_null() {
                    format!("{} IS NULL", quote_identifier(&item.column))
                } else {
                    parameter += 1;
                    format!(
                        "{} = ${}{}",
                        quote_identifier(&item.column),
                        parameter,
                        cast_for(&columns, &item.column)
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let table_ref = qualified_identifier(&schema, table);
        let where_clause = if single_row {
            format!("ctid = (SELECT ctid FROM {table_ref} WHERE {key_clause} LIMIT 1)")
        } else {
            key_clause
        };
        let sql = format!("UPDATE {table_ref} SET {set_clause} WHERE {where_clause}");

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for change in changes {
            query = bind_json_postgres(query, &change.value);
        }
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_postgres(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("更新行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn insert_row(&self, table: &str, values: &[ColumnValue]) -> Result<u64, String> {
        let schema = self.current_schema().await;
        let identifier = qualified_identifier(&schema, table);
        let columns = self.list_columns(table).await?;

        let sql = if values.is_empty() {
            format!("INSERT INTO {identifier} DEFAULT VALUES")
        } else {
            let fields = values
                .iter()
                .map(|item| quote_identifier(&item.column))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = values
                .iter()
                .enumerate()
                .map(|(index, item)| format!("${}{}", index + 1, cast_for(&columns, &item.column)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("INSERT INTO {identifier} ({fields}) VALUES ({placeholders})")
        };

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for value in values {
            query = bind_json_postgres(query, &value.value);
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

        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;

        let mut parameter = 0;
        let key_clause = keys
            .iter()
            .map(|item| {
                if item.value.is_null() {
                    format!("{} IS NULL", quote_identifier(&item.column))
                } else {
                    parameter += 1;
                    format!(
                        "{} = ${}{}",
                        quote_identifier(&item.column),
                        parameter,
                        cast_for(&columns, &item.column)
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let table_ref = qualified_identifier(&schema, table);
        let where_clause = if single_row {
            format!("ctid = (SELECT ctid FROM {table_ref} WHERE {key_clause} LIMIT 1)")
        } else {
            key_clause
        };
        let sql = format!("DELETE FROM {table_ref} WHERE {where_clause}");

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_postgres(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn object_ddl(&self, kind: &str, name: &str) -> Result<String, String> {
        let schema = self.current_schema().await;
        match kind {
            "table" | "view" | "matview" => Ok(self.table_structure(name).await?.ddl),
            "function" | "procedure" => {
                let prokind = if kind == "function" { "f" } else { "p" };
                let ddl: Option<String> = sqlx::query_scalar(
                    "SELECT pg_get_functiondef(p.oid) \
                     FROM pg_proc p \
                     JOIN pg_namespace n ON n.oid = p.pronamespace \
                     WHERE n.nspname = $1 AND p.proname = $2 \
                       AND p.prokind::text = $3 \
                     ORDER BY p.oid LIMIT 1",
                )
                .bind(&schema)
                .bind(name)
                .bind(prokind)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| format!("读取 DDL 失败: {error}"))?;

                ddl.ok_or_else(|| format!("没有可用的 DDL: {name}"))
            }
            "trigger" => {
                let ddl: Option<String> = sqlx::query_scalar(
                    "SELECT pg_get_triggerdef(t.oid) \
                     FROM pg_trigger t \
                     JOIN pg_class c ON c.oid = t.tgrelid \
                     JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE n.nspname = $1 AND t.tgname = $2 AND NOT t.tgisinternal \
                     ORDER BY t.oid LIMIT 1",
                )
                .bind(&schema)
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| format!("读取 DDL 失败: {error}"))?;

                ddl.ok_or_else(|| format!("没有可用的 DDL: {name}"))
            }
            "index" => {
                let ddl: Option<String> = sqlx::query_scalar(
                    "SELECT pg_get_indexdef(i.indexrelid) \
                     FROM pg_class i \
                     JOIN pg_namespace n ON n.oid = i.relnamespace \
                     WHERE n.nspname = $1 AND i.relname = $2 AND i.relkind = 'i' \
                     LIMIT 1",
                )
                .bind(&schema)
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| format!("读取 DDL 失败: {error}"))?;

                ddl.ok_or_else(|| format!("没有可用的 DDL: {name}"))
            }
            "sequence" => {
                let ddl: Option<String> = sqlx::query_scalar(
                    "SELECT format(\
                       'CREATE SEQUENCE %I.%I INCREMENT BY %s MINVALUE %s MAXVALUE %s START WITH %s CACHE %s%s;', \
                       n.nspname, c.relname, s.seqincrement, s.seqmin, s.seqmax, \
                       s.seqstart, s.seqcache, \
                       CASE WHEN s.seqcycle THEN ' CYCLE' ELSE '' END) \
                     FROM pg_class c \
                     JOIN pg_namespace n ON n.oid = c.relnamespace \
                     JOIN pg_sequence s ON s.seqrelid = c.oid \
                     WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind = 'S'",
                )
                .bind(&schema)
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| format!("读取 DDL 失败: {error}"))?;

                ddl.ok_or_else(|| format!("没有可用的 DDL: {name}"))
            }
            _ => Err("PostgreSQL 不支持读取该对象类型的 DDL".to_string()),
        }
    }

    async fn object_variants(
        &self,
        kind: &str,
        name: &str,
    ) -> Result<Vec<crate::models::ObjectVariant>, String> {
        if !matches!(kind, "function" | "procedure") {
            return Ok(Vec::new());
        }
        let schema = self.current_schema().await;
        let prokind = if kind == "function" { "f" } else { "p" };
        let rows = sqlx::query(
            "SELECT p.oid::text, \
                    COALESCE(pg_get_function_arguments(p.oid), ''), \
                    COALESCE(pg_get_function_result(p.oid), '') \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             WHERE n.nspname = $1 AND p.proname = $2 AND p.prokind::text = $3 \
             ORDER BY p.oid",
        )
        .bind(&schema)
        .bind(name)
        .bind(prokind)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取重载签名失败: {error}"))?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let id: String = row.try_get(0).unwrap_or_default();
                let args: String = row.try_get(1).unwrap_or_default();
                let result: String = row.try_get(2).unwrap_or_default();
                let label = if result.is_empty() {
                    format!("{name}({args})")
                } else {
                    format!("{name}({args}) → {result}")
                };
                crate::models::ObjectVariant { id, label }
            })
            .collect())
    }

    async fn object_ddl_with_identity(
        &self,
        kind: &str,
        name: &str,
        identity: Option<&str>,
    ) -> Result<String, String> {
        let Some(identity) = identity else {
            return self.object_ddl(kind, name).await;
        };
        if !matches!(kind, "function" | "procedure") {
            return self.object_ddl(kind, name).await;
        }
        let oid: i64 = identity
            .parse()
            .map_err(|_| "函数签名标识无效".to_string())?;
        let ddl: Option<String> = sqlx::query_scalar("SELECT pg_get_functiondef($1::oid)")
            .bind(oid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| format!("读取定义失败: {error}"))?;
        ddl.ok_or_else(|| format!("没有可用的 DDL: {name}"))
    }

    async fn create_database(
        &self,
        name: &str,
        _charset: Option<&str>,
        _collation: Option<&str>,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("数据库名称不能为空".to_string());
        }
        let sql = format!("CREATE DATABASE {}", quote_identifier(name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("创建数据库失败: {error}"))?;
        Ok(())
    }

    async fn create_schema(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("模式名称不能为空".to_string());
        }
        let sql = format!("CREATE SCHEMA {}", quote_identifier(name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("创建模式失败: {error}"))?;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("数据库名称不能为空".to_string());
        }
        if matches!(name, "postgres" | "template0" | "template1") {
            return Err("不能删除系统数据库".to_string());
        }
        let current: Option<String> = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&self.pool)
            .await
            .ok();
        if current.as_deref() == Some(name) {
            return Err("不能删除当前正在连接的数据库，请先切换到其他数据库".to_string());
        }
        let sql = format!("DROP DATABASE {}", quote_identifier(name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除数据库失败: {error}"))?;
        Ok(())
    }

    async fn drop_schema(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("模式名称不能为空".to_string());
        }
        let lower = name.to_ascii_lowercase();
        if lower == "pg_catalog"
            || lower == "information_schema"
            || lower == "pg_toast"
            || lower.starts_with("pg_temp_")
            || lower.starts_with("pg_toast_temp_")
        {
            return Err("不能删除系统模式".to_string());
        }
        if self.current_schema().await == name && name != "public" {
            self.switch_schema("public").await?;
        }
        let sql = format!("DROP SCHEMA {}", quote_identifier(name));
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除模式失败: {error}"))?;
        Ok(())
    }

    async fn save_object_definition(
        &self,
        kind: &str,
        name: &str,
        definition: &str,
    ) -> Result<(), String> {
        if !matches!(kind, "view" | "function" | "procedure") {
            return Err("PostgreSQL 仅支持在应用内修改视图、函数和存储过程".to_string());
        }
        let _ = self.object_ddl(kind, name).await?;
        let definition = definition.trim().trim_end_matches(';').trim();
        let upper = definition.to_ascii_uppercase();
        if !upper.starts_with("CREATE ") {
            return Err("定义必须以 CREATE 开头".to_string());
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
        let schema = self.current_schema().await;
        let source = qualified_identifier(&schema, source);
        let target = qualified_identifier(&schema, target);

        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE {target} (LIKE {source} INCLUDING ALL)"
        )))
        .execute(&self.pool)
        .await
        .map_err(|error| format!("创建表失败: {error}"))?;

        let copy_sql =
            format!("INSERT INTO {target} OVERRIDING SYSTEM VALUE SELECT * FROM {source}");
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
        let schema = self.current_schema().await;
        let replacement = qualified_identifier(&schema, target_name);
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
        let schema = self.current_schema().await;
        let source = qualified_identifier(&schema, source);
        let target = qualified_identifier(&schema, target);
        let suffix = if with_data { "" } else { " WITH NO DATA" };
        let sql = format!("CREATE TABLE {target} AS SELECT * FROM {source}{suffix}");
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("快照视图失败: {error}"))?;
        Ok(())
    }

    async fn rename_object(&self, kind: &str, name: &str, new_name: &str) -> Result<(), String> {
        let schema = self.current_schema().await;
        let old = qualified_identifier(&schema, name);
        let new = quote_identifier(new_name);
        let sql = match kind {
            "table" | "view" | "matview" | "sequence" => {
                format!("ALTER TABLE {old} RENAME TO {new}")
            }
            "index" => format!("ALTER INDEX {old} RENAME TO {new}"),
            "function" => format!("ALTER FUNCTION {old} RENAME TO {new}"),
            "procedure" => format!("ALTER PROCEDURE {old} RENAME TO {new}"),
            _ => return Err("PostgreSQL 暂不支持重命名该对象类型".to_string()),
        };

        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("重命名失败: {error}"))?;
        Ok(())
    }

    async fn drop_object(&self, kind: &str, name: &str) -> Result<(), String> {
        let schema = self.current_schema().await;

        if kind == "trigger" {
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT c.relname \
                 FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = $1 AND t.tgname = $2 AND NOT t.tgisinternal",
            )
            .bind(&schema)
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("查找触发器失败: {error}"))?;

            return match tables.as_slice() {
                [table] => {
                    let sql = format!(
                        "DROP TRIGGER {} ON {}",
                        quote_identifier(name),
                        qualified_identifier(&schema, table)
                    );
                    sqlx::query(sqlx::AssertSqlSafe(sql))
                        .execute(&self.pool)
                        .await
                        .map_err(|error| format!("删除对象失败: {error}"))?;
                    Ok(())
                }
                [] => Err(format!("触发器不存在: {name}")),
                _ => Err("同名触发器存在于多张表，请在表结构中删除".to_string()),
            };
        }

        let keyword = match kind {
            "table" => "TABLE",
            "view" => "VIEW",
            "matview" => "MATERIALIZED VIEW",
            "sequence" => "SEQUENCE",
            "index" => "INDEX",
            "function" | "procedure" => "ROUTINE",
            _ => return Err("PostgreSQL 不支持删除该对象类型".to_string()),
        };
        let sql = format!("DROP {keyword} {}", qualified_identifier(&schema, name));
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除对象失败: {error}"))?;
        Ok(())
    }
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn qualified_identifier(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_identifier(schema), quote_identifier(name))
}

/// PostgreSQL does not implicitly cast text parameters to column types, so
/// every generated placeholder carries an explicit cast taken from the actual
/// column type (`format_type` output is valid in a cast expression).
fn cast_for(columns: &[ColumnInfo], name: &str) -> String {
    columns
        .iter()
        .find(|column| column.name == name)
        .map(|column| format!("::{}", column.data_type))
        .unwrap_or_default()
}

async fn pg_primary_key_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<HashSet<String>, String> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT a.attname \
         FROM pg_index i \
         JOIN pg_class c ON c.oid = i.indrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(i.indkey) \
         WHERE n.nspname = $1 AND c.relname = $2 AND i.indisprimary",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取主键失败: {error}"))?;

    Ok(rows.into_iter().collect())
}

async fn pg_indexes(pool: &PgPool, schema: &str, table: &str) -> Result<Vec<IndexInfo>, String> {
    let rows = sqlx::query(
        "SELECT i.relname, ix.indisunique, ix.indisprimary, a.attname, u.ord \
         FROM pg_index ix \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_class c ON c.oid = ix.indrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         CROSS JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS u(attnum, ord) \
         LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = u.attnum \
         WHERE n.nspname = $1 AND c.relname = $2 \
         ORDER BY i.relname, u.ord",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取索引失败: {error}"))?;

    let mut indexes: BTreeMap<String, IndexInfo> = BTreeMap::new();
    for row in rows {
        let name: String = row.try_get(0).unwrap_or_default();
        let unique: bool = row.try_get(1).unwrap_or(false);
        let primary: bool = row.try_get(2).unwrap_or(false);
        let column: Option<String> = row.try_get(3).ok().flatten();

        let entry = indexes.entry(name.clone()).or_insert_with(|| IndexInfo {
            name,
            columns: Vec::new(),
            unique,
            primary,
        });
        if let Some(column) = column {
            entry.columns.push(column);
        }
    }

    Ok(indexes.into_values().collect())
}

async fn pg_foreign_keys(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, String> {
    let rows = sqlx::query(
        "SELECT con.conname, a.attname, rc.relname, ra.attname, \
                con.confupdtype::text AS on_update, \
                con.confdeltype::text AS on_delete, u.ord \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_class rc ON rc.oid = con.confrelid \
         CROSS JOIN LATERAL unnest(con.conkey, con.confkey) \
              WITH ORDINALITY AS u(attnum, refattnum, ord) \
         JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = u.attnum \
         JOIN pg_attribute ra ON ra.attrelid = con.confrelid AND ra.attnum = u.refattnum \
         WHERE con.contype = 'f' AND n.nspname = $1 AND c.relname = $2 \
         ORDER BY con.conname, u.ord",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取外键失败: {error}"))?;

    let mut foreign_keys: BTreeMap<String, ForeignKeyInfo> = BTreeMap::new();
    for row in rows {
        let name: String = row.try_get(0).unwrap_or_default();
        let column: String = row.try_get(1).unwrap_or_default();
        let referenced_table: String = row.try_get(2).unwrap_or_default();
        let referenced_column: String = row.try_get(3).unwrap_or_default();
        let on_update: String = row.try_get(4).unwrap_or_default();
        let on_delete: String = row.try_get(5).unwrap_or_default();

        let entry = foreign_keys
            .entry(name.clone())
            .or_insert_with(|| ForeignKeyInfo {
                name,
                columns: Vec::new(),
                referenced_table,
                referenced_columns: Vec::new(),
                on_delete: foreign_key_action(&on_delete),
                on_update: foreign_key_action(&on_update),
            });
        entry.columns.push(column);
        entry.referenced_columns.push(referenced_column);
    }

    Ok(foreign_keys.into_values().collect())
}

fn foreign_key_action(code: &str) -> String {
    match code {
        "r" => "RESTRICT",
        "c" => "CASCADE",
        "n" => "SET NULL",
        "d" => "SET DEFAULT",
        _ => "NO ACTION",
    }
    .to_string()
}

fn build_table_ddl(
    schema: &str,
    table: &str,
    columns: &[ColumnInfo],
    indexes: &[IndexInfo],
    foreign_keys: &[ForeignKeyInfo],
) -> String {
    let mut definitions: Vec<String> = Vec::new();

    for column in columns {
        let mut definition = format!("  {} {}", quote_identifier(&column.name), column.data_type);
        if column.auto_increment && column.default_value.is_none() {
            definition.push_str(" GENERATED BY DEFAULT AS IDENTITY");
        }
        if let Some(default) = &column.default_value {
            definition.push_str(&format!(" DEFAULT {default}"));
        }
        if !column.nullable {
            definition.push_str(" NOT NULL");
        }
        definitions.push(definition);
    }

    if let Some(primary) = indexes.iter().find(|index| index.primary) {
        let columns = primary
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        if !columns.is_empty() {
            definitions.push(format!("  PRIMARY KEY ({columns})"));
        }
    }

    for foreign_key in foreign_keys {
        let columns = foreign_key
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let referenced_columns = foreign_key
            .referenced_columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let mut definition = format!(
            "  CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
            quote_identifier(&foreign_key.name),
            quote_identifier(&foreign_key.referenced_table)
        );
        if !foreign_key.on_update.eq_ignore_ascii_case("NO ACTION") {
            definition.push_str(&format!(" ON UPDATE {}", foreign_key.on_update));
        }
        if !foreign_key.on_delete.eq_ignore_ascii_case("NO ACTION") {
            definition.push_str(&format!(" ON DELETE {}", foreign_key.on_delete));
        }
        definitions.push(definition);
    }

    format!(
        "CREATE TABLE {}.{} (\n{}\n);",
        quote_identifier(schema),
        quote_identifier(table),
        definitions.join(",\n")
    )
}

fn bind_json_postgres<'q>(
    query: sqlx::query::Query<'q, Postgres, PgArguments>,
    value: &'q Value,
) -> sqlx::query::Query<'q, Postgres, PgArguments> {
    match value {
        Value::Null => query.bind(Option::<String>::None),
        Value::Bool(value) => query.bind(*value),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                query.bind(value)
            } else if let Some(value) = number.as_u64() {
                match i64::try_from(value) {
                    Ok(value) => query.bind(value),
                    Err(_) => query.bind(number.to_string()),
                }
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

fn try_opt<'r, T>(row: &'r PgRow, index: usize) -> Option<T>
where
    T: sqlx::Decode<'r, Postgres> + sqlx::Type<Postgres>,
{
    row.try_get::<Option<T>, _>(index).ok().flatten()
}

fn scalar_or_placeholder(row: &PgRow, index: usize, type_name: &str) -> Value {
    if let Ok(raw) = row.try_get_raw(index) {
        if raw.is_null() {
            return Value::Null;
        }
        // Text format covers text-protocol results; enum labels and other
        // string-like binary values are still printable UTF-8. Values whose
        // bytes are not printable text keep a visible placeholder.
        if let Ok(text) = raw.as_str() {
            return Value::String(text.to_string());
        }
        if let Ok(bytes) = raw.as_bytes() {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if !text.is_empty() && text.chars().all(|character| !character.is_control()) {
                    return Value::String(text.to_string());
                }
            }
        }
    }
    Value::String(format!("<{type_name}>"))
}

/// PostgreSQL sends `inet` / `cidr` as family/bits/cidr/len/address bytes.
fn inet_to_string(row: &PgRow, index: usize) -> Option<String> {
    let raw = row.try_get_raw(index).ok()?;
    if raw.is_null() || !matches!(raw.format(), PgValueFormat::Binary) {
        return None;
    }
    let bytes = raw.as_bytes().ok()?;
    if bytes.len() < 5 {
        return None;
    }

    let family = bytes[0];
    let bits = bytes[1];
    let is_cidr = bytes[2] != 0;
    let length = bytes[3] as usize;
    let address = bytes.get(4..4 + length)?;

    let (ip, max_bits) = match (family, length) {
        (2, 4) => (
            std::net::Ipv4Addr::new(address[0], address[1], address[2], address[3]).to_string(),
            32u32,
        ),
        (3, 16) => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(address);
            (std::net::Ipv6Addr::from(octets).to_string(), 128u32)
        }
        _ => return None,
    };

    if !is_cidr && u32::from(bits) >= max_bits {
        Some(ip)
    } else {
        Some(format!("{ip}/{bits}"))
    }
}

fn interval_to_string(interval: &PgInterval) -> String {
    let mut parts = Vec::new();
    if interval.months != 0 {
        parts.push(format!("{} mons", interval.months));
    }
    if interval.days != 0 {
        parts.push(format!("{} days", interval.days));
    }
    if interval.microseconds != 0 || parts.is_empty() {
        let micros = interval.microseconds;
        let negative = micros < 0;
        let micros = micros.unsigned_abs();
        let total_seconds = micros / 1_000_000;
        let fraction = micros % 1_000_000;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let mut text = format!(
            "{}{:02}:{:02}:{:02}",
            if negative { "-" } else { "" },
            hours,
            minutes,
            seconds
        );
        if fraction != 0 {
            text.push_str(format!(".{fraction:06}").trim_end_matches('0'));
        }
        parts.push(text);
    }
    parts.join(" ")
}

fn array_to_json(row: &PgRow, index: usize, type_name: &str) -> Value {
    if let Some(values) = try_opt::<Vec<Option<String>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| value.map(Value::from).unwrap_or(Value::Null))
                .collect(),
        );
    }
    if let Some(values) = try_opt::<Vec<Option<i64>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| value.map(Value::from).unwrap_or(Value::Null))
                .collect(),
        );
    }
    if let Some(values) = try_opt::<Vec<Option<f64>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| value.map(Value::from).unwrap_or(Value::Null))
                .collect(),
        );
    }
    if let Some(values) = try_opt::<Vec<Option<bool>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| value.map(Value::from).unwrap_or(Value::Null))
                .collect(),
        );
    }
    if let Some(values) = try_opt::<Vec<Option<uuid::Uuid>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| {
                    value
                        .map(|value| Value::String(value.to_string()))
                        .unwrap_or(Value::Null)
                })
                .collect(),
        );
    }
    if let Some(values) = try_opt::<Vec<Option<Value>>>(row, index) {
        return Value::Array(
            values
                .into_iter()
                .map(|value| value.unwrap_or(Value::Null))
                .collect(),
        );
    }
    scalar_or_placeholder(row, index, type_name)
}

fn value_to_json(row: &PgRow, index: usize) -> Value {
    let type_name = row.column(index).type_info().name().to_ascii_uppercase();

    if type_name.ends_with("[]") || type_name.starts_with('_') {
        return array_to_json(row, index, &type_name);
    }

    match type_name.as_str() {
        "BOOL" | "BOOLEAN" => try_opt::<bool>(row, index)
            .map(Value::from)
            .unwrap_or(Value::Null),
        "INT2" | "INT4" | "INT8" | "OID" => try_opt::<i64>(row, index)
            .map(Value::from)
            .or_else(|| try_opt::<Oid>(row, index).map(|value| Value::from(i64::from(value.0))))
            .unwrap_or(Value::Null),
        "FLOAT4" | "FLOAT8" => try_opt::<f64>(row, index)
            .map(Value::from)
            .unwrap_or(Value::Null),
        "NUMERIC" => try_opt::<BigDecimal>(row, index)
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "TEXT" | "VARCHAR" | "BPCHAR" | "CHAR" | "NAME" | "UNKNOWN" | "CITEXT" => {
            try_opt::<String>(row, index)
                .map(Value::from)
                .unwrap_or(Value::Null)
        }
        "DATE" => try_opt::<NaiveDate>(row, index)
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "TIMESTAMP" => try_opt::<NaiveDateTime>(row, index)
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "TIMESTAMPTZ" => try_opt::<DateTime<Utc>>(row, index)
            .map(|value| Value::String(value.to_rfc3339()))
            .unwrap_or(Value::Null),
        "TIME" => try_opt::<NaiveTime>(row, index)
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "UUID" => try_opt::<uuid::Uuid>(row, index)
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        "JSON" | "JSONB" => try_opt::<Value>(row, index).unwrap_or(Value::Null),
        "BYTEA" => try_opt::<Vec<u8>>(row, index)
            .map(|bytes| Value::String(format!("<BLOB {} bytes>", bytes.len())))
            .unwrap_or(Value::Null),
        "INET" | "CIDR" => inet_to_string(row, index)
            .map(Value::String)
            .unwrap_or_else(|| scalar_or_placeholder(row, index, &type_name)),
        "INTERVAL" => try_opt::<PgInterval>(row, index)
            .map(|interval| Value::String(interval_to_string(&interval)))
            .unwrap_or(Value::Null),
        _ => {
            if let Some(value) = try_opt::<i64>(row, index) {
                return Value::from(value);
            }
            if let Some(value) = try_opt::<f64>(row, index) {
                return Value::from(value);
            }
            if let Some(value) = try_opt::<bool>(row, index) {
                return Value::from(value);
            }
            if let Some(value) = try_opt::<String>(row, index) {
                return Value::from(value);
            }
            if let Some(value) = try_opt::<BigDecimal>(row, index) {
                return Value::String(value.to_string());
            }
            if let Some(value) = try_opt::<DateTime<Utc>>(row, index) {
                return Value::String(value.to_rfc3339());
            }
            if let Some(value) = try_opt::<NaiveDateTime>(row, index) {
                return Value::String(value.to_string());
            }
            if let Some(value) = try_opt::<NaiveDate>(row, index) {
                return Value::String(value.to_string());
            }
            if let Some(value) = try_opt::<Value>(row, index) {
                return value;
            }
            if let Some(value) = try_opt::<Vec<u8>>(row, index) {
                return Value::String(format!("<BLOB {} bytes>", value.len()));
            }
            scalar_or_placeholder(row, index, &type_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config(host: &str, port: u16, username: &str, database: &str) -> ConnectionConfig {
        ConnectionConfig {
            driver: "postgres".to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
            database: database.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn quotes_identifiers() {
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
        assert_eq!(
            qualified_identifier("my schema", "t"),
            "\"my schema\".\"t\""
        );
    }

    /// Live regression against a real PostgreSQL server. Set
    /// `MYDB_TEST_PG_HOST` (plus optional PORT/USER/PASSWORD/DATABASE) to run:
    ///
    /// ```text
    /// MYDB_TEST_PG_HOST=127.0.0.1 MYDB_TEST_PG_PORT=55432 \
    ///   MYDB_TEST_PG_USER=postgres MYDB_TEST_PG_DATABASE=postgres \
    ///   cargo test postgres -- --nocapture
    /// ```
    #[tokio::test]
    async fn live_catalog_and_data_roundtrip() {
        let Ok(host) = std::env::var("MYDB_TEST_PG_HOST") else {
            return;
        };
        let port: u16 = std::env::var("MYDB_TEST_PG_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(5432);
        let username =
            std::env::var("MYDB_TEST_PG_USER").unwrap_or_else(|_| "postgres".to_string());
        let password = std::env::var("MYDB_TEST_PG_PASSWORD").ok();
        let database =
            std::env::var("MYDB_TEST_PG_DATABASE").unwrap_or_else(|_| "postgres".to_string());

        let config = test_config(&host, port, &username, &database);
        let driver = PostgresDriver::connect(&config, &host, port, password.as_deref(), None)
            .await
            .expect("connect");
        driver.test_connection().await.expect("ping");

        driver
            .execute("DROP SCHEMA IF EXISTS mydb_test CASCADE", 100)
            .await
            .expect("drop schema");
        driver
            .execute("CREATE SCHEMA mydb_test", 100)
            .await
            .expect("create schema");

        let schemas = driver.list_schemas().await.expect("schemas");
        assert!(schemas.contains(&"mydb_test".to_string()));

        driver
            .switch_schema("mydb_test")
            .await
            .expect("switch schema");
        assert!(driver.switch_schema("missing_schema").await.is_err());

        driver
            .execute(
                "CREATE TABLE parents (\
                   id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY, \
                   name TEXT NOT NULL\
                 )",
                100,
            )
            .await
            .expect("create parents");
        driver
            .execute(
                "CREATE TABLE children (\
                   id BIGSERIAL PRIMARY KEY, \
                   parent_id BIGINT REFERENCES parents(id) ON DELETE CASCADE, \
                   label VARCHAR(64) NOT NULL DEFAULT 'x', \
                   price NUMERIC(10,2), \
                   tags TEXT[], \
                   meta JSONB, \
                   flag BOOLEAN\
                 )",
                100,
            )
            .await
            .expect("create children");
        driver
            .execute("CREATE INDEX idx_children_label ON children(label)", 100)
            .await
            .expect("create index");
        driver
            .execute(
                "CREATE VIEW child_labels AS SELECT id, label FROM children",
                100,
            )
            .await
            .expect("create view");
        driver
            .execute(
                "CREATE MATERIALIZED VIEW child_counts AS \
                 SELECT parent_id, count(*) AS n FROM children GROUP BY parent_id",
                100,
            )
            .await
            .expect("create matview");

        let objects = driver.list_tables().await.expect("list tables");
        let names: Vec<(&str, &str)> = objects
            .iter()
            .map(|item| (item.name.as_str(), item.kind.as_str()))
            .collect();
        assert!(names.contains(&("children", "table")), "objects: {names:?}");
        assert!(
            names.contains(&("child_labels", "view")),
            "objects: {names:?}"
        );
        assert!(
            names.contains(&("child_counts", "matview")),
            "objects: {names:?}"
        );
        assert!(
            names.iter().any(|(_, kind)| *kind == "sequence"),
            "objects: {names:?}"
        );

        let columns = driver.list_columns("children").await.expect("columns");
        let id = columns
            .iter()
            .find(|column| column.name == "id")
            .expect("id column");
        assert!(id.primary_key);
        assert!(id.auto_increment);
        assert_eq!(id.data_type, "bigint");
        let nullable = columns
            .iter()
            .find(|column| column.name == "label")
            .expect("label column");
        assert!(!nullable.nullable);
        assert_eq!(
            nullable.default_value.as_deref(),
            Some("'x'::character varying")
        );

        let structure = driver.table_structure("children").await.expect("structure");
        assert!(
            structure
                .ddl
                .contains("CREATE TABLE \"mydb_test\".\"children\"")
        );
        assert!(structure.ddl.contains("PRIMARY KEY"));
        assert!(structure.indexes.iter().any(|index| {
            index.name == "idx_children_label" && index.columns == vec!["label".to_string()]
        }));
        assert!(structure.foreign_keys.iter().any(|key| {
            key.referenced_table == "parents"
                && key.columns == vec!["parent_id".to_string()]
                && key.referenced_columns == vec!["id".to_string()]
                && key.on_delete == "CASCADE"
        }));

        driver
            .execute("INSERT INTO parents (name) VALUES ('p1')", 100)
            .await
            .expect("insert parent");

        // Types without a Rust decoder fall back to the wire text format.
        driver
            .execute(
                "CREATE TABLE network (id INT PRIMARY KEY, addr INET, span INTERVAL)",
                100,
            )
            .await
            .expect("create network");
        driver
            .insert_row(
                "network",
                &[
                    ColumnValue {
                        column: "id".to_string(),
                        value: json!(1),
                    },
                    ColumnValue {
                        column: "addr".to_string(),
                        value: json!("192.168.1.1"),
                    },
                    ColumnValue {
                        column: "span".to_string(),
                        value: json!("1 hour"),
                    },
                ],
            )
            .await
            .expect("insert network");
        let network = driver
            .read_rows("network", 10, 0, None, None, None)
            .await
            .expect("network rows");
        assert_eq!(network.rows[0][1], json!("192.168.1.1"));
        assert_eq!(network.rows[0][2], json!("01:00:00"));

        let inserted = driver
            .insert_row(
                "children",
                &[
                    ColumnValue {
                        column: "parent_id".to_string(),
                        value: json!(1),
                    },
                    ColumnValue {
                        column: "label".to_string(),
                        value: json!("a"),
                    },
                    ColumnValue {
                        column: "price".to_string(),
                        value: json!(12.5),
                    },
                    ColumnValue {
                        column: "tags".to_string(),
                        value: json!("{x,y}"),
                    },
                    ColumnValue {
                        column: "meta".to_string(),
                        value: json!("{\"k\":1}"),
                    },
                    ColumnValue {
                        column: "flag".to_string(),
                        value: json!(true),
                    },
                ],
            )
            .await
            .expect("insert row");
        assert_eq!(inserted, 1);

        let page = driver
            .read_rows(
                "children",
                10,
                0,
                Some("label = 'a'"),
                Some("id"),
                Some("desc"),
            )
            .await
            .expect("page");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0][2], json!("a"));

        driver
            .update_row(
                "children",
                &[ColumnValue {
                    column: "id".to_string(),
                    value: json!(1),
                }],
                &[ColumnValue {
                    column: "label".to_string(),
                    value: json!("updated"),
                }],
                false,
            )
            .await
            .expect("update");

        let result = driver
            .execute(
                "SELECT id, label, price, tags, meta, flag FROM children ORDER BY id",
                100,
            )
            .await
            .expect("select");
        assert!(result.is_query);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][1], json!("updated"));
        assert_eq!(result.rows[0][2], json!("12.50"));
        assert_eq!(result.rows[0][3], json!(["x", "y"]));
        assert_eq!(result.rows[0][4], json!({"k": 1}));
        assert_eq!(result.rows[0][5], json!(true));

        driver
            .delete_row(
                "children",
                &[ColumnValue {
                    column: "id".to_string(),
                    value: json!(1),
                }],
                false,
            )
            .await
            .expect("delete");

        // Object DDL / rename / drop.
        driver
            .execute("CREATE TABLE probe (id integer PRIMARY KEY)", 100)
            .await
            .expect("create probe");
        let ddl = driver
            .object_ddl("table", "probe")
            .await
            .expect("probe ddl");
        assert!(ddl.contains("CREATE TABLE \"mydb_test\".\"probe\""));

        driver
            .rename_object("table", "probe", "probe2")
            .await
            .expect("rename probe");
        let objects = driver.list_tables().await.expect("objects after rename");
        assert!(objects.iter().any(|item| item.name == "probe2"));

        driver
            .drop_object("table", "probe2")
            .await
            .expect("drop probe");
        let objects = driver.list_tables().await.expect("objects after drop");
        assert!(!objects.iter().any(|item| item.name == "probe2"));

        driver
            .execute(
                "CREATE FUNCTION add_one(x integer) RETURNS integer \
                 LANGUAGE sql AS $$ SELECT x + 1 $$",
                100,
            )
            .await
            .expect("create function");
        let function_ddl = driver
            .object_ddl("function", "add_one")
            .await
            .expect("function ddl");
        assert!(function_ddl.contains("FUNCTION"));
        driver
            .drop_object("function", "add_one")
            .await
            .expect("drop function");

        driver
            .execute("DROP SCHEMA mydb_test CASCADE", 100)
            .await
            .expect("cleanup");
    }
}
