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
    /// MongoDB authentication database (authSource). Empty uses `admin`.
    #[serde(default)]
    pub mongo_auth_source: String,
    /// MongoDB connection-string mode; the full URI lives in the keychain.
    #[serde(default)]
    pub mongo_uri_mode: bool,
    /// Masked MongoDB URI for display. Never contains a password.
    #[serde(default)]
    pub mongo_uri: String,
    pub ssl_mode: String,
    #[serde(default)]
    pub ssl_ca_path: String,
    #[serde(default)]
    pub ssl_client_cert_path: String,
    #[serde(default)]
    pub ssl_client_key_path: String,
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
    /// SSH private-key passphrase persistence flag (secret stays in keychain).
    #[serde(default)]
    pub save_ssh_key_passphrase: bool,
    /// Read-only connections reject every write at the command layer.
    #[serde(default)]
    pub read_only: bool,
    // Derived, never persisted
    #[serde(skip)]
    pub has_password: bool,
    #[serde(skip)]
    pub has_mongo_uri_secret: bool,
    #[serde(skip)]
    pub has_ssh_password: bool,
    #[serde(skip)]
    pub has_ssh_key_passphrase: bool,
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
            mongo_auth_source: String::new(),
            mongo_uri_mode: false,
            mongo_uri: String::new(),
            ssl_mode: default_ssl_mode(),
            ssl_ca_path: String::new(),
            ssl_client_cert_path: String::new(),
            ssl_client_key_path: String::new(),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: default_ssh_port(),
            ssh_username: String::new(),
            ssh_auth: default_ssh_auth(),
            ssh_key_path: String::new(),
            save_password: false,
            save_ssh_password: false,
            save_ssh_key_passphrase: false,
            read_only: false,
            has_password: false,
            has_mongo_uri_secret: false,
            has_ssh_password: false,
            has_ssh_key_passphrase: false,
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
pub struct MongoCollectionInfo {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDocumentPage {
    /// Documents encoded as MongoDB extended JSON (ObjectId, dates, binary...).
    pub documents: Vec<Value>,
    pub total: u64,
    pub skip: u64,
    pub limit: u64,
    pub elapsed_ms: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoConsoleOutcome {
    pub index: usize,
    pub statement: String,
    /// "documents" | "document" | "scalar" | "values" | "write" | "empty"
    pub kind: String,
    pub database: String,
    pub collection: Option<String>,
    pub documents: Vec<Value>,
    pub values: Vec<Value>,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub error: Option<String>,
    pub message: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoConsoleConfirmation {
    pub index: usize,
    pub database: String,
    pub collection: String,
    pub method: String,
    pub reason: String,
    pub estimated_count: Option<u64>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoConsoleResponse {
    pub outcomes: Vec<MongoConsoleOutcome>,
    pub confirmations: Vec<MongoConsoleConfirmation>,
    pub database: String,
    /// True when a write may have created / changed a collection.
    pub refresh_collections: bool,
    pub error: Option<String>,
    pub error_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoIndexInfo {
    pub name: String,
    pub keys: Value,
    pub unique: bool,
    pub sparse: bool,
    pub expire_after_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoUserInfo {
    pub username: String,
    pub database: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoRoleInfo {
    pub name: String,
    pub database: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoGridFsFile {
    pub id: Value,
    pub filename: String,
    pub length: u64,
    pub chunk_size: u32,
    pub upload_date: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoIndexSize {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoTimeseriesInfo {
    pub time_field: Option<String>,
    pub meta_field: Option<String>,
    pub granularity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoCollectionStats {
    pub namespace: String,
    pub kind: String,
    pub capped: bool,
    pub count: Option<u64>,
    pub size: Option<u64>,
    pub storage_size: Option<u64>,
    pub avg_obj_size: Option<f64>,
    pub nindexes: Option<u32>,
    pub total_index_size: Option<u64>,
    pub index_sizes: Vec<MongoIndexSize>,
    pub timeseries: Option<MongoTimeseriesInfo>,
    pub validator: Option<Value>,
    pub read_only: Option<bool>,
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
pub struct ExplainResult {
    pub driver: String,
    pub statement: String,
    pub explain_sql: String,
    pub format: String,
    pub result: QueryResult,
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
    #[serde(default)]
    pub comment: Option<String>,
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
pub struct ScopeForeignKey {
    pub table: String,
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportObject {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DatabaseExportOptions {
    pub include_structure: bool,
    pub include_data: bool,
    pub include_drops: bool,
    pub create_database: bool,
    pub remove_definer: bool,
    pub rows_per_insert: u32,
    pub use_transaction: bool,
}

impl Default for DatabaseExportOptions {
    fn default() -> Self {
        Self {
            include_structure: true,
            include_data: true,
            include_drops: false,
            create_database: false,
            remove_definer: true,
            rows_per_insert: 100,
            use_transaction: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseExportProgress {
    pub operation_id: Option<String>,
    pub phase: String,
    pub object_name: String,
    pub object_index: usize,
    pub object_total: usize,
    pub rows: u64,
    pub bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseExportFailure {
    pub kind: String,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseExportSummary {
    pub path: String,
    pub object_count: usize,
    pub table_count: usize,
    pub rows: u64,
    pub bytes: u64,
    pub failures: Vec<DatabaseExportFailure>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub detected_delimiter: String,
    pub format: String,
    /// Worksheet names for Excel files; empty for CSV / JSON.
    #[serde(default)]
    pub sheets: Vec<String>,
    /// Worksheet used for this preview, when the format is Excel.
    #[serde(default)]
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMapping {
    pub source: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportOptions {
    pub delimiter: Option<String>,
    pub has_header: bool,
    /// `rollback` / `stop` / `skip`.
    pub error_mode: String,
    pub empty_as_null: bool,
    /// Selected Excel worksheet; ignored for CSV / JSON.
    #[serde(default)]
    pub sheet: Option<String>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            delimiter: None,
            has_header: true,
            error_mode: "rollback".to_string(),
            empty_as_null: true,
            sheet: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub inserted: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub rolled_back: bool,
    pub failures: Vec<ImportFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableExportProgress {
    pub operation_id: Option<String>,
    pub rows: u64,
    pub bytes: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub operation_id: Option<String>,
    pub processed: usize,
    pub inserted: usize,
    pub failed: usize,
    pub total: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDocsSnapshot {
    pub tables: Vec<DocsTable>,
    pub failures: Vec<DocsFailure>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocsTable {
    pub name: String,
    pub kind: String,
    pub structure: TableStructure,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocsFailure {
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneratorColumn {
    pub name: String,
    pub generator: String,
    pub enabled: bool,
    pub null_percent: u8,
    pub sequence_start: i64,
    pub sequence_step: i64,
    pub min_number: f64,
    pub max_number: f64,
    pub decimals: u8,
    pub text_min: usize,
    pub text_max: usize,
    /// `alpha` / `numeric` / `alphanumeric`.
    pub charset: String,
    pub regex: String,
    pub date_from: String,
    pub date_to: String,
    pub bool_percent: u8,
    pub fixed_value: String,
}

impl Default for GeneratorColumn {
    fn default() -> Self {
        Self {
            name: String::new(),
            generator: "integer".to_string(),
            enabled: true,
            null_percent: 0,
            sequence_start: 1,
            sequence_step: 1,
            min_number: 1.0,
            max_number: 1_000_000.0,
            decimals: 2,
            text_min: 4,
            text_max: 16,
            charset: "alphanumeric".to_string(),
            regex: String::new(),
            date_from: String::new(),
            date_to: String::new(),
            bool_percent: 50,
            fixed_value: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub table: String,
    pub columns: Vec<GeneratorColumn>,
    pub count: usize,
    pub seed: u64,
    pub batch_size: usize,
    #[serde(default)]
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSummary {
    pub inserted: usize,
    pub generated: usize,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateProgress {
    pub operation_id: Option<String>,
    pub phase: String,
    pub generated: usize,
    pub inserted: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupSchedule {
    /// `hourly` / `daily` / `weekly`.
    pub kind: String,
    pub interval_hours: u32,
    /// Local time, 0-23.
    pub hour: u32,
    pub minute: u32,
    /// 0 = Monday … 6 = Sunday (ISO weekday - 1).
    pub weekday: u32,
}

impl Default for BackupSchedule {
    fn default() -> Self {
        Self {
            kind: "daily".to_string(),
            interval_hours: 1,
            hour: 2,
            minute: 0,
            weekday: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupJob {
    pub id: String,
    pub name: String,
    pub connection_id: String,
    pub connection_name: String,
    pub connection_available: bool,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub include_structure: bool,
    pub include_data: bool,
    pub include_drops: bool,
    /// `all` / `include` / `exclude`.
    pub table_mode: String,
    pub table_pattern: String,
    pub output_dir: String,
    pub filename_template: String,
    pub enabled: bool,
    pub schedule: BackupSchedule,
    /// Number of successful backups to keep (1-30).
    pub retention: u32,
    /// Local ISO-8601 timestamp.
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    /// Set when the app was closed at the scheduled time. Not auto-run.
    pub missed: bool,
}

impl Default for BackupJob {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            connection_id: String::new(),
            connection_name: String::new(),
            connection_available: true,
            database: None,
            schema: None,
            include_structure: true,
            include_data: true,
            include_drops: false,
            table_mode: "all".to_string(),
            table_pattern: String::new(),
            output_dir: String::new(),
            filename_template: "mydb-backup-{connection}-{database}-{timestamp}.sql".to_string(),
            enabled: true,
            schedule: BackupSchedule::default(),
            retention: 5,
            next_run: None,
            last_run: None,
            last_status: None,
            last_error: None,
            missed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRun {
    pub id: String,
    pub job_id: String,
    pub job_name: String,
    pub connection_id: String,
    pub connection_name: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: String,
    pub path: String,
    pub bytes: u64,
    pub rows: u64,
    pub tables: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRunSummary {
    pub run: BackupRun,
    pub next_run: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectVariant {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelQueryResult {
    pub cancelled: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxExportSummary {
    pub path: String,
    pub rows: u64,
    pub sheet_name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptOptions {
    pub stop_on_error: bool,
    pub use_transaction: bool,
}

impl Default for ScriptOptions {
    fn default() -> Self {
        Self {
            stop_on_error: true,
            use_transaction: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFileStatementPreview {
    pub index: usize,
    pub line: usize,
    pub preview: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFileViolation {
    pub index: usize,
    pub line: usize,
    pub preview: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlFilePreview {
    pub path: String,
    pub bytes: u64,
    pub statement_count: usize,
    pub statements: Vec<SqlFileStatementPreview>,
    pub violation_count: usize,
    pub violations: Vec<SqlFileViolation>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlScriptFailure {
    pub index: usize,
    pub line: usize,
    pub preview: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlScriptSummary {
    pub path: String,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub rows_affected: u64,
    pub elapsed_ms: u64,
    pub rolled_back: bool,
    pub cancelled: bool,
    pub failures: Vec<SqlScriptFailure>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlScriptProgress {
    pub operation_id: Option<String>,
    pub index: usize,
    pub total: usize,
    pub line: usize,
    pub preview: String,
    pub status: String,
    pub rows_affected: u64,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSort {
    pub column: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingTableChange {
    /// `delete`, `update` or `insert`.
    pub kind: String,
    #[serde(default)]
    pub keys: Vec<ColumnValue>,
    #[serde(default)]
    pub changes: Vec<ColumnValue>,
    #[serde(default)]
    pub values: Vec<ColumnValue>,
    #[serde(default)]
    pub single_row: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableChangeResult {
    pub applied: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaApplyResult {
    pub applied: usize,
    pub total: usize,
    pub failed_at: Option<usize>,
    pub error: Option<String>,
    pub rolled_back: bool,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaApplyProgress {
    pub operation_id: Option<String>,
    pub index: usize,
    pub total: usize,
    pub preview: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetric {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerOverview {
    pub driver: String,
    pub version: String,
    pub current_user: String,
    pub current_database: String,
    pub metrics: Vec<ServerMetric>,
    pub sampled_at: String,
    pub max_connections: Option<u64>,
    pub connection_usage_percent: Option<f64>,
    pub database_size_bytes: Option<u64>,
    pub table_count: Option<u64>,
    pub qps: Option<f64>,
    pub cache_hit_percent: Option<f64>,
    pub active_sessions: Option<u64>,
    pub idle_sessions: Option<u64>,
    pub top_tables: Vec<ServerTopTable>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTopTable {
    pub name: String,
    pub total_bytes: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUser {
    pub name: String,
    pub host: String,
    pub attributes: Vec<ServerMetric>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUserDetail {
    pub name: String,
    pub host: String,
    pub grants: Vec<String>,
    pub memberships: Vec<String>,
    pub privileges: Vec<ServerMetric>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProcess {
    pub id: String,
    pub user: String,
    pub database: String,
    pub state: String,
    pub time_seconds: i64,
    pub info: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferTable {
    pub source_table: String,
    pub target_table: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TransferOptions {
    /// append | upsert | truncate
    pub mode: String,
    pub create_missing: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            mode: "append".to_string(),
            create_missing: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferFailure {
    pub table: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    pub tables: usize,
    pub rows: u64,
    pub failures: Vec<TransferFailure>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub operation_id: Option<String>,
    pub table: String,
    pub table_index: usize,
    pub table_total: usize,
    pub rows: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSearchHit {
    pub database: Option<String>,
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub column: Option<String>,
    pub owner: Option<String>,
}
