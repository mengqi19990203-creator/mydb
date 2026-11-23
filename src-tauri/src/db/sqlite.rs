use crate::db::{DatabaseDriver, DatabaseSession};
use crate::db::query::is_read_query;
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
        sqlx::query_scalar(
            "SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = ?",
        )
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

        let set_clause = changes
            .iter()
            .map(|item| format!("{} = ?", quote_identifier(&item.column)))
            .collect::<Vec<_>>()
            .join(", ");
        let where_clause = keys
            .iter()
            .map(|item| format!("{} = ?", quote_identifier(&item.column)))
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "UPDATE {} SET {set_clause} WHERE {where_clause}",
            quote_identifier(table)
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for change in changes {
            query = bind_json_sqlite(query, &change.value);
        }
        for key in keys {
            query = bind_json_sqlite(query, &key.value);
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

    async fn delete_row(&self, table: &str, keys: &[ColumnValue]) -> Result<u64, String> {
        if keys.is_empty() {
            return Err("该表没有主键，无法安全删除".to_string());
        }

        let where_clause = keys
            .iter()
            .map(|item| format!("{} = ?", quote_identifier(&item.column)))
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "DELETE FROM {} WHERE {where_clause}",
            quote_identifier(table)
        );

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for key in keys {
            query = bind_json_sqlite(query, &key.value);
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

    async fn rename_object(
        &self,
        kind: &str,
        name: &str,
        new_name: &str,
    ) -> Result<(), String> {
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
        if let Some(to) = to {
            entry.referenced_columns.push(to);
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
            .execute(
                "CREATE VIEW author_names AS SELECT name FROM authors",
                1000,
            )
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
            driver.index_table("idx_items_name").await.expect("index table"),
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
            .execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", 1000)
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

        assert!(driver
            .object_ddl("table", "items")
            .await
            .expect("table ddl")
            .contains("CREATE TABLE"));
        assert!(driver
            .object_ddl("view", "item_names")
            .await
            .expect("view ddl")
            .contains("CREATE VIEW"));
        assert!(driver
            .object_ddl("index", "idx_items_name")
            .await
            .expect("index ddl")
            .contains("CREATE INDEX"));
        assert!(driver
            .object_ddl("trigger", "trg_items")
            .await
            .expect("trigger ddl")
            .contains("CREATE TRIGGER"));

        driver
            .rename_object("table", "items", "items2")
            .await
            .expect("rename table");
        let objects = driver.list_tables().await.expect("objects after rename");
        assert!(objects.iter().any(|item| item.name == "items2"));
        assert!(!objects.iter().any(|item| item.name == "items"));

        assert!(driver.rename_object("view", "item_names", "x").await.is_err());

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
        assert!(copied_structure
            .indexes
            .iter()
            .any(|index| index.name == "items_copy_idx_items_name"));

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
            .execute("CREATE TABLE logs (id INTEGER PRIMARY KEY, message TEXT)", 1000)
            .await
            .expect("create logs");

        let diagram = driver.er_diagram().await.expect("diagram");
        assert_eq!(diagram.tables.len(), 3);
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
        assert!(orders
            .columns
            .iter()
            .any(|column| column.name == "id" && column.primary_key));
        assert_eq!(
            orders
                .columns
                .iter()
                .find(|column| column.name == "user_id")
                .expect("user_id column")
                .data_type,
            "INTEGER"
        );
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
