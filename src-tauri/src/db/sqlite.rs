use crate::db::query::is_read_query;
use crate::db::{DatabaseDriver, DatabaseSession};
use crate::models::{
    ColumnInfo, ColumnValue, ForeignKeyInfo, IndexInfo, QueryResult, TableInfo, TableRows,
    TableStructure,
};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use serde_json::Value;
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, SqlitePool, TypeInfo};
use std::collections::BTreeMap;
use std::time::Instant;

pub struct SqliteDriver {
    pool: SqlitePool,
}

impl SqliteDriver {
    pub async fn connect(file: &str) -> Result<Self, String> {
        let options = SqliteConnectOptions::new()
            .filename(file)
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|error| format!("连接 SQLite 失败: {error}"))?;

        Ok(Self { pool })
    }
}

async fn execute_on_connection(
    connection: &mut sqlx::SqliteConnection,
    sql: &str,
    max_rows: usize,
) -> Result<QueryResult, String> {
    let started = Instant::now();
    let sql = sql.trim();
    if sql.is_empty() {
        return Err("SQL 不能为空".to_string());
    }

    if is_read_query(sql) {
        // The user explicitly supplied this SQL in the editor, like any database client.
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

struct SqliteSession {
    connection: sqlx::pool::PoolConnection<sqlx::Sqlite>,
}

#[async_trait]
impl DatabaseSession for SqliteSession {
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
impl DatabaseDriver for SqliteDriver {
    async fn test_connection(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| format!("连接测试失败: {error}"))?;
        Ok(())
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        let rows = sqlx::query(
            "SELECT name, type FROM sqlite_master \
             WHERE type IN ('table', 'view', 'index', 'trigger') \
               AND name NOT LIKE 'sqlite_%' \
             ORDER BY type, name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取表列表失败: {error}"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let name: String = row.try_get("name").unwrap_or_default();
                let kind: String = row.try_get("type").unwrap_or_else(|_| "table".to_string());
                TableInfo { name, kind }
            })
            .collect())
    }

    async fn list_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKeyInfo>, String> {
        sqlite_foreign_keys(&self.pool, table).await
    }

    async fn index_table(&self, index_name: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar("SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = ?")
            .bind(index_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| format!("读取索引所属表失败: {error}"))
    }

    async fn execute(&self, sql: &str, max_rows: usize) -> Result<QueryResult, String> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        execute_on_connection(&mut connection, sql, max_rows).await
    }

    async fn open_session(&self) -> Result<Box<dyn DatabaseSession>, String> {
        let connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;
        Ok(Box::new(SqliteSession { connection }))
    }

    async fn execute_transaction(&self, statements: &[String]) -> Result<u64, String> {
        if statements.is_empty() {
            return Ok(0);
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| format!("开启事务失败: {error}"))?;
        let mut count = 0u64;
        for (index, statement) in statements.iter().enumerate() {
            sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))
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

    async fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let sql = format!("PRAGMA table_info({})", quote_identifier(table));
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("读取字段信息失败: {error}"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let name: String = row.try_get("name").unwrap_or_default();
                let data_type: String = row.try_get("type").unwrap_or_default();
                let not_null: i64 = row.try_get("notnull").unwrap_or(0);
                let primary_key: i64 = row.try_get("pk").unwrap_or(0);
                let default_value: Option<String> = row.try_get("dflt_value").ok();

                ColumnInfo {
                    name,
                    auto_increment: primary_key > 0
                        && data_type.to_ascii_uppercase().contains("INT"),
                    data_type,
                    nullable: not_null == 0,
                    primary_key: primary_key > 0,
                    default_value,
                    comment: None,
                }
            })
            .collect())
    }

    async fn table_structure(&self, table: &str) -> Result<TableStructure, String> {
        let columns = self.list_columns(table).await?;
        let indexes = sqlite_indexes(&self.pool, table).await?;
        let foreign_keys = sqlite_foreign_keys(&self.pool, table).await?;

        let ddl = sqlx::query_scalar::<_, Option<String>>(
            "SELECT sql FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?",
        )
        .bind(table)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("读取 DDL 失败: {error}"))?
        .flatten()
        .unwrap_or_default();

        Ok(TableStructure {
            columns,
            indexes,
            foreign_keys,
            ddl,
        })
    }

    async fn rebuild_column(
        &self,
        table: &str,
        column: &str,
        name: &str,
        data_type: &str,
        nullable: bool,
        default_value: Option<&str>,
    ) -> Result<(), String> {
        let table_ref = quote_identifier(table);
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| format!("获取数据库连接失败: {error}"))?;

        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("关闭外键检查失败: {error}"))?;

        let outcome = async {
            sqlx::raw_sql("BEGIN IMMEDIATE")
                .execute(&mut *connection)
                .await
                .map_err(|error| format!("开启重建事务失败: {error}"))?;

            if name != column {
                sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "ALTER TABLE {table_ref} RENAME COLUMN {} TO {}",
                    quote_identifier(column),
                    quote_identifier(name)
                )))
                .execute(&mut *connection)
                .await
                .map_err(|error| format!("重命名字段失败: {error}"))?;
            }

            let ddl: Option<String> = sqlx::query_scalar(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| format!("读取表定义失败: {error}"))?
            .flatten();
            let ddl = ddl
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("没有可用的表定义，无法重建表: {table}"))?;

            let columns = sqlite_column_names(&mut *connection, table).await?;
            if columns.is_empty() {
                return Err("表没有可复制的字段".to_string());
            }
            let objects: Vec<(String, String)> = sqlx::query_as(
                "SELECT type, sql FROM sqlite_master \
                 WHERE tbl_name = ? AND type IN ('index', 'trigger') AND sql IS NOT NULL \
                 ORDER BY type DESC, name",
            )
            .bind(table)
            .fetch_all(&mut *connection)
            .await
            .map_err(|error| format!("读取索引 / 触发器失败: {error}"))?;

            let rewritten = rewrite_sqlite_column(&ddl, name, data_type, nullable, default_value)?;
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let temp = format!("__mydb_rebuild_{nanos}");
            let temp_ref = quote_identifier(&temp);
            let temp_ddl = super::driver::rename_ddl_object(&rewritten, &temp_ref)?;

            sqlx::raw_sql(sqlx::AssertSqlSafe(temp_ddl))
                .execute(&mut *connection)
                .await
                .map_err(|error| format!("创建临时表失败: {error}"))?;

            let column_list = columns
                .iter()
                .map(|value| quote_identifier(value))
                .collect::<Vec<_>>()
                .join(", ");
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {temp_ref} ({column_list}) SELECT {column_list} FROM {table_ref}"
            )))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("复制表数据失败: {error}"))?;

            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TABLE {table_ref}")))
                .execute(&mut *connection)
                .await
                .map_err(|error| format!("删除旧表失败: {error}"))?;

            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {temp_ref} RENAME TO {table_ref}"
            )))
            .execute(&mut *connection)
            .await
            .map_err(|error| format!("重命名临时表失败: {error}"))?;

            for (kind, sql) in objects {
                sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                    .execute(&mut *connection)
                    .await
                    .map_err(|error| format!("恢复{kind}失败: {error}"))?;
            }

            sqlx::raw_sql("COMMIT")
                .execute(&mut *connection)
                .await
                .map_err(|error| format!("提交重建事务失败: {error}"))?;
            Ok(())
        }
        .await;

        if outcome.is_err() {
            let _ = sqlx::raw_sql("ROLLBACK").execute(&mut *connection).await;
        }
        let _ = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await;

        outcome
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
        let columns = self.list_columns(table).await?;
        let identifier = quote_identifier(table);
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
        let mut sql = format!("SELECT * FROM {}", quote_identifier(table));
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

    async fn dump_index_statements(&self, table: &str) -> Result<Vec<String>, String> {
        let rows = sqlx::query(
            "SELECT sql FROM sqlite_master \
             WHERE type = 'index' AND tbl_name = ? AND sql IS NOT NULL \
             ORDER BY name",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取索引定义失败: {error}"))?;
        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<Option<String>, _>(0).ok().flatten())
            .map(|sql| {
                let trimmed = sql.trim().trim_end_matches(';').to_string();
                format!("{trimmed};")
            })
            .collect())
    }

    fn dump_indexes_separately(&self) -> bool {
        true
    }

    async fn dump_insert_plan(&self, table: &str) -> Result<(Vec<String>, String), String> {
        Ok((
            self.list_columns(table)
                .await?
                .into_iter()
                .map(|column| column.name)
                .collect(),
            String::new(),
        ))
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
        let expressions = columns
            .iter()
            .map(|column| format!("quote({})", quote_identifier(column)))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT {expressions} FROM {}", quote_identifier(table));
        let mut stream = sqlx::query(sqlx::AssertSqlSafe(sql)).fetch(&self.pool);
        let mut count = 0u64;
        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|error| format!("读取表数据失败: {error}"))?
        {
            let values = (0..columns.len())
                .map(|index| {
                    row.try_get::<Option<String>, _>(index)
                        .ok()
                        .flatten()
                        .or_else(|| Some("NULL".to_string()))
                })
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
            "trigger" => "TRIGGER",
            "index" => "INDEX",
            _ => return Err(format!("SQLite 不支持删除该对象类型: {kind}")),
        };
        Ok(format!(
            "DROP {keyword} IF EXISTS {};",
            quote_identifier(name)
        ))
    }

    async fn read_rows_sorted(
        &self,
        table: &str,
        limit: u64,
        offset: u64,
        filter: Option<&str>,
        sorts: &[crate::models::TableSort],
    ) -> Result<TableRows, String> {
        let columns = self.list_columns(table).await?;
        let identifier = quote_identifier(table);
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
        let table_ref = quote_identifier(table);
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
                        let key_clause = change
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
                        let where_clause = if change.single_row {
                            format!(
                                "rowid = (SELECT rowid FROM {table_ref} WHERE {key_clause} LIMIT 1)"
                            )
                        } else {
                            key_clause
                        };
                        let sql = format!("DELETE FROM {table_ref} WHERE {where_clause}");
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_sqlite(query, &key.value);
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
                        let key_clause = change
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
                        let where_clause = if change.single_row {
                            format!(
                                "rowid = (SELECT rowid FROM {table_ref} WHERE {key_clause} LIMIT 1)"
                            )
                        } else {
                            key_clause
                        };
                        let sql =
                            format!("UPDATE {table_ref} SET {set_clause} WHERE {where_clause}");
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                        for item in &change.changes {
                            query = bind_json_sqlite(query, &item.value);
                        }
                        for key in &change.keys {
                            if !key.value.is_null() {
                                query = bind_json_sqlite(query, &key.value);
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
                        let placeholders = vec!["?"; change.values.len()].join(", ");
                        format!("INSERT INTO {table_ref} ({fields}) VALUES ({placeholders})")
                    };
                    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for item in &change.values {
                        query = bind_json_sqlite(query, &item.value);
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
        let table_ref = quote_identifier(table);
        let where_clause = if single_row {
            format!("rowid = (SELECT rowid FROM {table_ref} WHERE {key_clause} LIMIT 1)")
        } else {
            key_clause
        };
        let sql = format!("UPDATE {table_ref} SET {set_clause} WHERE {where_clause}");

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for change in changes {
            query = bind_json_sqlite(query, &change.value);
        }
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_sqlite(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("更新行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn insert_row(&self, table: &str, values: &[ColumnValue]) -> Result<u64, String> {
        let identifier = quote_identifier(table);
        let sql = if values.is_empty() {
            format!("INSERT INTO {identifier} DEFAULT VALUES")
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
            query = bind_json_sqlite(query, &value.value);
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
        let table_ref = quote_identifier(table);
        let where_clause = if single_row {
            format!("rowid = (SELECT rowid FROM {table_ref} WHERE {key_clause} LIMIT 1)")
        } else {
            key_clause
        };
        let sql = format!("DELETE FROM {table_ref} WHERE {where_clause}");

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in keys {
            if !key.value.is_null() {
                query = bind_json_sqlite(query, &key.value);
            }
        }

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除行失败: {error}"))?;

        Ok(result.rows_affected())
    }

    async fn object_ddl(&self, kind: &str, name: &str) -> Result<String, String> {
        let object_type = match kind {
            "table" | "view" | "index" | "trigger" => kind,
            _ => return Err("SQLite 不支持读取该对象类型的 DDL".to_string()),
        };

        let ddl = sqlx::query_scalar::<_, Option<String>>(
            "SELECT sql FROM sqlite_master WHERE type = ? AND name = ?",
        )
        .bind(object_type)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("读取 DDL 失败: {error}"))?
        .flatten();

        ddl.filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("没有可用的 DDL: {name}"))
    }

    async fn save_object_definition(
        &self,
        kind: &str,
        name: &str,
        definition: &str,
    ) -> Result<(), String> {
        if !matches!(kind, "view" | "trigger") {
            return Err("SQLite 仅支持在应用内修改视图和触发器".to_string());
        }
        let _ = self.object_ddl(kind, name).await?;
        let drop = format!(
            "DROP {} {}",
            if kind == "view" { "VIEW" } else { "TRIGGER" },
            quote_identifier(name)
        );
        let create = definition.trim().trim_end_matches(';').trim();
        if create.is_empty() {
            return Err("对象定义不能为空".to_string());
        }
        self.execute_transaction(&[drop, create.to_string()])
            .await
            .map(|_| ())
    }

    async fn duplicate_table(&self, source: &str, target: &str) -> Result<(), String> {
        let ddl = self.object_ddl("table", source).await?;
        let ddl = super::driver::rename_ddl_object(&ddl, &quote_identifier(target))?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("创建表失败: {error}"))?;

        let sql = format!(
            "INSERT INTO {} SELECT * FROM {}",
            quote_identifier(target),
            quote_identifier(source)
        );
        if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
        {
            let _ = sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP TABLE {}",
                quote_identifier(target)
            )))
            .execute(&self.pool)
            .await;
            return Err(format!("复制数据失败: {error}"));
        }

        // SQLite keeps indexes outside CREATE TABLE, so copy them explicitly.
        let indexes: Vec<(String, String)> = sqlx::query_as(
            "SELECT name, sql FROM sqlite_master \
             WHERE type = 'index' AND tbl_name = ? AND sql IS NOT NULL",
        )
        .bind(source)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("读取索引失败: {error}"))?;

        for (index_name, index_sql) in indexes {
            let new_index_name = format!("{target}_{index_name}");
            let ddl = super::driver::rename_index_ddl(
                &index_sql,
                &quote_identifier(&new_index_name),
                &quote_identifier(target),
            )?;
            if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
                .execute(&self.pool)
                .await
            {
                let _ = sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "DROP TABLE {}",
                    quote_identifier(target)
                )))
                .execute(&self.pool)
                .await;
                return Err(format!("复制索引失败: {error}"));
            }
        }
        Ok(())
    }

    async fn duplicate_view(&self, source: &str, target: &str) -> Result<(), String> {
        let ddl = self.object_ddl("view", source).await?;
        let ddl = super::driver::rename_ddl_object(&ddl, &quote_identifier(target))?;
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
        let filter = if with_data { "" } else { " WHERE 0" };
        let sql = format!(
            "CREATE TABLE {} AS SELECT * FROM {}{filter}",
            quote_identifier(target),
            quote_identifier(source)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("快照视图失败: {error}"))?;
        Ok(())
    }

    async fn rename_object(&self, kind: &str, name: &str, new_name: &str) -> Result<(), String> {
        if kind != "table" {
            return Err("SQLite 仅支持重命名表".to_string());
        }
        let sql = format!(
            "ALTER TABLE {} RENAME TO {}",
            quote_identifier(name),
            quote_identifier(new_name)
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("重命名失败: {error}"))?;
        Ok(())
    }

    async fn drop_object(&self, kind: &str, name: &str) -> Result<(), String> {
        let keyword = match kind {
            "table" => "TABLE",
            "view" => "VIEW",
            "index" => "INDEX",
            "trigger" => "TRIGGER",
            _ => return Err("SQLite 不支持删除该对象类型".to_string()),
        };
        let sql = format!("DROP {keyword} {}", quote_identifier(name));
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .map_err(|error| format!("删除对象失败: {error}"))?;
        Ok(())
    }
}

async fn sqlite_column_names(
    connection: &mut sqlx::SqliteConnection,
    table: &str,
) -> Result<Vec<String>, String> {
    let sql = format!("PRAGMA table_info({})", quote_identifier(table));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| format!("读取字段列表失败: {error}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect())
}

const SQLITE_CONSTRAINT_KEYWORDS: &[&str] = &[
    "PRIMARY",
    "NOT",
    "NULL",
    "UNIQUE",
    "CHECK",
    "DEFAULT",
    "COLLATE",
    "REFERENCES",
    "GENERATED",
    "CONSTRAINT",
    "AS",
];

fn skip_sqlite_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn skip_sqlite_quote(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let quote = *bytes.get(start)?;
    if quote == b'[' {
        return text[start..].find(']').map(|offset| start + offset + 1);
    }
    if !matches!(quote, b'\'' | b'"' | b'`') {
        return None;
    }
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn matching_sqlite_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = open + 1;
    let mut depth = 1usize;
    while index < bytes.len() {
        if let Some(end) = skip_sqlite_quote(text, index) {
            index = end;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn read_sqlite_word(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut index = skip_sqlite_ws(bytes, start);
    if index >= bytes.len() || !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
        return None;
    }
    let begin = index;
    while index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' || bytes[index] == b'$')
    {
        index += 1;
    }
    Some((text[begin..index].to_ascii_uppercase(), index))
}

fn read_sqlite_identifier(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut index = skip_sqlite_ws(bytes, start);
    if index >= bytes.len() {
        return None;
    }
    if let Some(end) = skip_sqlite_quote(text, index) {
        return Some((text[index..end].to_string(), end));
    }
    if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
        return None;
    }
    let begin = index;
    while index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' || bytes[index] == b'$')
    {
        index += 1;
    }
    Some((text[begin..index].to_string(), index))
}

fn sqlite_identifier_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            return inner.replace("\"\"", "\"");
        }
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            return inner.replace("''", "'");
        }
        if trimmed.starts_with('`') && trimmed.ends_with('`') {
            return inner.replace("``", "`");
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            return inner.to_string();
        }
    }
    trimmed.to_string()
}

fn sqlite_table_body(ddl: &str) -> Option<(usize, usize)> {
    let bytes = ddl.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if let Some(end) = skip_sqlite_quote(ddl, index) {
            index = end;
            continue;
        }
        if bytes[index] == b'(' {
            return matching_sqlite_paren(ddl, index).map(|close| (index, close));
        }
        index += 1;
    }
    None
}

fn split_sqlite_definitions(
    text: &str,
    start: usize,
    end: usize,
) -> Result<Vec<(usize, usize)>, String> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut part_start = start;
    let mut index = start;
    while index < end {
        if let Some(quote_end) = skip_sqlite_quote(text, index) {
            if quote_end > end {
                return Err("表定义中的引号未闭合".to_string());
            }
            index = quote_end;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                ranges.push((part_start, index));
                part_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    ranges.push((part_start, end));
    Ok(ranges)
}

fn skip_sqlite_default_value(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = skip_sqlite_ws(bytes, start);
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'(' {
        return matching_sqlite_paren(text, index)
            .map(|close| close + 1)
            .unwrap_or(text.len());
    }
    if let Some(end) = skip_sqlite_quote(text, index) {
        return end;
    }
    if matches!(bytes[index], b'+' | b'-') {
        index = skip_sqlite_ws(bytes, index + 1);
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b',' | b')')
        {
            index += 1;
        }
        return index;
    }
    while index < bytes.len()
        && !bytes[index].is_ascii_whitespace()
        && !matches!(bytes[index], b',' | b')')
    {
        index += 1;
    }
    index
}

fn strip_sqlite_null_and_default(suffix: &str) -> String {
    let bytes = suffix.as_bytes();
    let mut removals: Vec<(usize, usize)> = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if let Some(end) = skip_sqlite_quote(suffix, index) {
            index = end;
            continue;
        }
        if bytes[index] == b'(' {
            index = matching_sqlite_paren(suffix, index)
                .map(|close| close + 1)
                .unwrap_or(bytes.len());
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let Some((word, word_end)) = read_sqlite_word(suffix, index) else {
                index += 1;
                continue;
            };
            if word == "NOT" {
                if let Some((next, next_end)) = read_sqlite_word(suffix, word_end) {
                    if next == "NULL" {
                        removals.push((index, next_end));
                        index = next_end;
                        continue;
                    }
                }
            } else if word == "DEFAULT" {
                let end = skip_sqlite_default_value(suffix, word_end);
                removals.push((index, end));
                index = end;
                continue;
            }
            index = word_end;
            continue;
        }
        index += 1;
    }

    if removals.is_empty() {
        return suffix.trim().to_string();
    }

    let mut result = String::new();
    let mut cursor = 0usize;
    for (start, end) in removals {
        if start > cursor {
            result.push_str(&suffix[cursor..start]);
        }
        cursor = end;
    }
    if cursor < suffix.len() {
        result.push_str(&suffix[cursor..]);
    }
    result.trim().to_string()
}

fn rewrite_sqlite_column(
    ddl: &str,
    column: &str,
    new_type: &str,
    nullable: bool,
    default_value: Option<&str>,
) -> Result<String, String> {
    let (open, close) =
        sqlite_table_body(ddl).ok_or_else(|| "无法解析表定义中的字段列表".to_string())?;
    let ranges = split_sqlite_definitions(ddl, open + 1, close)?;
    for (start, end) in ranges {
        let definition = &ddl[start..end];
        let leading = &definition[..definition.len() - definition.trim_start().len()];
        let inner = definition.trim_start();
        let Some((raw_name, name_end)) = read_sqlite_identifier(inner, 0) else {
            continue;
        };
        if !sqlite_identifier_value(&raw_name).eq_ignore_ascii_case(column) {
            continue;
        }

        let bytes = inner.as_bytes();
        let type_start = skip_sqlite_ws(bytes, name_end);
        let mut suffix_start = inner.len();
        let mut index = type_start;
        while index < inner.len() {
            if let Some(quote_end) = skip_sqlite_quote(inner, index) {
                index = quote_end;
                continue;
            }
            if bytes[index] == b'(' {
                index = matching_sqlite_paren(inner, index)
                    .map(|paren| paren + 1)
                    .unwrap_or(inner.len());
                continue;
            }
            if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
                let Some((word, word_end)) = read_sqlite_word(inner, index) else {
                    index += 1;
                    continue;
                };
                if SQLITE_CONSTRAINT_KEYWORDS.contains(&word.as_str()) {
                    suffix_start = index;
                    break;
                }
                index = word_end;
                continue;
            }
            index += 1;
        }

        let suffix = &inner[suffix_start..];
        let generated = read_sqlite_word(suffix, 0)
            .map(|(word, _)| word == "GENERATED" || word == "AS")
            .unwrap_or(false)
            || suffix.to_ascii_uppercase().contains("GENERATED ALWAYS AS");
        if generated {
            return Err(format!(
                "字段 {column} 是生成列，SQLite 重建暂不支持修改生成列"
            ));
        }

        let remaining = strip_sqlite_null_and_default(suffix);
        let mut rebuilt = format!("{} {}", &inner[..name_end], new_type);
        if !remaining.is_empty() {
            rebuilt.push(' ');
            rebuilt.push_str(&remaining);
        }
        if !nullable {
            rebuilt.push_str(" NOT NULL");
        }
        if let Some(default) = default_value {
            rebuilt.push_str(" DEFAULT ");
            rebuilt.push_str(default);
        }

        return Ok(format!(
            "{}{}{}{}",
            &ddl[..start],
            leading,
            rebuilt,
            &ddl[end..]
        ));
    }

    Err(format!("在表定义中找不到字段 {column}"))
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

async fn sqlite_indexes(pool: &SqlitePool, table: &str) -> Result<Vec<IndexInfo>, String> {
    let sql = format!("PRAGMA index_list({})", quote_identifier(table));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取索引失败: {error}"))?;

    let mut indexes = Vec::new();
    for row in rows {
        let name: String = row.try_get("name").unwrap_or_default();
        let unique: i64 = row.try_get("unique").unwrap_or(0);
        let origin: String = row.try_get("origin").unwrap_or_default();

        let info_sql = format!("PRAGMA index_info({})", quote_identifier(&name));
        let info_rows = sqlx::query(sqlx::AssertSqlSafe(info_sql))
            .fetch_all(pool)
            .await
            .map_err(|error| format!("读取索引列失败: {error}"))?;
        let columns = info_rows
            .into_iter()
            .filter_map(|item| item.try_get::<String, _>("name").ok())
            .collect();

        indexes.push(IndexInfo {
            name,
            columns,
            unique: unique != 0,
            primary: origin.eq_ignore_ascii_case("pk"),
        });
    }

    Ok(indexes)
}

async fn sqlite_foreign_keys(
    pool: &SqlitePool,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, String> {
    let sql = format!("PRAGMA foreign_key_list({})", quote_identifier(table));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取外键失败: {error}"))?;

    let mut foreign_keys: BTreeMap<i64, ForeignKeyInfo> = BTreeMap::new();
    for row in rows {
        let id: i64 = row.try_get("id").unwrap_or(0);
        let referenced_table: String = row.try_get("table").unwrap_or_default();
        let from: String = row.try_get("from").unwrap_or_default();
        let to: Option<String> = row.try_get("to").ok();
        let on_update: String = row.try_get("on_update").unwrap_or_default();
        let on_delete: String = row.try_get("on_delete").unwrap_or_default();

        let entry = foreign_keys.entry(id).or_insert_with(|| ForeignKeyInfo {
            name: format!("fk_{id}"),
            columns: Vec::new(),
            referenced_table,
            referenced_columns: Vec::new(),
            on_delete,
            on_update,
        });
        entry.columns.push(from);
        if let Some(to) = to.filter(|value| !value.trim().is_empty()) {
            entry.referenced_columns.push(to);
        }
    }

    for foreign_key in foreign_keys.values_mut() {
        if foreign_key.referenced_columns.len() >= foreign_key.columns.len() {
            continue;
        }
        let sql = format!(
            "PRAGMA table_info({})",
            quote_identifier(&foreign_key.referenced_table)
        );
        let columns = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(pool)
            .await
            .map_err(|error| format!("读取外键目标字段失败: {error}"))?;
        let primary: Vec<String> = columns
            .into_iter()
            .filter(|row| row.try_get::<i64, _>("pk").unwrap_or(0) > 0)
            .filter_map(|row| row.try_get::<String, _>("name").ok())
            .collect();
        if primary.len() == foreign_key.columns.len() {
            foreign_key.referenced_columns = primary;
        }
    }

    Ok(foreign_keys.into_values().collect())
}

fn bind_json_sqlite<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments>,
    value: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DatabaseDriver;
    use serde_json::json;

    async fn test_driver() -> (tempfile::TempDir, SqliteDriver) {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("test.db");
        let driver = SqliteDriver::connect(file.to_str().expect("utf-8 path"))
            .await
            .expect("connect");
        (dir, driver)
    }

    #[tokio::test]
    async fn streams_filtered_sorted_export_rows() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE orders (id INTEGER PRIMARY KEY, amount INTEGER, status TEXT)",
                1000,
            )
            .await
            .expect("create");
        driver
            .execute(
                "INSERT INTO orders (id, amount, status) VALUES (1, 30, 'paid'), (2, 10, 'paid'), (3, 20, 'new')",
                1000,
            )
            .await
            .expect("insert");
        let mut rows = Vec::new();
        let count = driver
            .export_rows_filtered(
                "orders",
                None,
                Some("status = 'paid'"),
                &[crate::models::TableSort {
                    column: "amount".to_string(),
                    direction: "desc".to_string(),
                }],
                &mut |row| {
                    rows.push(row);
                    Ok(())
                },
            )
            .await
            .expect("stream");
        assert_eq!(count, 2);
        assert_eq!(rows[0][1], serde_json::json!(30));
        assert_eq!(rows[1][1], serde_json::json!(10));
    }

    #[tokio::test]
    async fn lists_scope_foreign_keys_and_fills_implicit_target_columns() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE parent (id INTEGER PRIMARY KEY, x TEXT, y TEXT, UNIQUE (x, y))",
                1000,
            )
            .await
            .expect("parent");
        driver
            .execute(
                "CREATE TABLE child (pid INTEGER REFERENCES parent, a TEXT, b TEXT, FOREIGN KEY (a, b) REFERENCES parent (x, y))",
                1000,
            )
            .await
            .expect("child");
        let foreign_keys = driver.scope_foreign_keys().await.expect("scope fks");
        let implicit = foreign_keys
            .iter()
            .find(|foreign_key| foreign_key.table == "child" && foreign_key.columns == vec!["pid"])
            .expect("implicit fk");
        assert_eq!(implicit.referenced_table, "parent");
        assert_eq!(implicit.referenced_columns, vec!["id"]);
        let composite = foreign_keys
            .iter()
            .find(|foreign_key| foreign_key.table == "child" && foreign_key.columns.len() == 2)
            .expect("composite fk");
        assert_eq!(composite.columns, vec!["a", "b"]);
        assert_eq!(composite.referenced_columns, vec!["x", "y"]);
    }

    #[tokio::test]
    async fn executes_sql_and_maps_types() {
        let (_dir, driver) = test_driver().await;
        driver.test_connection().await.expect("test connection");

        let created = driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL, note TEXT)",
                1000,
            )
            .await
            .expect("create table");
        assert!(!created.is_query);

        let inserted = driver
            .execute(
                "INSERT INTO items (name, price, note) VALUES ('apple', 3.5, NULL)",
                1000,
            )
            .await
            .expect("insert");
        assert_eq!(inserted.rows_affected, 1);

        let result = driver
            .execute("SELECT id, name, price, note FROM items", 1000)
            .await
            .expect("select");

        assert!(result.is_query);
        assert_eq!(result.columns, vec!["id", "name", "price", "note"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], json!(1));
        assert_eq!(result.rows[0][1], json!("apple"));
        assert_eq!(result.rows[0][2], json!(3.5));
        assert_eq!(result.rows[0][3], Value::Null);
    }

    #[tokio::test]
    async fn lists_scope_columns_for_completion() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute("CREATE VIEW author_names AS SELECT name FROM authors", 1000)
            .await
            .expect("create view");

        let scope = driver.scope_columns().await.expect("scope columns");
        let authors = scope
            .iter()
            .find(|table| table.name == "authors")
            .expect("authors table");
        assert_eq!(authors.kind, "table");
        assert_eq!(
            authors
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "name"]
        );
        assert!(authors.columns[0].primary_key);

        let view = scope
            .iter()
            .find(|table| table.name == "author_names")
            .expect("view");
        assert_eq!(view.kind, "view");
        assert_eq!(view.columns.len(), 1);
    }

    #[tokio::test]
    async fn executes_transactions_atomically() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create table");

        let applied = driver
            .execute_transaction(&[
                "INSERT INTO items (id, name) VALUES (1, 'a')".to_string(),
                "INSERT INTO items (id, name) VALUES (2, 'b')".to_string(),
            ])
            .await
            .expect("commit");
        assert_eq!(applied, 2);

        let error = driver
            .execute_transaction(&[
                "INSERT INTO items (id, name) VALUES (3, 'c')".to_string(),
                "INSERT INTO items (id, name) VALUES (1, 'dup')".to_string(),
            ])
            .await
            .expect_err("rollback");
        assert!(error.contains("第 2 条语句"));
        assert!(error.contains("回滚"));

        let count = driver
            .execute("SELECT COUNT(*) FROM items", 1000)
            .await
            .expect("count");
        assert_eq!(count.rows[0][0], json!(2));

        let rolled_back = driver
            .execute("SELECT COUNT(*) FROM items WHERE id = 3", 1000)
            .await
            .expect("count id 3");
        assert_eq!(rolled_back.rows[0][0], json!(0));
    }

    #[tokio::test]
    async fn manual_transactions_commit_and_rollback() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE items (id INTEGER PRIMARY KEY)", 1000)
            .await
            .expect("create table");

        let mut session = driver.open_session().await.expect("session");
        session.begin().await.expect("begin");
        session
            .execute("INSERT INTO items VALUES (1)", 1000)
            .await
            .expect("insert");
        let inside = session
            .execute("SELECT COUNT(*) FROM items", 1000)
            .await
            .expect("count inside");
        assert_eq!(inside.rows[0][0], json!(1));
        session.rollback().await.expect("rollback");
        drop(session);

        let after = driver
            .execute("SELECT COUNT(*) FROM items", 1000)
            .await
            .expect("count after rollback");
        assert_eq!(after.rows[0][0], json!(0));

        let mut session = driver.open_session().await.expect("session");
        session.begin().await.expect("begin");
        session
            .execute("INSERT INTO items VALUES (2)", 1000)
            .await
            .expect("insert");
        session.commit().await.expect("commit");
        drop(session);

        let after = driver
            .execute("SELECT COUNT(*) FROM items", 1000)
            .await
            .expect("count after commit");
        assert_eq!(after.rows[0][0], json!(1));
    }

    #[tokio::test]
    async fn finds_index_owner_table() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute("CREATE INDEX idx_items_name ON items(name)", 1000)
            .await
            .expect("create index");

        assert_eq!(
            driver
                .index_table("idx_items_name")
                .await
                .expect("index table"),
            Some("items".to_string())
        );
        assert_eq!(
            driver.index_table("missing_index").await.expect("missing"),
            None
        );
    }

    #[tokio::test]
    async fn lists_tables_and_views() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY)", 1000)
            .await
            .expect("create table");
        driver
            .execute("CREATE VIEW user_ids AS SELECT id FROM users", 1000)
            .await
            .expect("create view");

        let objects = driver.list_tables().await.expect("list tables");
        let names: Vec<(&str, &str)> = objects
            .iter()
            .map(|item| (item.name.as_str(), item.kind.as_str()))
            .collect();

        assert!(names.contains(&("users", "table")));
        assert!(names.contains(&("user_ids", "view")));
    }

    #[tokio::test]
    async fn detects_queries_with_leading_comments() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE notes (body TEXT)", 1000)
            .await
            .expect("create table");
        driver
            .execute("INSERT INTO notes (body) VALUES ('hello')", 1000)
            .await
            .expect("insert");

        let result = driver
            .execute("-- preview\nSELECT body FROM notes", 1000)
            .await
            .expect("select");

        assert!(result.is_query);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], json!("hello"));
    }

    #[tokio::test]
    async fn reads_and_mutates_table_rows() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO items (name, price) VALUES ('a', 1.5), ('b', 2.5), ('c', 3.5)",
                1000,
            )
            .await
            .expect("insert");

        let columns = driver.list_columns("items").await.expect("columns");
        assert_eq!(columns.len(), 3);
        assert!(columns[0].primary_key);
        assert!(columns[0].auto_increment);

        let page = driver
            .read_rows("items", 2, 0, None, None, None)
            .await
            .expect("page");
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 2);

        driver
            .update_row(
                "items",
                &[ColumnValue {
                    column: "id".to_string(),
                    value: json!(1),
                }],
                &[ColumnValue {
                    column: "name".to_string(),
                    value: json!("aa"),
                }],
                false,
            )
            .await
            .expect("update");

        driver
            .insert_row(
                "items",
                &[
                    ColumnValue {
                        column: "name".to_string(),
                        value: json!("d"),
                    },
                    ColumnValue {
                        column: "price".to_string(),
                        value: json!(4.5),
                    },
                ],
            )
            .await
            .expect("insert row");

        driver
            .delete_row(
                "items",
                &[ColumnValue {
                    column: "id".to_string(),
                    value: json!(2),
                }],
                false,
            )
            .await
            .expect("delete");

        let all = driver
            .read_rows("items", 10, 0, None, None, None)
            .await
            .expect("all rows");
        assert_eq!(all.total, 3);

        let names: Vec<String> = all
            .rows
            .iter()
            .filter_map(|row| row[1].as_str().map(|value| value.to_string()))
            .collect();
        assert!(names.contains(&"aa".to_string()));
        assert!(names.contains(&"c".to_string()));
        assert!(names.contains(&"d".to_string()));
        assert!(!names.contains(&"b".to_string()));

        let sorted = driver
            .read_rows("items", 10, 0, None, Some("name"), Some("desc"))
            .await
            .expect("sorted rows");
        assert_eq!(sorted.rows[0][1], json!("d"));

        let filtered = driver
            .read_rows("items", 10, 0, Some("name = 'c'"), None, None)
            .await
            .expect("filtered rows");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.rows[0][1], json!("c"));
    }

    #[tokio::test]
    async fn reads_table_structure() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE parents (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create parents");
        driver
            .execute(
                "CREATE TABLE children (\
                   id INTEGER PRIMARY KEY, \
                   parent_id INTEGER REFERENCES parents(id), \
                   label TEXT NOT NULL DEFAULT 'x'\
                 )",
                1000,
            )
            .await
            .expect("create children");
        driver
            .execute("CREATE INDEX idx_children_label ON children(label)", 1000)
            .await
            .expect("create index");

        let structure = driver
            .table_structure("children")
            .await
            .expect("table structure");

        assert!(structure.ddl.contains("CREATE TABLE"));
        assert!(
            structure
                .columns
                .iter()
                .any(|column| column.name == "label"
                    && column.default_value.as_deref() == Some("'x'"))
        );
        assert!(
            structure
                .indexes
                .iter()
                .any(|index| index.name == "idx_children_label"
                    && index.columns == vec!["label".to_string()])
        );
        assert!(structure.foreign_keys.iter().any(|key| {
            key.referenced_table == "parents"
                && key.columns == vec!["parent_id".to_string()]
                && key.referenced_columns == vec!["id".to_string()]
        }));
    }

    #[tokio::test]
    async fn reads_renames_and_drops_objects() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute("CREATE VIEW item_names AS SELECT name FROM items", 1000)
            .await
            .expect("create view");
        driver
            .execute("CREATE INDEX idx_items_name ON items(name)", 1000)
            .await
            .expect("create index");
        driver
            .execute(
                "CREATE TRIGGER trg_items AFTER INSERT ON items BEGIN SELECT 1; END",
                1000,
            )
            .await
            .expect("create trigger");

        assert!(
            driver
                .object_ddl("table", "items")
                .await
                .expect("table ddl")
                .contains("CREATE TABLE")
        );
        assert!(
            driver
                .object_ddl("view", "item_names")
                .await
                .expect("view ddl")
                .contains("CREATE VIEW")
        );
        assert!(
            driver
                .object_ddl("index", "idx_items_name")
                .await
                .expect("index ddl")
                .contains("CREATE INDEX")
        );
        assert!(
            driver
                .object_ddl("trigger", "trg_items")
                .await
                .expect("trigger ddl")
                .contains("CREATE TRIGGER")
        );

        driver
            .rename_object("table", "items", "items2")
            .await
            .expect("rename table");
        let objects = driver.list_tables().await.expect("objects after rename");
        assert!(objects.iter().any(|item| item.name == "items2"));
        assert!(!objects.iter().any(|item| item.name == "items"));

        assert!(
            driver
                .rename_object("view", "item_names", "x")
                .await
                .is_err()
        );

        driver
            .drop_object("view", "item_names")
            .await
            .expect("drop view");
        driver
            .drop_object("index", "idx_items_name")
            .await
            .expect("drop index");
        driver
            .drop_object("trigger", "trg_items")
            .await
            .expect("drop trigger");
        driver
            .drop_object("table", "items2")
            .await
            .expect("drop table");

        let objects = driver.list_tables().await.expect("objects after drop");
        assert!(objects.is_empty());
    }

    #[tokio::test]
    async fn truncates_large_results() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE numbers (value INTEGER)", 1000)
            .await
            .expect("create table");
        driver
            .execute("INSERT INTO numbers (value) VALUES (1), (2), (3)", 1000)
            .await
            .expect("insert");

        let result = driver
            .execute("SELECT value FROM numbers ORDER BY value", 2)
            .await
            .expect("select");

        assert_eq!(result.rows.len(), 2);
        assert!(result.truncated);
    }

    #[tokio::test]
    async fn duplicates_table_and_view_and_snapshots() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO items (id, name) VALUES (1, 'apple'), (2, 'pear')",
                1000,
            )
            .await
            .expect("insert");
        driver
            .execute("CREATE VIEW item_names AS SELECT id, name FROM items", 1000)
            .await
            .expect("create view");
        driver
            .execute("CREATE INDEX idx_items_name ON items(name)", 1000)
            .await
            .expect("create index");

        driver
            .duplicate_table("items", "items_copy")
            .await
            .expect("copy table");
        let copied = driver
            .read_rows("items_copy", 10, 0, None, None, None)
            .await
            .expect("read copied table");
        assert_eq!(copied.rows.len(), 2);
        let copied_ddl = driver
            .object_ddl("table", "items_copy")
            .await
            .expect("copied ddl");
        assert!(copied_ddl.contains("items_copy"));
        assert!(copied_ddl.contains("PRIMARY KEY"));
        let copied_structure = driver
            .table_structure("items_copy")
            .await
            .expect("copied structure");
        assert!(
            copied_structure
                .indexes
                .iter()
                .any(|index| index.name == "items_copy_idx_items_name")
        );

        driver
            .duplicate_view("item_names", "item_names_copy")
            .await
            .expect("copy view");
        let copied_view = driver
            .read_rows("item_names_copy", 10, 0, None, None, None)
            .await
            .expect("read copied view");
        assert_eq!(copied_view.rows.len(), 2);

        driver
            .snapshot_view("item_names", "names_empty", false)
            .await
            .expect("empty snapshot");
        let empty = driver
            .read_rows("names_empty", 10, 0, None, None, None)
            .await
            .expect("read empty snapshot");
        assert!(empty.rows.is_empty());
        assert_eq!(empty.columns.len(), 2);

        driver
            .snapshot_view("item_names", "names_filled", true)
            .await
            .expect("filled snapshot");
        let filled = driver
            .read_rows("names_filled", 10, 0, None, None, None)
            .await
            .expect("read filled snapshot");
        assert_eq!(filled.rows.len(), 2);
    }

    #[tokio::test]
    async fn builds_er_diagram_with_relations() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create users");
        driver
            .execute(
                "CREATE TABLE orders (\
                   id INTEGER PRIMARY KEY, \
                   user_id INTEGER REFERENCES users(id), \
                   total REAL\
                 )",
                1000,
            )
            .await
            .expect("create orders");
        driver
            .execute(
                "CREATE TABLE logs (id INTEGER PRIMARY KEY, message TEXT)",
                1000,
            )
            .await
            .expect("create logs");
        driver
            .execute(
                "CREATE VIEW order_totals AS \
                 SELECT user_id, SUM(total) AS total FROM orders GROUP BY user_id",
                1000,
            )
            .await
            .expect("create view");

        let diagram = driver.er_diagram().await.expect("diagram");
        assert_eq!(diagram.tables.len(), 4);
        assert!(diagram.relations.iter().any(|relation| {
            relation.from_table == "orders"
                && relation.to_table == "users"
                && relation.from_columns == vec!["user_id"]
                && relation.to_columns == vec!["id"]
        }));

        let orders = diagram
            .tables
            .iter()
            .find(|table| table.name == "orders")
            .expect("orders node");
        assert!(
            orders
                .columns
                .iter()
                .any(|column| column.name == "id" && column.primary_key)
        );
        assert_eq!(
            orders
                .columns
                .iter()
                .find(|column| column.name == "user_id")
                .expect("user_id column")
                .data_type,
            "INTEGER"
        );

        let view = diagram
            .tables
            .iter()
            .find(|table| table.name == "order_totals")
            .expect("view node");
        assert_eq!(view.kind, "view");
        assert!(view.columns.iter().any(|column| column.name == "user_id"));
    }

    #[tokio::test]
    async fn streams_export_rows() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE items (id INTEGER, note TEXT)", 1000)
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO items (id, note) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
                1000,
            )
            .await
            .expect("insert");

        let mut seen = Vec::new();
        let count = driver
            .export_rows("items", Some(2), &mut |row| {
                seen.push(row);
                Ok(())
            })
            .await
            .expect("export limited");
        assert_eq!(count, 2);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0][0], json!(1));

        let mut total = 0u64;
        driver
            .export_rows("items", None, &mut |_row| {
                total += 1;
                Ok(())
            })
            .await
            .expect("export all");
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn updates_and_deletes_one_duplicate_row_without_primary_key() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE notes (message TEXT, tag TEXT)", 1000)
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO notes (message, tag) VALUES ('same', 'x'), ('same', 'x')",
                1000,
            )
            .await
            .expect("insert");

        let keys = vec![
            ColumnValue {
                column: "message".to_string(),
                value: json!("same"),
            },
            ColumnValue {
                column: "tag".to_string(),
                value: json!("x"),
            },
        ];
        let updated = driver
            .update_row(
                "notes",
                &keys,
                &[ColumnValue {
                    column: "tag".to_string(),
                    value: json!("y"),
                }],
                true,
            )
            .await
            .expect("update one");
        assert_eq!(updated, 1);

        let rows = driver
            .read_rows("notes", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 2);
        assert_eq!(
            rows.rows.iter().filter(|row| row[1] == json!("y")).count(),
            1
        );

        let remaining = vec![
            ColumnValue {
                column: "message".to_string(),
                value: json!("same"),
            },
            ColumnValue {
                column: "tag".to_string(),
                value: json!("y"),
            },
        ];
        let deleted = driver
            .delete_row("notes", &remaining, true)
            .await
            .expect("delete one");
        assert_eq!(deleted, 1);
        let rows = driver
            .read_rows("notes", 10, 0, None, None, None)
            .await
            .expect("rows after delete");
        assert_eq!(rows.rows.len(), 1);
        assert_eq!(rows.rows[0][1], json!("x"));
    }

    #[tokio::test]
    async fn updates_null_key_columns_with_is_null() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE notes (message TEXT, tag TEXT)", 1000)
            .await
            .expect("create table");
        driver
            .execute("INSERT INTO notes (message, tag) VALUES ('a', NULL)", 1000)
            .await
            .expect("insert");

        let keys = vec![
            ColumnValue {
                column: "message".to_string(),
                value: json!("a"),
            },
            ColumnValue {
                column: "tag".to_string(),
                value: Value::Null,
            },
        ];
        let updated = driver
            .update_row(
                "notes",
                &keys,
                &[ColumnValue {
                    column: "tag".to_string(),
                    value: json!("b"),
                }],
                true,
            )
            .await
            .expect("update null key");
        assert_eq!(updated, 1);
        let rows = driver
            .read_rows("notes", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert_eq!(rows.rows[0][1], json!("b"));
    }

    #[tokio::test]
    async fn dumps_sql_literals_with_binary_and_quotes() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE dump_items (\
                   id INTEGER PRIMARY KEY, \
                   name TEXT, \
                   note TEXT, \
                   payload BLOB\
                 )",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO dump_items (id, name, note, payload) \
                 VALUES (1, 'O''Brien', NULL, X'00FF10')",
                1000,
            )
            .await
            .expect("insert");
        driver
            .execute("CREATE INDEX idx_dump_items_name ON dump_items(name)", 1000)
            .await
            .expect("create index");

        let (columns, clause) = driver.dump_insert_plan("dump_items").await.expect("plan");
        assert_eq!(columns, vec!["id", "name", "note", "payload"]);
        assert!(clause.is_empty());

        let mut rows = Vec::new();
        let count = driver
            .stream_sql_literal_rows("dump_items", &columns, &mut |values| {
                rows.push(values);
                Ok(())
            })
            .await
            .expect("dump rows");
        assert_eq!(count, 1);
        let values = rows[0]
            .iter()
            .map(|value| value.as_deref().unwrap_or("NULL"))
            .collect::<Vec<_>>();
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "'O''Brien'");
        assert_eq!(values[2], "NULL");
        assert_eq!(values[3], "X'00FF10'");

        let ddl = driver.dump_table_ddl("dump_items").await.expect("ddl");
        assert!(ddl.contains("CREATE TABLE dump_items"));
        let indexes = driver
            .dump_index_statements("dump_items")
            .await
            .expect("indexes");
        assert_eq!(indexes.len(), 1);
        assert!(indexes[0].contains("idx_dump_items_name"));
        assert!(driver.dump_indexes_separately());
        assert!(!driver.dump_foreign_keys_separately());

        let target_dir = tempfile::tempdir().expect("target dir");
        let target_file = target_dir.path().join("target.db");
        let target = SqliteDriver::connect(target_file.to_str().expect("path"))
            .await
            .expect("target connect");
        target
            .execute(&ddl, 1000)
            .await
            .expect("create target table");
        target
            .execute(
                &format!(
                    "INSERT INTO dump_items ({}) VALUES ({})",
                    columns
                        .iter()
                        .map(|column| quote_identifier(column))
                        .collect::<Vec<_>>()
                        .join(", "),
                    values.join(", ")
                ),
                1000,
            )
            .await
            .expect("insert literals");
        let check = target
            .execute(
                "SELECT name, note IS NULL, hex(payload) FROM dump_items",
                10,
            )
            .await
            .expect("check target");
        assert_eq!(check.rows[0][0], json!("O'Brien"));
        assert_eq!(check.rows[0][1], json!(1));
        assert_eq!(check.rows[0][2], json!("00FF10"));
        target
            .execute(&indexes[0], 1000)
            .await
            .expect("create target index");
    }

    #[tokio::test]
    async fn applies_batch_changes_atomically_and_sorts_multiple_columns() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (\
                   id INTEGER PRIMARY KEY AUTOINCREMENT, \
                   name TEXT, \
                   status TEXT\
                 )",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute(
                "INSERT INTO items (name, status) VALUES \
                 ('a', 'active'), ('b', 'active'), ('c', 'paused')",
                1000,
            )
            .await
            .expect("insert");

        let changes = vec![
            crate::models::PendingTableChange {
                kind: "delete".to_string(),
                keys: vec![ColumnValue {
                    column: "id".to_string(),
                    value: json!(3),
                }],
                changes: Vec::new(),
                values: Vec::new(),
                single_row: false,
            },
            crate::models::PendingTableChange {
                kind: "update".to_string(),
                keys: vec![ColumnValue {
                    column: "id".to_string(),
                    value: json!(1),
                }],
                changes: vec![ColumnValue {
                    column: "name".to_string(),
                    value: json!("aa"),
                }],
                values: Vec::new(),
                single_row: false,
            },
            crate::models::PendingTableChange {
                kind: "insert".to_string(),
                keys: Vec::new(),
                changes: Vec::new(),
                values: vec![
                    ColumnValue {
                        column: "name".to_string(),
                        value: json!("d"),
                    },
                    ColumnValue {
                        column: "status".to_string(),
                        value: json!("active"),
                    },
                ],
                single_row: false,
            },
        ];
        let applied = driver
            .apply_table_changes("items", &changes)
            .await
            .expect("apply");
        assert_eq!(applied, 3);
        let rows = driver
            .read_rows("items", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 3);

        let failing = vec![
            crate::models::PendingTableChange {
                kind: "delete".to_string(),
                keys: vec![ColumnValue {
                    column: "id".to_string(),
                    value: json!(2),
                }],
                changes: Vec::new(),
                values: Vec::new(),
                single_row: false,
            },
            crate::models::PendingTableChange {
                kind: "update".to_string(),
                keys: vec![ColumnValue {
                    column: "id".to_string(),
                    value: json!(999),
                }],
                changes: vec![ColumnValue {
                    column: "name".to_string(),
                    value: json!("missing"),
                }],
                values: Vec::new(),
                single_row: false,
            },
        ];
        let error = driver
            .apply_table_changes("items", &failing)
            .await
            .expect_err("missing row must fail");
        assert!(error.contains("目标行已不存在"));
        let rows = driver
            .read_rows("items", 10, 0, None, None, None)
            .await
            .expect("rows after rollback");
        assert_eq!(rows.rows.len(), 3, "delete must be rolled back");

        let sorts = vec![crate::models::TableSort {
            column: "status".to_string(),
            direction: "desc".to_string(),
        }];
        let sorted = driver
            .read_rows_sorted("items", 10, 0, None, &sorts)
            .await
            .expect("sorted rows");
        assert_eq!(sorted.rows.len(), 3);
        assert!(sorted.rows.iter().all(|row| row[2] == json!("active")));
        let first_id = sorted.rows[0][0].as_i64().expect("id");
        let second_id = sorted.rows[1][0].as_i64().expect("id");
        let third_id = sorted.rows[2][0].as_i64().expect("id");
        assert!(
            first_id < second_id && second_id < third_id,
            "primary key tiebreaker must apply"
        );
    }

    #[tokio::test]
    async fn rebuilds_column_and_preserves_indexes_triggers_and_data() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (\
                   id INTEGER PRIMARY KEY AUTOINCREMENT, \
                   name TEXT NOT NULL DEFAULT 'n/a', \
                   qty TEXT\
                 )",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute("CREATE INDEX idx_items_name ON items(name)", 1000)
            .await
            .expect("create index");
        driver
            .execute(
                "CREATE TRIGGER trg_items AFTER INSERT ON items \
                 BEGIN UPDATE items SET name = name WHERE id = new.id; END",
                1000,
            )
            .await
            .expect("create trigger");
        driver
            .execute(
                "INSERT INTO items (name, qty) VALUES ('a', '1'), ('b', '2')",
                1000,
            )
            .await
            .expect("insert");

        driver
            .rebuild_column("items", "qty", "quantity", "INTEGER", false, Some("0"))
            .await
            .expect("rebuild column");

        let structure = driver.table_structure("items").await.expect("structure");
        let quantity = structure
            .columns
            .iter()
            .find(|column| column.name == "quantity")
            .expect("renamed column");
        assert_eq!(quantity.data_type, "INTEGER");
        assert!(!quantity.nullable);
        assert_eq!(quantity.default_value.as_deref(), Some("0"));
        assert!(
            structure
                .indexes
                .iter()
                .any(|index| index.name == "idx_items_name")
        );

        let rows = driver
            .read_rows("items", 10, 0, None, Some("id"), Some("asc"))
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 2);
        assert_eq!(rows.rows[0][2], json!(1));
        assert_eq!(rows.rows[1][2], json!(2));

        driver
            .execute("INSERT INTO items (name) VALUES ('c')", 1000)
            .await
            .expect("insert after rebuild");
        let rows = driver
            .read_rows("items", 10, 0, None, Some("id"), Some("asc"))
            .await
            .expect("rows after insert");
        assert_eq!(rows.rows.len(), 3);
        assert_eq!(rows.rows[2][0], json!(3));
        assert_eq!(rows.rows[2][2], json!(0));

        let objects = driver
            .execute(
                "SELECT type, name FROM sqlite_master \
                 WHERE name IN ('idx_items_name', 'trg_items') ORDER BY name",
                100,
            )
            .await
            .expect("objects");
        assert_eq!(objects.rows.len(), 2);
    }

    #[tokio::test]
    async fn rebuild_column_rolls_back_when_existing_data_violates_not_null() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute("CREATE TABLE notes (id INTEGER, note TEXT)", 1000)
            .await
            .expect("create table");
        driver
            .execute("INSERT INTO notes (id, note) VALUES (1, NULL)", 1000)
            .await
            .expect("insert");

        let error = driver
            .rebuild_column("notes", "note", "note", "TEXT", false, None)
            .await
            .expect_err("NOT NULL should fail");
        assert!(error.contains("复制表数据失败"));

        let structure = driver.table_structure("notes").await.expect("structure");
        let note = structure
            .columns
            .iter()
            .find(|column| column.name == "note")
            .expect("note column");
        assert!(note.nullable);
        let rows = driver
            .read_rows("notes", 10, 0, None, None, None)
            .await
            .expect("rows");
        assert_eq!(rows.rows.len(), 1);
        assert_eq!(rows.rows[0][1], json!(null));
    }

    #[tokio::test]
    async fn saves_view_and_trigger_definitions_in_a_transaction() {
        let (_dir, driver) = test_driver().await;
        driver
            .execute(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
                1000,
            )
            .await
            .expect("create table");
        driver
            .execute("CREATE VIEW item_names AS SELECT name FROM items", 1000)
            .await
            .expect("create view");
        driver
            .execute(
                "CREATE TRIGGER trg_items AFTER INSERT ON items BEGIN SELECT 1; END",
                1000,
            )
            .await
            .expect("create trigger");

        driver
            .save_object_definition(
                "view",
                "item_names",
                "CREATE VIEW item_names AS SELECT name FROM items WHERE name IS NOT NULL",
            )
            .await
            .expect("save view");
        let view = driver
            .object_ddl("view", "item_names")
            .await
            .expect("view ddl");
        assert!(view.contains("WHERE name IS NOT NULL"));

        driver
            .save_object_definition(
                "trigger",
                "trg_items",
                "CREATE TRIGGER trg_items AFTER INSERT ON items BEGIN SELECT 2; END",
            )
            .await
            .expect("save trigger");
        let trigger = driver
            .object_ddl("trigger", "trg_items")
            .await
            .expect("trigger ddl");
        assert!(trigger.contains("SELECT 2"));

        let previous = driver
            .object_ddl("view", "item_names")
            .await
            .expect("previous view");
        assert!(
            driver
                .save_object_definition(
                    "view",
                    "item_names",
                    "CREATE VIEW item_names AS SELEC broken",
                )
                .await
                .is_err()
        );
        assert_eq!(
            driver
                .object_ddl("view", "item_names")
                .await
                .expect("view survives failed save"),
            previous
        );
    }
}

fn value_to_json(row: &SqliteRow, index: usize) -> Value {
    let type_name = row.column(index).type_info().name().to_ascii_uppercase();

    match type_name.as_str() {
        "INTEGER" | "INT" | "BIGINT" | "SMALLINT" | "TINYINT" => row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "REAL" | "DOUBLE" | "FLOAT" | "NUMERIC" | "DECIMAL" => row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" => row
            .try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
        "BLOB" => row
            .try_get::<Option<Vec<u8>>, _>(index)
            .ok()
            .flatten()
            .map(|bytes| Value::String(format!("<BLOB {} bytes>", bytes.len())))
            .unwrap_or(Value::Null),
        _ => {
            if let Ok(Some(value)) = row.try_get::<Option<i64>, _>(index) {
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
