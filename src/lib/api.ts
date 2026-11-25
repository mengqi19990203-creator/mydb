import { invoke } from "@tauri-apps/api/core";
import type {
  BackupJob,
  BackupRun,
  BackupRunSummary,
  CancelQueryResult,
  ColumnInfo,
  ColumnValue,
  ConnectionConfig,
  ConnectionContext,
  ConnectionGroupInfo,
  DatabaseExportOptions,
  DatabaseDocsSnapshot,
  DatabaseExportSummary,
  DatabaseObjects,
  ErDiagram,
  ExplainResult,
  ExportObject,
  GeneratePreview,
  GenerateRequest,
  GenerateSummary,
  HistoryEntry,
  ImportMapping,
  ImportOptions,
  ImportPreview,
  ImportSummary,
  ObjectVariant,
  PendingTableChange,
  QueryResult,
  RedisCommandResult,
  RedisDatabases,
  RedisKeyDetail,
  RedisScanResult,
  SchemaDiff,
  SchemaApplyResult,
  ScopeSearchHit,
  ScopeTableColumns,
  ScriptOptions,
  ServerOverview,
  ServerProcess,
  ServerUser,
  ServerUserDetail,
  SqlFilePreview,
  SqlScriptSummary,
  TableChangeResult,
  TableDataDiff,
  TableInfo,
  TableRows,
  TableSort,
  TableStructure,
  TransferOptions,
  TransferResult,
  TransferTable,
  XlsxExportSummary,
} from "./types";

export function listConnections(): Promise<ConnectionConfig[]> {
  return invoke("list_connections");
}

export function saveConnection(
  config: ConnectionConfig,
  password?: string,
  sshPassword?: string,
  sshKeyPassphrase?: string,
): Promise<ConnectionConfig> {
  return invoke("save_connection", {
    config,
    password: password ?? null,
    sshPassword: sshPassword ?? null,
    sshKeyPassphrase: sshKeyPassphrase ?? null,
  });
}

export function clearConnectionSecret(
  id: string,
  kind: "db" | "ssh" | "ssh-key",
): Promise<void> {
  return invoke("clear_connection_secret", { id, kind });
}

export function deleteConnection(id: string): Promise<void> {
  return invoke("delete_connection", { id });
}

export function closeConnection(id: string): Promise<void> {
  return invoke("close_connection", { id });
}

export function duplicateConnection(
  id: string,
  newName: string | null,
  copySecrets: boolean,
): Promise<ConnectionConfig> {
  return invoke("duplicate_connection", { id, newName, copySecrets });
}

export type ConnectionInfoFormat =
  | "hostPort"
  | "url"
  | "urlWithPassword"
  | "jdbc"
  | "dsn"
  | "cli";

export function connectionInfo(
  id: string,
  format: ConnectionInfoFormat,
  includePassword: boolean,
): Promise<string> {
  return invoke("connection_info", { id, format, includePassword });
}

export function setConnectionReadOnly(
  id: string,
  readOnly: boolean,
): Promise<ConnectionConfig> {
  return invoke("set_connection_read_only", { id, readOnly });
}

export function listConnectionGroups(): Promise<ConnectionGroupInfo[]> {
  return invoke("list_connection_groups");
}

export function saveConnectionGroup(
  group: ConnectionGroupInfo,
): Promise<ConnectionGroupInfo[]> {
  return invoke("save_connection_group", { group });
}

export function renameConnectionGroup(
  oldName: string,
  newName: string,
): Promise<ConnectionGroupInfo[]> {
  return invoke("rename_connection_group", { oldName, newName });
}

export function deleteConnectionGroup(
  name: string,
): Promise<ConnectionGroupInfo[]> {
  return invoke("delete_connection_group", { name });
}

export function moveConnectionToGroup(
  id: string,
  group: string | null,
): Promise<void> {
  return invoke("move_connection_to_group", { id, group });
}

export function testConnection(
  config: ConnectionConfig,
  password?: string,
  sshPassword?: string,
  sshKeyPassphrase?: string,
): Promise<void> {
  return invoke("test_connection", {
    config,
    password: password ?? null,
    sshPassword: sshPassword ?? null,
    sshKeyPassphrase: sshKeyPassphrase ?? null,
  });
}

export function listTables(connectionId: string): Promise<TableInfo[]> {
  return invoke("list_tables", { connectionId });
}

export function listScopeColumns(
  connectionId: string,
): Promise<ScopeTableColumns[]> {
  return invoke("list_scope_columns", { connectionId });
}

export function listDatabases(connectionId: string): Promise<string[]> {
  return invoke("list_databases", { connectionId });
}

export function createDatabase(
  connectionId: string,
  name: string,
  charset?: string | null,
  collation?: string | null,
): Promise<void> {
  return invoke("create_database", {
    connectionId,
    name,
    charset: charset ?? null,
    collation: collation ?? null,
  });
}

export function createSchema(
  connectionId: string,
  name: string,
): Promise<void> {
  return invoke("create_schema", { connectionId, name });
}

export function dropDatabase(
  connectionId: string,
  name: string,
): Promise<void> {
  return invoke("drop_database", { connectionId, name });
}

export function dropSchema(
  connectionId: string,
  name: string,
): Promise<void> {
  return invoke("drop_schema", { connectionId, name });
}

export function listSchemas(connectionId: string): Promise<string[]> {
  return invoke("list_schemas", { connectionId });
}

export function getConnectionContext(
  connectionId: string,
): Promise<ConnectionContext> {
  return invoke("get_connection_context", { connectionId });
}

export function listContextTables(
  connectionId: string,
  database: string | null,
  schema: string | null,
): Promise<TableInfo[]> {
  return invoke("list_context_tables", {
    connectionId,
    database,
    schema,
  });
}

export function listContextSchemas(
  connectionId: string,
  database: string | null,
): Promise<string[]> {
  return invoke("list_context_schemas", { connectionId, database });
}

export function listDatabaseObjects(
  connectionId: string,
): Promise<DatabaseObjects[]> {
  return invoke("list_database_objects", { connectionId });
}

export function switchSchema(
  connectionId: string,
  schema: string,
): Promise<void> {
  return invoke("switch_schema", { connectionId, schema });
}

export function switchDatabase(
  connectionId: string,
  database: string,
): Promise<void> {
  return invoke("switch_database", { connectionId, database });
}

export function executeQuery(
  connectionId: string,
  sql: string,
  maxRows = 1000,
  operationId?: string,
): Promise<QueryResult> {
  return invoke("execute_query", {
    connectionId,
    sql,
    maxRows,
    operationId: operationId ?? null,
  });
}

export function cancelQuery(operationId: string): Promise<CancelQueryResult> {
  return invoke("cancel_query", { operationId });
}

export function explainQuery(
  connectionId: string,
  sql: string,
): Promise<ExplainResult> {
  return invoke("explain_query", { connectionId, sql });
}

export function searchScopeObjects(
  connectionId: string,
  query: string,
): Promise<ScopeSearchHit[]> {
  return invoke("search_scope_objects", { connectionId, query });
}

export function beginTransaction(
  connectionId: string,
  sessionId: string,
): Promise<void> {
  return invoke("begin_transaction", { connectionId, sessionId });
}

export function executeTransactionQuery(
  connectionId: string,
  sessionId: string,
  sql: string,
  maxRows = 1000,
  operationId?: string,
): Promise<QueryResult> {
  return invoke("execute_transaction_query", {
    connectionId,
    sessionId,
    sql,
    maxRows,
    operationId: operationId ?? null,
  });
}

export function commitTransaction(
  connectionId: string,
  sessionId: string,
): Promise<void> {
  return invoke("commit_transaction", { connectionId, sessionId });
}

export function rollbackTransaction(
  connectionId: string,
  sessionId: string,
): Promise<void> {
  return invoke("rollback_transaction", { connectionId, sessionId });
}

export function closeTransactionSession(sessionId: string): Promise<void> {
  return invoke("close_transaction_session", { sessionId });
}

export function listColumns(
  connectionId: string,
  table: string,
): Promise<ColumnInfo[]> {
  return invoke("list_columns", { connectionId, table });
}

export function getTableStructure(
  connectionId: string,
  table: string,
): Promise<TableStructure> {
  return invoke("get_table_structure", { connectionId, table });
}

export function readTableRows(
  connectionId: string,
  table: string,
  limit = 100,
  offset = 0,
  filter?: string,
  orderBy?: string,
  orderDir?: "asc" | "desc",
  sorts?: TableSort[],
): Promise<TableRows> {
  return invoke("read_table_rows", {
    connectionId,
    table,
    limit,
    offset,
    filter: filter ?? null,
    orderBy: orderBy ?? null,
    orderDir: orderDir ?? null,
    sorts: sorts?.length ? sorts : null,
  });
}

export function applyTableChanges(
  connectionId: string,
  table: string,
  changes: PendingTableChange[],
): Promise<TableChangeResult> {
  return invoke("apply_table_changes", {
    connectionId,
    table,
    changes,
  });
}

export function previewGeneratedData(
  connectionId: string,
  request: GenerateRequest,
): Promise<GeneratePreview> {
  return invoke("preview_generated_data", { connectionId, request });
}

export function generateTableData(
  connectionId: string,
  request: GenerateRequest,
): Promise<GenerateSummary> {
  return invoke("generate_table_data", { connectionId, request });
}

export function cancelGenerate(operationId: string): Promise<void> {
  return invoke("cancel_generate", { operationId });
}

export function updateTableRow(
  connectionId: string,
  table: string,
  keys: ColumnValue[],
  changes: ColumnValue[],
  singleRow = false,
): Promise<number> {
  return invoke("update_table_row", {
    connectionId,
    table,
    keys,
    changes,
    singleRow,
  });
}

export function insertTableRow(
  connectionId: string,
  table: string,
  values: ColumnValue[],
): Promise<number> {
  return invoke("insert_table_row", { connectionId, table, values });
}

export function deleteTableRow(
  connectionId: string,
  table: string,
  keys: ColumnValue[],
  singleRow = false,
): Promise<number> {
  return invoke("delete_table_row", {
    connectionId,
    table,
    keys,
    singleRow,
  });
}

export function getObjectDdl(
  connectionId: string,
  kind: string,
  name: string,
  identity?: string | null,
): Promise<string> {
  return invoke("get_object_ddl", {
    connectionId,
    kind,
    name,
    identity: identity ?? null,
  });
}

export function listObjectVariants(
  connectionId: string,
  kind: string,
  name: string,
): Promise<ObjectVariant[]> {
  return invoke("list_object_variants", { connectionId, kind, name });
}

export function saveObjectDefinition(
  connectionId: string,
  kind: string,
  name: string,
  definition: string,
): Promise<void> {
  return invoke("save_object_definition", {
    connectionId,
    kind,
    name,
    definition,
  });
}

export function getIndexTable(
  connectionId: string,
  indexName: string,
): Promise<string> {
  return invoke("get_index_table", { connectionId, indexName });
}

export function getErDiagram(connectionId: string): Promise<ErDiagram> {
  return invoke("get_er_diagram", { connectionId });
}

export function databaseDocsSnapshot(
  connectionId: string,
): Promise<DatabaseDocsSnapshot> {
  return invoke("database_docs_snapshot", { connectionId });
}

export function compareSchemas(
  sourceConnectionId: string,
  targetConnectionId: string,
  sourceDatabase: string | null,
  sourceSchema: string | null,
  targetDatabase: string | null,
  targetSchema: string | null,
  sourceTable: string | null,
  targetTable: string | null,
  includeIndexes: boolean,
  includeForeignKeys: boolean,
  includeDrops: boolean,
): Promise<SchemaDiff> {
  return invoke("compare_schemas", {
    sourceConnectionId,
    targetConnectionId,
    sourceDatabase,
    sourceSchema,
    targetDatabase,
    targetSchema,
    sourceTable,
    targetTable,
    includeIndexes,
    includeForeignKeys,
    includeDrops,
  });
}

export function alterTableColumn(
  connectionId: string,
  table: string,
  column: string,
  name: string,
  dataType: string,
  nullable: boolean,
  defaultValue: string | null,
): Promise<void> {
  return invoke("alter_table_column", {
    connectionId,
    table,
    column,
    name,
    dataType,
    nullable,
    defaultValue,
  });
}

export function serverOverview(connectionId: string): Promise<ServerOverview> {
  return invoke("server_overview", { connectionId });
}

export function transferTableData(
  sourceConnectionId: string,
  targetConnectionId: string,
  sourceDatabase: string | null,
  sourceSchema: string | null,
  targetDatabase: string | null,
  targetSchema: string | null,
  tables: TransferTable[],
  options: TransferOptions,
  operationId: string,
): Promise<TransferResult> {
  return invoke("transfer_table_data", {
    sourceConnectionId,
    targetConnectionId,
    sourceDatabase,
    sourceSchema,
    targetDatabase,
    targetSchema,
    tables,
    options,
    operationId,
  });
}

export function cancelDataTransfer(operationId: string): Promise<void> {
  return invoke("cancel_data_transfer", { operationId });
}

export function serverProcesses(connectionId: string): Promise<ServerProcess[]> {
  return invoke("server_processes", { connectionId });
}

export function serverUsers(connectionId: string): Promise<ServerUser[]> {
  return invoke("server_users", { connectionId });
}

export function serverUserGrants(
  connectionId: string,
  name: string,
  host: string,
): Promise<ServerUserDetail> {
  return invoke("server_user_grants", { connectionId, name, host });
}

export function terminateServerProcess(
  connectionId: string,
  processId: string,
): Promise<void> {
  return invoke("terminate_server_process", { connectionId, processId });
}

export function applySchemaSync(
  targetConnectionId: string,
  targetDatabase: string | null,
  targetSchema: string | null,
  statements: string[],
  operationId: string,
): Promise<SchemaApplyResult> {
  return invoke("apply_schema_sync", {
    targetConnectionId,
    targetDatabase,
    targetSchema,
    statements,
    operationId,
  });
}

export function cancelSchemaSync(operationId: string): Promise<void> {
  return invoke("cancel_schema_sync", { operationId });
}

export function compareTableData(
  sourceConnectionId: string,
  targetConnectionId: string,
  sourceTable: string,
  targetTable: string,
  sourceDatabase: string | null,
  sourceSchema: string | null,
  targetDatabase: string | null,
  targetSchema: string | null,
  maxRows: number,
): Promise<TableDataDiff> {
  return invoke("compare_table_data", {
    sourceConnectionId,
    targetConnectionId,
    sourceTable,
    targetTable,
    sourceDatabase,
    sourceSchema,
    targetDatabase,
    targetSchema,
    maxRows,
  });
}

export function applyTableDataSync(
  targetConnectionId: string,
  targetDatabase: string | null,
  targetSchema: string | null,
  statements: string[],
): Promise<number> {
  return invoke("apply_table_data_sync", {
    targetConnectionId,
    targetDatabase,
    targetSchema,
    statements,
  });
}

export type ExportFormat = "csv" | "json" | "sql" | "ddl" | "xlsx";

export function exportQueryResultXlsx(
  columns: string[],
  rows: unknown[][],
  path: string,
  sheetName: string,
  withHeader: boolean,
): Promise<XlsxExportSummary> {
  return invoke("export_query_result_xlsx", {
    columns,
    rows,
    path,
    sheetName,
    withHeader,
  });
}

export function exportObject(
  connectionId: string,
  kind: string,
  name: string,
  format: ExportFormat,
  path: string,
  limit: number | null,
  withHeader: boolean,
  operationId: string,
): Promise<number> {
  return invoke("export_object", {
    connectionId,
    kind,
    name,
    format,
    path,
    limit,
    withHeader,
    operationId,
  });
}

export function cancelExport(operationId: string): Promise<void> {
  return invoke("cancel_export", { operationId });
}

export function exportDatabaseSql(
  connectionId: string,
  path: string,
  objects: ExportObject[],
  options: DatabaseExportOptions,
  operationId: string,
): Promise<DatabaseExportSummary> {
  return invoke("export_database_sql", {
    connectionId,
    path,
    objects,
    options,
    operationId,
  });
}

export function previewSqlFile(
  connectionId: string,
  path: string,
): Promise<SqlFilePreview> {
  return invoke("preview_sql_file", { connectionId, path });
}

export function listBackupJobs(): Promise<BackupJob[]> {
  return invoke("list_backup_jobs");
}

export function saveBackupJob(job: BackupJob): Promise<BackupJob> {
  return invoke("save_backup_job", { job });
}

export function deleteBackupJob(id: string): Promise<void> {
  return invoke("delete_backup_job", { id });
}

export function runBackupJob(
  id: string,
  operationId: string,
): Promise<BackupRunSummary> {
  return invoke("run_backup_job", { id, operationId });
}

export function cancelBackup(operationId: string): Promise<void> {
  return invoke("cancel_backup", { operationId });
}

export function listBackupRuns(): Promise<BackupRun[]> {
  return invoke("list_backup_runs");
}

export function skipMissedBackups(): Promise<BackupJob[]> {
  return invoke("skip_missed_backups");
}

export function executeSqlFile(
  connectionId: string,
  path: string,
  options: ScriptOptions,
  operationId: string,
): Promise<SqlScriptSummary> {
  return invoke("execute_sql_file", {
    connectionId,
    path,
    options,
    operationId,
  });
}

export function cancelSqlScript(operationId: string): Promise<void> {
  return invoke("cancel_sql_script", { operationId });
}

export function maintainTable(
  connectionId: string,
  table: string,
  action: "empty" | "truncate",
): Promise<number> {
  return invoke("table_maintenance", { connectionId, table, action });
}

export function duplicateObject(
  connectionId: string,
  kind: string,
  source: string,
  target: string,
): Promise<void> {
  return invoke("duplicate_object", {
    connectionId,
    kind,
    source,
    target,
  });
}

export function snapshotViewToTable(
  connectionId: string,
  source: string,
  target: string,
  withData: boolean,
): Promise<void> {
  return invoke("snapshot_view_to_table", {
    connectionId,
    source,
    target,
    withData,
  });
}

export function renameObject(
  connectionId: string,
  kind: string,
  name: string,
  newName: string,
): Promise<void> {
  return invoke("rename_object", { connectionId, kind, name, newName });
}

export function dropObject(
  connectionId: string,
  kind: string,
  name: string,
): Promise<void> {
  return invoke("drop_object", { connectionId, kind, name });
}

export function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke("write_text_file", { path, contents });
}

export function readTextFile(path: string): Promise<string> {
  return invoke("read_text_file", { path });
}

export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

export interface QueryWindowPayload {
  title?: string;
  connectionId?: string | null;
  database?: string | null;
  schema?: string | null;
  sql?: string;
}

export function openQueryWindow(
  payload: QueryWindowPayload,
): Promise<string> {
  return invoke("open_query_window", {
    title: payload.title ?? null,
    connectionId: payload.connectionId ?? null,
    database: payload.database ?? null,
    schema: payload.schema ?? null,
    sql: payload.sql ?? null,
  });
}

export function connectionHasQueryWindow(
  connectionId: string,
): Promise<boolean> {
  return invoke("connection_has_query_window", { connectionId });
}

export interface NativeMenuState {
  enabled: Record<string, boolean>;
  checked: Record<string, boolean>;
}

export function syncMenuState(state: NativeMenuState): Promise<void> {
  return invoke("sync_menu_state", { state });
}

export function previewImportFile(
  path: string,
  format: string,
  options: ImportOptions,
): Promise<ImportPreview> {
  return invoke("preview_import_file", { path, format, options });
}

export function executeImportFile(
  connectionId: string,
  table: string,
  path: string,
  format: string,
  options: ImportOptions,
  mappings: ImportMapping[],
  operationId: string,
): Promise<ImportSummary> {
  return invoke("execute_import_file", {
    connectionId,
    table,
    path,
    format,
    options,
    mappings,
    operationId,
  });
}

export function cancelImport(operationId: string): Promise<void> {
  return invoke("cancel_import", { operationId });
}

export function importCsv(
  connectionId: string,
  table: string,
  path: string,
  hasHeader = true,
): Promise<number> {
  return invoke("import_csv", { connectionId, table, path, hasHeader });
}

export function importJson(
  connectionId: string,
  table: string,
  path: string,
): Promise<number> {
  return invoke("import_json", { connectionId, table, path });
}

export function listHistory(): Promise<HistoryEntry[]> {
  return invoke("list_history");
}

export function clearHistory(): Promise<void> {
  return invoke("clear_history");
}

export function redisPing(connectionId: string): Promise<string> {
  return invoke("redis_ping", { connectionId });
}

export function redisDatabases(
  connectionId: string,
): Promise<RedisDatabases> {
  return invoke("redis_databases", { connectionId });
}

export function redisSelectDatabase(
  connectionId: string,
  database: number,
): Promise<number> {
  return invoke("redis_select_database", { connectionId, database });
}

export function redisScanKeys(
  connectionId: string,
  pattern = "*",
  limit = 500,
  cursor = 0,
): Promise<RedisScanResult> {
  return invoke("redis_scan_keys", {
    connectionId,
    pattern,
    limit,
    cursor,
  });
}

export function redisKeyDetail(
  connectionId: string,
  key: string,
): Promise<RedisKeyDetail> {
  return invoke("redis_key_detail", { connectionId, key });
}

export function redisDeleteKey(
  connectionId: string,
  key: string,
): Promise<number> {
  return invoke("redis_delete_key", { connectionId, key });
}

export function redisSetTtl(
  connectionId: string,
  key: string,
  ttlSeconds: number | null,
): Promise<number> {
  return invoke("redis_set_ttl", { connectionId, key, ttlSeconds });
}

export function redisRenameKey(
  connectionId: string,
  key: string,
  newKey: string,
  overwrite: boolean,
): Promise<void> {
  return invoke("redis_rename_key", {
    connectionId,
    key,
    newKey,
    overwrite,
  });
}

export function redisSetString(
  connectionId: string,
  key: string,
  value: string,
): Promise<void> {
  return invoke("redis_set_string", { connectionId, key, value });
}

export function redisHashSet(
  connectionId: string,
  key: string,
  field: string,
  value: string,
): Promise<number> {
  return invoke("redis_hash_set", { connectionId, key, field, value });
}

export function redisHashDeleteField(
  connectionId: string,
  key: string,
  field: string,
): Promise<number> {
  return invoke("redis_hash_delete_field", {
    connectionId,
    key,
    field,
  });
}

export function redisListPush(
  connectionId: string,
  key: string,
  value: string,
  left: boolean,
): Promise<number> {
  return invoke("redis_list_push", { connectionId, key, value, left });
}

export function redisListSet(
  connectionId: string,
  key: string,
  index: number,
  value: string,
): Promise<void> {
  return invoke("redis_list_set", { connectionId, key, index, value });
}

export function redisListRemove(
  connectionId: string,
  key: string,
  value: string,
): Promise<number> {
  return invoke("redis_list_remove", { connectionId, key, value });
}

export function redisSetAdd(
  connectionId: string,
  key: string,
  member: string,
): Promise<number> {
  return invoke("redis_set_add", { connectionId, key, member });
}

export function redisSetRemove(
  connectionId: string,
  key: string,
  member: string,
): Promise<number> {
  return invoke("redis_set_remove", { connectionId, key, member });
}

export function redisZsetSet(
  connectionId: string,
  key: string,
  member: string,
  score: number,
): Promise<number> {
  return invoke("redis_zset_set", { connectionId, key, member, score });
}

export function redisZsetRemove(
  connectionId: string,
  key: string,
  member: string,
): Promise<number> {
  return invoke("redis_zset_remove", { connectionId, key, member });
}

export function redisCommand(
  connectionId: string,
  command: string,
): Promise<RedisCommandResult> {
  return invoke("redis_command", { connectionId, command });
}
