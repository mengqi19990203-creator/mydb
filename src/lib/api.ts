import { invoke } from "@tauri-apps/api/core";
import type {
  ColumnInfo,
  ColumnValue,
  ConnectionConfig,
  ConnectionContext,
  ConnectionGroupInfo,
  DatabaseObjects,
  ErDiagram,
  HistoryEntry,
  QueryResult,
  RedisCommandResult,
  RedisDatabases,
  RedisKeyDetail,
  RedisScanResult,
  SchemaDiff,
  ScopeTableColumns,
  TableDataDiff,
  TableInfo,
  TableRows,
  TableStructure,
} from "./types";

export function listConnections(): Promise<ConnectionConfig[]> {
  return invoke("list_connections");
}

export function saveConnection(
  config: ConnectionConfig,
  password?: string,
  sshPassword?: string,
): Promise<ConnectionConfig> {
  return invoke("save_connection", {
    config,
    password: password ?? null,
    sshPassword: sshPassword ?? null,
  });
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
): Promise<void> {
  return invoke("test_connection", {
    config,
    password: password ?? null,
    sshPassword: sshPassword ?? null,
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
): Promise<QueryResult> {
  return invoke("execute_query", { connectionId, sql, maxRows });
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
): Promise<QueryResult> {
  return invoke("execute_transaction_query", {
    connectionId,
    sessionId,
    sql,
    maxRows,
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
): Promise<TableRows> {
  return invoke("read_table_rows", {
    connectionId,
    table,
    limit,
    offset,
    filter: filter ?? null,
    orderBy: orderBy ?? null,
    orderDir: orderDir ?? null,
  });
}

export function updateTableRow(
  connectionId: string,
  table: string,
  keys: ColumnValue[],
  changes: ColumnValue[],
): Promise<number> {
  return invoke("update_table_row", { connectionId, table, keys, changes });
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
): Promise<number> {
  return invoke("delete_table_row", { connectionId, table, keys });
}

export function getObjectDdl(
  connectionId: string,
  kind: string,
  name: string,
): Promise<string> {
  return invoke("get_object_ddl", { connectionId, kind, name });
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
  });
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

export type ExportFormat = "csv" | "json" | "sql" | "ddl";

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
): Promise<RedisScanResult> {
  return invoke("redis_scan_keys", { connectionId, pattern, limit });
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

export function redisCommand(
  connectionId: string,
  command: string,
): Promise<RedisCommandResult> {
  return invoke("redis_command", { connectionId, command });
}
