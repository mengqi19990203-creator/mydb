use crate::models::{
    ColumnInfo, ColumnValue, DatabaseObjects, ErColumn, ErDiagram, ErRelation, ErTable,
    ForeignKeyInfo, QueryResult, TableInfo, TableRows, TableStructure,
};
use async_trait::async_trait;
use std::collections::HashSet;

/// A pinned connection with an explicit transaction, used by the SQL editor's
/// manual BEGIN / COMMIT / ROLLBACK controls.
#[async_trait]
pub trait DatabaseSession: Send {
    async fn begin(&mut self) -> Result<(), String>;
    async fn execute(&mut self, sql: &str, max_rows: usize) -> Result<QueryResult, String>;
    async fn commit(&mut self) -> Result<(), String>;
    async fn rollback(&mut self) -> Result<(), String>;
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn list_databases(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    /// MySQL: switches the current database inside the existing connection.
    async fn switch_database(&self, _database: &str) -> Result<(), String> {
        Err("当前连接不支持直接切换数据库".to_string())
    }
    /// Objects for every database, fetched in one metadata pass.
    async fn list_database_objects(&self) -> Result<Vec<DatabaseObjects>, String> {
        Ok(Vec::new())
    }
    /// PostgreSQL-style schemas inside the current database. Empty for
    /// drivers whose object tree has no schema level.
    async fn list_schemas(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    /// Makes `schema` the default namespace for subsequent object/data
    /// operations. Only PostgreSQL implements this today.
    async fn switch_schema(&self, _schema: &str) -> Result<(), String> {
        Err("当前连接不支持切换模式".to_string())
    }
    /// The database / schema currently used for unqualified names. Used to
    /// describe the source and target of a sync comparison, and to resolve
    /// same-connection cross-database / cross-schema targets.
    async fn context(&self) -> (Option<String>, Option<String>) {
        (None, None)
    }
    async fn list_tables(&self) -> Result<Vec<TableInfo>, String>;
    /// Owning table of an index, when the driver exposes standalone indexes.
    async fn index_table(&self, _index_name: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    /// DDL for a table, view or other named object where the driver supports it.
    async fn object_ddl(&self, _kind: &str, _name: &str) -> Result<String, String> {
        Err("当前连接不支持读取该对象的 DDL".to_string())
    }
    /// Renames an object in place.
    async fn rename_object(
        &self,
        _kind: &str,
        _name: &str,
        _new_name: &str,
    ) -> Result<(), String> {
        Err("当前连接不支持重命名该对象".to_string())
    }
    /// Drops an object. Callers are responsible for confirming with the user.
    async fn drop_object(&self, _kind: &str, _name: &str) -> Result<(), String> {
        Err("当前连接不支持删除该对象".to_string())
    }
    async fn execute(&self, sql: &str, max_rows: usize) -> Result<QueryResult, String>;
    /// Opens a dedicated connection for an interactive transaction.
    async fn open_session(&self) -> Result<Box<dyn DatabaseSession>, String> {
        Err("当前连接不支持手动事务".to_string())
    }
    /// Executes every statement inside one transaction. A failure rolls the
    /// whole batch back. Used by table data sync so a partial apply cannot
    /// leave the target half-written.
    async fn execute_transaction(&self, _statements: &[String]) -> Result<u64, String> {
        Err("当前连接不支持事务批量执行".to_string())
    }

    async fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, String>;
    async fn table_structure(&self, table: &str) -> Result<TableStructure, String>;
    async fn read_rows(
        &self,
        table: &str,
        limit: u64,
        offset: u64,
        filter: Option<&str>,
        order_by: Option<&str>,
        order_dir: Option<&str>,
    ) -> Result<TableRows, String>;
    async fn update_row(
        &self,
        table: &str,
        keys: &[ColumnValue],
        changes: &[ColumnValue],
    ) -> Result<u64, String>;
    async fn insert_row(&self, table: &str, values: &[ColumnValue]) -> Result<u64, String>;
    async fn delete_row(&self, table: &str, keys: &[ColumnValue]) -> Result<u64, String>;
    /// Foreign keys declared on `table`, used by the ER diagram.
    async fn list_foreign_keys(&self, _table: &str) -> Result<Vec<ForeignKeyInfo>, String> {
        Ok(Vec::new())
    }
    /// Tables and foreign-key relations for the current database/schema.
    async fn er_diagram(&self) -> Result<ErDiagram, String> {
        let tables = self.list_tables().await?;
        let names: HashSet<&str> = tables
            .iter()
            .filter(|table| table.kind == "table")
            .map(|table| table.name.as_str())
            .collect();

        let mut nodes = Vec::new();
        let mut relations = Vec::new();
        for table in tables.iter().filter(|table| table.kind == "table") {
            let columns = self.list_columns(&table.name).await?;
            nodes.push(ErTable {
                name: table.name.clone(),
                columns: columns
                    .into_iter()
                    .map(|column| ErColumn {
                        name: column.name,
                        data_type: column.data_type,
                        primary_key: column.primary_key,
                    })
                    .collect(),
            });

            for key in self.list_foreign_keys(&table.name).await? {
                if !names.contains(key.referenced_table.as_str()) {
                    continue;
                }
                relations.push(ErRelation {
                    name: key.name,
                    from_table: table.name.clone(),
                    from_columns: key.columns,
                    to_table: key.referenced_table,
                    to_columns: key.referenced_columns,
                });
            }
        }

        Ok(ErDiagram { tables: nodes, relations })
    }
    /// Streams every row of `table` (or the first `limit` rows) into `sink`.
    /// The default implementation buffers via `read_rows`; drivers override it
    /// with a cursor-based loop so exports do not hold the table in memory.
    async fn export_rows(
        &self,
        table: &str,
        limit: Option<u64>,
        sink: &mut (dyn FnMut(Vec<serde_json::Value>) -> Result<(), String> + Send),
    ) -> Result<u64, String> {
        let rows = self
            .read_rows(
                table,
                limit.unwrap_or(i64::MAX as u64),
                0,
                None,
                None,
                None,
            )
            .await?;
        let mut count = 0u64;
        for row in rows.rows {
            sink(row)?;
            count += 1;
        }
        Ok(count)
    }
    /// Creates `target` as a structure-and-data copy of table `source`.
    async fn duplicate_table(&self, _source: &str, _target: &str) -> Result<(), String> {
        Err("当前连接不支持复制表".to_string())
    }
    /// Creates `target` as a copy of view `source` with the same definition.
    async fn duplicate_view(&self, _source: &str, _target: &str) -> Result<(), String> {
        Err("当前连接不支持复制视图".to_string())
    }
    /// Creates table `target` from view `source` via CREATE TABLE AS SELECT.
    async fn snapshot_view(
        &self,
        _source: &str,
        _target: &str,
        _with_data: bool,
    ) -> Result<(), String> {
        Err("当前连接不支持将视图快照为表".to_string())
    }
}

/// Replaces the object name in a `CREATE [modifiers] TABLE|VIEW <name>`
/// statement. `replacement` must already be a quoted identifier, for example
/// `"schema"."new_name"` or `` `db`.`new_name` ``.
pub fn rename_ddl_object(ddl: &str, replacement: &str) -> Result<String, String> {
    let bytes = ddl.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            quote @ (b'\'' | b'"' | b'`') => {
                index = skip_quoted(ddl, index, quote)
                    .ok_or_else(|| "DDL 中的引号未闭合".to_string())?;
            }
            b'[' => {
                index = ddl[index..]
                    .find(']')
                    .map(|offset| index + offset + 1)
                    .ok_or_else(|| "DDL 中的方括号未闭合".to_string())?;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric()
                        || bytes[index] == b'_'
                        || bytes[index] == b'$')
                {
                    index += 1;
                }
                let word = ddl[start..index].to_ascii_uppercase();
                if word == "TABLE" || word == "VIEW" {
                    let name_start = skip_words(ddl, index, &["IF", "NOT", "EXISTS"]);
                    let (start, end) = parse_qualified_identifier(ddl, name_start)?;
                    return Ok(format!(
                        "{}{}{}",
                        &ddl[..start],
                        replacement,
                        &ddl[end..]
                    ));
                }
            }
            _ => index += 1,
        }
    }
    Err("无法在 DDL 中定位对象名".to_string())
}

/// Replaces the index name and the table name in a `CREATE [UNIQUE] INDEX`
/// statement. Both replacements must already be quoted identifiers.
pub fn rename_index_ddl(
    ddl: &str,
    replacement_index: &str,
    replacement_table: &str,
) -> Result<String, String> {
    let result = replace_identifier_after_keyword(ddl, "INDEX", replacement_index)?;
    replace_identifier_after_keyword(&result, "ON", replacement_table)
}

fn replace_identifier_after_keyword(
    ddl: &str,
    keyword: &str,
    replacement: &str,
) -> Result<String, String> {
    let bytes = ddl.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            quote @ (b'\'' | b'"' | b'`') => {
                index = skip_quoted(ddl, index, quote)
                    .ok_or_else(|| "DDL 中的引号未闭合".to_string())?;
            }
            b'[' => {
                index = ddl[index..]
                    .find(']')
                    .map(|offset| index + offset + 1)
                    .ok_or_else(|| "DDL 中的方括号未闭合".to_string())?;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric()
                        || bytes[index] == b'_'
                        || bytes[index] == b'$')
                {
                    index += 1;
                }
                if ddl[start..index].eq_ignore_ascii_case(keyword) {
                    let name_start = skip_words(ddl, index, &["IF", "NOT", "EXISTS"]);
                    let (start, end) = parse_qualified_identifier(ddl, name_start)?;
                    return Ok(format!(
                        "{}{}{}",
                        &ddl[..start],
                        replacement,
                        &ddl[end..]
                    ));
                }
            }
            _ => index += 1,
        }
    }
    Err(format!("无法在 DDL 中定位 {keyword} 子句"))
}

/// Removes the `DEFINER=...` clause from a MySQL `SHOW CREATE VIEW` result so
/// the copy can be created by a different account.
pub fn strip_mysql_definer(ddl: &str) -> String {
    let upper = ddl.to_ascii_uppercase();
    let Some(position) = upper.find("DEFINER=") else {
        return ddl.to_string();
    };
    let bytes = ddl.as_bytes();
    let mut end = position + "DEFINER=".len();
    while end < bytes.len() {
        match bytes[end] {
            b' ' | b'\t' | b'\r' | b'\n' => break,
            quote @ (b'\'' | b'"' | b'`') => {
                end = skip_quoted(ddl, end, quote).unwrap_or(bytes.len());
            }
            _ => end += 1,
        }
    }
    let mut result = String::with_capacity(ddl.len());
    result.push_str(&ddl[..position]);
    result.push_str(&ddl[end..]);
    result
}

fn skip_quoted(ddl: &str, start: usize, quote: u8) -> Option<usize> {
    let bytes = ddl.as_bytes();
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == quote {
            if index + 1 < bytes.len() && bytes[index + 1] == quote {
                index += 2;
                continue;
            }
            return Some(index + 1);
        }
        if quote == b'\'' && bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
            continue;
        }
        index += 1;
    }
    None
}

fn skip_words(ddl: &str, mut index: usize, words: &[&str]) -> usize {
    let bytes = ddl.as_bytes();
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
        {
            index += 1;
        }
        let word = ddl[start..index].to_ascii_uppercase();
        if words.iter().any(|item| *item == word) {
            continue;
        }
        return start;
    }
}

fn parse_qualified_identifier(ddl: &str, start: usize) -> Result<(usize, usize), String> {
    let (_, mut end) = parse_identifier(ddl, start)?;
    let bytes = ddl.as_bytes();
    loop {
        let mut next = end;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if next < bytes.len() && bytes[next] == b'.' {
            let mut after = next + 1;
            while after < bytes.len() && bytes[after].is_ascii_whitespace() {
                after += 1;
            }
            let (_, identifier_end) = parse_identifier(ddl, after)?;
            end = identifier_end;
        } else {
            break;
        }
    }
    Ok((start, end))
}

fn parse_identifier(ddl: &str, start: usize) -> Result<(usize, usize), String> {
    let bytes = ddl.as_bytes();
    if start >= bytes.len() {
        return Err("DDL 中缺少对象名".to_string());
    }
    let end = match bytes[start] {
        quote @ (b'"' | b'`') => skip_quoted(ddl, start, quote)
            .ok_or_else(|| "DDL 中的引号未闭合".to_string())?,
        b'[' => ddl[start..]
            .find(']')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| "DDL 中的方括号未闭合".to_string())?,
        byte if byte.is_ascii_alphabetic() || byte == b'_' => {
            let mut index = start + 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'_'
                    || bytes[index] == b'$')
            {
                index += 1;
            }
            index
        }
        _ => return Err("DDL 中缺少对象名".to_string()),
    };
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_plain_table() {
        let ddl = "CREATE TABLE \"items\" (id INTEGER)";
        assert_eq!(
            rename_ddl_object(ddl, "\"items_copy\"").expect("rename"),
            "CREATE TABLE \"items_copy\" (id INTEGER)"
        );
    }

    #[test]
    fn renames_unquoted_table_with_if_not_exists() {
        let ddl = "CREATE TABLE IF NOT EXISTS items (id INTEGER)";
        assert_eq!(
            rename_ddl_object(ddl, "\"items_copy\"").expect("rename"),
            "CREATE TABLE IF NOT EXISTS \"items_copy\" (id INTEGER)"
        );
    }

    #[test]
    fn renames_schema_qualified_view() {
        let ddl =
            "CREATE VIEW \"public\".\"item_names\" AS\n SELECT id FROM \"public\".\"items\";";
        assert_eq!(
            rename_ddl_object(ddl, "\"public\".\"item_names_copy\"").expect("rename"),
            "CREATE VIEW \"public\".\"item_names_copy\" AS\n SELECT id FROM \"public\".\"items\";"
        );
    }

    #[test]
    fn renames_mysql_view_and_strips_definer() {
        let ddl = "CREATE ALGORITHM=UNDEFINED DEFINER=`root`@`localhost` SQL SECURITY DEFINER VIEW `v` AS select 1 AS `x`";
        let stripped = strip_mysql_definer(ddl);
        assert!(!stripped.contains("DEFINER="));
        let renamed = rename_ddl_object(&stripped, "`v_copy`").expect("rename");
        assert!(renamed.contains("VIEW `v_copy` AS"));
    }

    #[test]
    fn ignores_keywords_inside_string_literals() {
        let ddl = "CREATE VIEW \"v\" AS SELECT 'TABLE x VIEW y' AS note";
        let renamed = rename_ddl_object(ddl, "\"w\"").expect("rename");
        assert!(renamed.starts_with("CREATE VIEW \"w\" AS"));
    }

    #[test]
    fn renames_sqlite_index() {
        let ddl = "CREATE UNIQUE INDEX idx_items_name ON items(name)";
        let renamed = rename_index_ddl(
            ddl,
            "\"items_copy_idx_items_name\"",
            "\"items_copy\"",
        )
        .expect("rename index");
        assert_eq!(
            renamed,
            "CREATE UNIQUE INDEX \"items_copy_idx_items_name\" ON \"items_copy\"(name)"
        );
    }
}
