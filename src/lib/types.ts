export type Driver = "sqlite" | "mysql" | "postgres" | "redis";
export type SslMode = "disabled" | "preferred" | "required";
export type SshAuth = "key" | "password";

export interface ConnectionConfig {
  id: string;
  name: string;
  driver: Driver;
  group: string;
  color: string;
  file: string;
  host: string;
  port: number;
  username: string;
  database: string;
  sslMode: SslMode;
  sshEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuth: SshAuth;
  sshKeyPath: string;
  savePassword: boolean;
  saveSshPassword: boolean;
  readOnly: boolean;
  hasPassword: boolean;
  hasSshPassword: boolean;
}

export interface ConnectionGroupInfo {
  name: string;
  color: string;
  order: number;
  collapsed: boolean;
}

export interface ConnectionContext {
  database: string | null;
  schema: string | null;
}

export interface TableInfo {
  name: string;
  kind: "table" | "view" | string;
}

export interface DatabaseObjects {
  database: string;
  objects: TableInfo[];
}

export interface QueryResult {
  columns: string[];
  rows: unknown[][];
  rowsAffected: number;
  elapsedMs: number;
  truncated: boolean;
  isQuery: boolean;
}

export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  autoIncrement: boolean;
  defaultValue: string | null;
}

export interface ScopeTableColumns {
  name: string;
  kind: string;
  columns: ColumnInfo[];
}

export interface IndexInfo {
  name: string;
  columns: string[];
  unique: boolean;
  primary: boolean;
}

export interface ForeignKeyInfo {
  name: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
  onDelete: string;
  onUpdate: string;
}

export interface TableStructure {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  ddl: string;
}

export interface ErColumn {
  name: string;
  dataType: string;
  primaryKey: boolean;
}

export interface ErTable {
  name: string;
  kind: string;
  columns: ErColumn[];
}

export interface ErRelation {
  name: string;
  fromTable: string;
  fromColumns: string[];
  toTable: string;
  toColumns: string[];
}

export interface ErDiagram {
  tables: ErTable[];
  relations: ErRelation[];
}

export interface ColumnChange {
  name: string;
  sourceType: string;
  targetType: string;
  details: string[];
  suggestedSql: string | null;
}

export interface SchemaTableDiff {
  name: string;
  status: "same" | "changed" | "onlySource" | "onlyTarget";
  sourceRows: number | null;
  targetRows: number | null;
  addedColumns: string[];
  missingColumns: string[];
  changedColumns: ColumnChange[];
  addedIndexes: string[];
  extraIndexes: string[];
  changedIndexes: string[];
  addedForeignKeys: string[];
  extraForeignKeys: string[];
  changedForeignKeys: string[];
  sql: string[];
  warnings: string[];
}

export interface SchemaDiff {
  sourceLabel: string;
  targetLabel: string;
  sameCount: number;
  changedCount: number;
  onlySourceCount: number;
  onlyTargetCount: number;
  tables: SchemaTableDiff[];
  sql: string;
}

export interface RowChange {
  status: "added" | "updated" | "removed";
  key: string;
  changedColumns: string[];
}

export interface TableDataDiff {
  sourceLabel: string;
  targetLabel: string;
  sourceTable: string;
  targetTable: string;
  keyColumns: string[];
  comparedColumns: string[];
  skippedColumns: string[];
  sourceOnlyColumns: string[];
  targetOnlyColumns: string[];
  insertCount: number;
  updateCount: number;
  deleteCount: number;
  sameCount: number;
  sourceTotal: number;
  targetTotal: number;
  truncated: boolean;
  changes: RowChange[];
  changesTruncated: boolean;
  statements: string[];
  sql: string;
  warnings: string[];
}

export interface ColumnValue {
  column: string;
  value: unknown;
}

export interface TableRows {
  columns: ColumnInfo[];
  rows: unknown[][];
  total: number;
  offset: number;
  limit: number;
}

export interface HistoryEntry {
  id: string;
  connectionId: string;
  connectionName: string;
  sql: string;
  executedAt: string;
  elapsedMs: number;
  success: boolean;
  error: string | null;
}

export interface RedisKeyEntry {
  key: string;
  keyType: string;
  ttl: number;
}

export interface RedisScanResult {
  keys: RedisKeyEntry[];
  scanned: number;
  truncated: boolean;
}

export interface RedisKeyDetail {
  key: string;
  keyType: string;
  ttl: number;
  value: string;
  truncated: boolean;
}

export interface RedisCommandResult {
  output: string;
  elapsedMs: number;
  truncated: boolean;
}

export interface RedisDatabaseInfo {
  index: number;
  keys: number | null;
}

export interface RedisDatabases {
  databases: RedisDatabaseInfo[];
  count: number;
  cluster: boolean;
}
