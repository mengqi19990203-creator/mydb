use crate::db::{DatabaseDriver, DatabaseSession};
use crate::db::query::is_read_query;
use crate::models::{
    ColumnInfo, ColumnValue, ConnectionConfig, ForeignKeyInfo, IndexInfo, QueryResult, TableInfo,
    TableRows, TableStructure,
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
            _ => PgSslMode::Disable,
        };
        let database = if config.database.trim().is_empty() {
            "postgres"
        } else {
            config.database.trim()
        };

        let options = PgConnectOptions::new()
            .host(host)
            .port(if port == 0 { 5432 } else { port })
            .username(&config.username)
            .password(password.unwrap_or_default())
            .database(database)
            .ssl_mode(ssl_mode)
            .application_name("mydb");

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

    async fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let schema = self.current_schema().await;
        let rows = sqlx::query(
            "SELECT a.attname, \
                    pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type, \
                    NOT a.attnotnull AS nullable, \
                    pg_get_expr(d.adbin, d.adrelid) AS default_value, \
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
                    auto_increment: row.try_get(4).unwrap_or(false),
                }
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
                    format!(
                        "第 {} 条语句执行失败（事务已回滚）: {error}",
                        index + 1
                    )
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

    async fn update_row(
        &self,
        table: &str,
        keys: &[ColumnValue],
        changes: &[ColumnValue],
    ) -> Result<u64, String> {
        if keys.is_empty() {
            return Err("该表没有主键，无法安全编辑".to_string());
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
        let where_clause = keys
            .iter()
            .enumerate()
            .map(|(index, item)| {
                format!(
                    "{} = ${}{}",
                    quote_identifier(&item.column),
                    changes.len() + index + 1,
                    cast_for(&columns, &item.column)
                )
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "UPDATE {} SET {set_clause} WHERE {where_clause}",
            qualified_identifier(&schema, table)
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for change in changes {
            query = bind_json_postgres(query, &change.value);
        }
        for key in keys {
            query = bind_json_postgres(query, &key.value);
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
                .map(|(index, item)| {
                    format!("${}{}", index + 1, cast_for(&columns, &item.column))
                })
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

    async fn delete_row(&self, table: &str, keys: &[ColumnValue]) -> Result<u64, String> {
        if keys.is_empty() {
            return Err("该表没有主键，无法安全删除".to_string());
        }

        let schema = self.current_schema().await;
        let columns = self.list_columns(table).await?;

        let where_clause = keys
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
            .join(" AND ");
        let sql = format!(
            "DELETE FROM {} WHERE {where_clause}",
            qualified_identifier(&schema, table)
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in keys {
            query = bind_json_postgres(query, &key.value);
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

    async fn rename_object(
        &self,
        kind: &str,
        name: &str,
        new_name: &str,
    ) -> Result<(), String> {
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
        let sql = format!(
            "DROP {keyword} {}",
            qualified_identifier(&schema, name)
        );
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
    format!(
        "{}.{}",
        quote_identifier(schema),
        quote_identifier(name)
    )
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

async fn pg_indexes(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, String> {
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
        let mut definition = format!(
            "  {} {}",
            quote_identifier(&column.name),
            column.data_type
        );
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
                .map(|value| value.map(|value| Value::String(value.to_string())).unwrap_or(Value::Null))
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
            .or_else(|| {
                try_opt::<Oid>(row, index).map(|value| Value::from(i64::from(value.0)))
            })
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

        driver.switch_schema("mydb_test").await.expect("switch schema");
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
        assert!(
            names.contains(&("children", "table")),
            "objects: {names:?}"
        );
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
        assert_eq!(nullable.default_value.as_deref(), Some("'x'::character varying"));

        let structure = driver
            .table_structure("children")
            .await
            .expect("structure");
        assert!(structure.ddl.contains("CREATE TABLE \"mydb_test\".\"children\""));
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
            .read_rows("children", 10, 0, Some("label = 'a'"), Some("id"), Some("desc"))
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