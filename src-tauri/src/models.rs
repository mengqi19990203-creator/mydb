use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_driver() -> String {
    "sqlite".to_string()
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_ssl_mode() -> String {
    "disabled".to_string()
}

fn default_color() -> String {
    "#58a6ff".to_string()
}

fn default_ssh_port() -> u16 {
    22
}

fn default_ssh_auth() -> String {
    "key".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub group: String,
    pub color: String,
    // SQLite
    pub file: String,
    // Server databases
    pub host: String,
    pub port: u16,
    pub username: String,
    pub database: String,
    pub ssl_mode: String,
    // SSH tunnel
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_auth: String,
    pub ssh_key_path: String,
    // Password persistence flags
    pub save_password: bool,
    pub save_ssh_password: bool,
    /// Read-only connections reject every write at the command layer.
    #[serde(default)]
    pub read_only: bool,
    // Derived, never persisted
    #[serde(skip)]
    pub has_password: bool,
    #[serde(skip)]
    pub has_ssh_password: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionGroup {
    pub name: String,
    pub color: String,
    pub order: i64,
    pub collapsed: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            driver: default_driver(),
            group: String::new(),
            color: default_color(),
            file: String::new(),
            host: default_host(),
            port: 0,
            username: String::new(),
            database: String::new(),
            ssl_mode: default_ssl_mode(),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: default_ssh_port(),
            ssh_username: String::new(),
            ssh_auth: default_ssh_auth(),
            ssh_key_path: String::new(),
            save_password: false,
            save_ssh_password: false,
            read_only: false,
            has_password: false,
            has_ssh_password: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseObjects {
    pub database: String,
    pub objects: Vec<TableInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub rows_affected: u64,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub is_query: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeTableColumns {
    pub name: String,
    pub kind: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: String,
    pub on_update: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub ddl: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErColumn {
    pub name: String,
    pub data_type: String,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErTable {
    pub name: String,
    pub kind: String,
    pub columns: Vec<ErColumn>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErRelation {
    pub name: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErDiagram {
    pub tables: Vec<ErTable>,
    pub relations: Vec<ErRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnValue {
    pub column: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRows {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Value>>,
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
}
