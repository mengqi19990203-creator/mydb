export type Driver = "sqlite" | "mysql" | "postgres" | "redis" | "mongodb";
export type SslMode =
  | "disabled"
  | "preferred"
  | "required"
  | "verify-ca"
  | "verify-full";
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
  mongoAuthSource?: string;
  sslMode: SslMode;
  sslCaPath: string;
  sslClientCertPath: string;
  sslClientKeyPath: string;
  sshEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuth: SshAuth;
  sshKeyPath: string;
  savePassword: boolean;
  saveSshPassword: boolean;
  saveSshKeyPassphrase: boolean;
  readOnly: boolean;
  hasPassword: boolean;
  hasSshPassword: boolean;
  hasSshKeyPassphrase: boolean;
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

export interface MongoCollectionInfo {
  name: string;
  kind: "collection" | "view" | "timeseries" | string;
}

export interface MongoDocumentPage {
  documents: Record<string, unknown>[];
  total: number;
  skip: number;
  limit: number;
  elapsedMs: number;
  truncated: boolean;
}

export interface MongoIndexInfo {
  name: string;
  keys: unknown;
  unique: boolean;
  sparse: boolean;
  expireAfterSeconds: number | null;
}


export type SqlCheckSeverity = "error" | "warning" | "info";

export interface SqlCheckIssue {
  severity: SqlCheckSeverity;
  code: string;
  message: string;
  /// Absolute character offset in the editor document.
  from: number;
  to: number;
  /// 1-based position for display.
  line: number;
  column: number;
}

export interface SqlCheckResult {
  issues: SqlCheckIssue[];
  scope: "statement" | "tab" | "selection";
  checkedFrom: number;
  checkedTo: number;
  objectCheck: "available" | "unavailable";
  tokenCount: number;
  truncated: boolean;
}

export interface QueryResult {
  columns: string[];
  rows: unknown[][];
  rowsAffected: number;
  elapsedMs: number;
  truncated: boolean;
  isQuery: boolean;
}

export interface CancelQueryResult {
  cancelled: boolean;
  message: string;
}

export interface ExplainResult {
  driver: Driver;
  statement: string;
  explainSql: string;
  format: "json" | "table" | "text";
  result: QueryResult;
}

export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  autoIncrement: boolean;
  defaultValue: string | null;
  comment?: string | null;
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

export interface ScopeForeignKey {
  table: string;
  name: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
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

export interface ImportPreview {
  columns: string[];
  rows: string[][];
  detectedDelimiter: string;
  format: string;
  /** Worksheet names for Excel imports; empty for CSV / JSON. */
  sheets?: string[];
  sheet?: string | null;
}

export interface ImportMapping {
  source: string;
  target: string | null;
}

export interface ImportOptions {
  delimiter: string | null;
  hasHeader: boolean;
  errorMode: "rollback" | "stop" | "skip" | string;
  emptyAsNull: boolean;
  /** Selected Excel worksheet; ignored for CSV / JSON. */
  sheet?: string | null;
}

export interface ImportFailure {
  row: number;
  message: string;
}

export interface ImportSummary {
  inserted: number;
  failed: number;
  skipped: number;
  elapsedMs: number;
  cancelled: boolean;
  rolledBack: boolean;
  failures: ImportFailure[];
}

export interface ImportProgress {
  operationId: string | null;
  processed: number;
  inserted: number;
  failed: number;
  total: number | null;
  message: string;
}

export interface DatabaseDocsSnapshot {
  tables: DocsTable[];
  failures: DocsFailure[];
  truncated: boolean;
}

export interface DocsTable {
  name: string;
  kind: string;
  structure: TableStructure;
}

export interface DocsFailure {
  name: string;
  error: string;
}

export interface ObjectVariant {
  id: string;
  label: string;
}

export interface ExportObject {
  kind: string;
  name: string;
}

export interface DatabaseExportOptions {
  includeStructure: boolean;
  includeData: boolean;
  includeDrops: boolean;
  createDatabase: boolean;
  removeDefiner: boolean;
  rowsPerInsert: number;
  useTransaction: boolean;
}

export interface DatabaseExportProgress {
  operationId: string | null;
  phase: string;
  objectName: string;
  objectIndex: number;
  objectTotal: number;
  rows: number;
  bytes: number;
  message: string;
}

export interface DatabaseExportFailure {
  kind: string;
  name: string;
  message: string;
}

export interface DatabaseExportSummary {
  path: string;
  objectCount: number;
  tableCount: number;
  rows: number;
  bytes: number;
  failures: DatabaseExportFailure[];
  warnings: string[];
}

export interface XlsxExportSummary {
  path: string;
  rows: number;
  sheetName: string;
  bytes: number;
}

export interface ScriptOptions {
  stopOnError: boolean;
  useTransaction: boolean;
}

export interface SqlFileStatementPreview {
  index: number;
  line: number;
  preview: string;
  kind: string;
}

export interface SqlFileViolation {
  index: number;
  line: number;
  preview: string;
  message: string;
}

export interface SqlFilePreview {
  path: string;
  bytes: number;
  statementCount: number;
  statements: SqlFileStatementPreview[];
  violationCount: number;
  violations: SqlFileViolation[];
  readOnly: boolean;
}

export interface SqlScriptFailure {
  index: number;
  line: number;
  preview: string;
  message: string;
}

export interface SqlScriptSummary {
  path: string;
  total: number;
  succeeded: number;
  failed: number;
  skipped: number;
  rowsAffected: number;
  elapsedMs: number;
  rolledBack: boolean;
  cancelled: boolean;
  failures: SqlScriptFailure[];
  notes: string[];
}

export interface SqlScriptProgress {
  operationId: string | null;
  index: number;
  total: number;
  line: number;
  preview: string;
  status: "running" | "success" | "failed";
  rowsAffected: number;
  elapsedMs: number;
  error: string | null;
}

export interface TableSort {
  column: string;
  direction: "asc" | "desc";
}

export interface PendingTableChange {
  kind: "delete" | "update" | "insert";
  keys: ColumnValue[];
  changes: ColumnValue[];
  values: ColumnValue[];
  singleRow: boolean;
}

export interface RowDetailField {
  name: string;
  value: unknown;
  type?: string;
  nullable?: boolean;
  primaryKey?: boolean;
  autoIncrement?: boolean;
  comment?: string | null;
  hidden?: boolean;
  modified?: boolean;
  editable?: boolean;
  disabledReason?: string;
  binary?: boolean;
}

export interface TableChangeResult {
  applied: number;
}

export interface SchemaApplyResult {
  applied: number;
  total: number;
  failedAt: number | null;
  error: string | null;
  rolledBack: boolean;
  cancelled: boolean;
  elapsedMs: number;
}

export interface SchemaApplyProgress {
  operationId: string | null;
  index: number;
  total: number;
  preview: string;
  status: "running" | "success" | "failed";
  error: string | null;
}

export interface BackupSchedule {
  kind: "hourly" | "daily" | "weekly";
  intervalHours: number;
  hour: number;
  minute: number;
  weekday: number;
}

export interface BackupJob {
  id: string;
  name: string;
  connectionId: string;
  connectionName: string;
  connectionAvailable: boolean;
  database: string | null;
  schema: string | null;
  includeStructure: boolean;
  includeData: boolean;
  includeDrops: boolean;
  tableMode: "all" | "include" | "exclude";
  tablePattern: string;
  outputDir: string;
  filenameTemplate: string;
  enabled: boolean;
  schedule: BackupSchedule;
  retention: number;
  nextRun: string | null;
  lastRun: string | null;
  lastStatus: string | null;
  lastError: string | null;
  missed: boolean;
}

export interface BackupRun {
  id: string;
  jobId: string;
  jobName: string;
  connectionId: string;
  connectionName: string;
  startedAt: string;
  finishedAt: string;
  status: "success" | "failed" | "cancelled" | "running" | string;
  path: string;
  bytes: number;
  rows: number;
  tables: number;
  error: string | null;
}

export interface BackupRunSummary {
  run: BackupRun;
  nextRun: string | null;
}

export interface GeneratorColumn {
  name: string;
  generator: string;
  enabled: boolean;
  nullPercent: number;
  sequenceStart: number;
  sequenceStep: number;
  minNumber: number;
  maxNumber: number;
  decimals: number;
  textMin: number;
  textMax: number;
  charset: "alpha" | "numeric" | "alphanumeric" | string;
  regex: string;
  dateFrom: string;
  dateTo: string;
  boolPercent: number;
  fixedValue: string;
}

export interface GenerateRequest {
  table: string;
  columns: GeneratorColumn[];
  count: number;
  seed: number;
  batchSize: number;
  operationId?: string | null;
}

export interface GeneratePreview {
  columns: string[];
  rows: unknown[][];
  warnings: string[];
}

export interface GenerateSummary {
  inserted: number;
  generated: number;
  elapsedMs: number;
  cancelled: boolean;
}

export interface GenerateProgress {
  operationId: string | null;
  phase: string;
  generated: number;
  inserted: number;
  total: number;
  message: string;
}

export interface ServerMetric {
  label: string;
  value: string;
}

export interface ServerOverview {
  driver: string;
  version: string;
  currentUser: string;
  currentDatabase: string;
  metrics: ServerMetric[];
  sampledAt: string;
  maxConnections: number | null;
  connectionUsagePercent: number | null;
  databaseSizeBytes: number | null;
  tableCount: number | null;
  qps: number | null;
  cacheHitPercent: number | null;
  activeSessions: number | null;
  idleSessions: number | null;
  topTables: ServerTopTable[];
}

export interface ServerTopTable {
  name: string;
  totalBytes: number;
  dataBytes: number;
  indexBytes: number;
}

export interface ServerUser {
  name: string;
  host: string;
  attributes: ServerMetric[];
}

export interface ServerUserDetail {
  name: string;
  host: string;
  grants: string[];
  memberships: string[];
  privileges: ServerMetric[];
  notes: string[];
}

export interface ServerProcess {
  id: string;
  user: string;
  database: string;
  state: string;
  timeSeconds: number;
  info: string;
}

export interface ScopeSearchHit {
  database: string | null;
  schema: string | null;
  name: string;
  kind: string;
  column: string | null;
  owner: string | null;
}

export interface TransferTable {
  sourceTable: string;
  targetTable: string;
}

export interface TransferOptions {
  mode: "append" | "upsert" | "truncate";
  createMissing: boolean;
}

export interface TransferFailure {
  table: string;
  message: string;
}

export interface TransferResult {
  tables: number;
  rows: number;
  failures: TransferFailure[];
  cancelled: boolean;
}

export interface TransferProgress {
  operationId: string | null;
  table: string;
  tableIndex: number;
  tableTotal: number;
  rows: number;
  message: string;
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
  cursor: number;
}

export interface RedisHashEntry {
  field: string;
  value: string;
}

export interface RedisListEntry {
  index: number;
  value: string;
}

export interface RedisZsetEntry {
  member: string;
  score: number;
}

export interface RedisKeyDetail {
  key: string;
  keyType: string;
  ttl: number;
  value: string;
  truncated: boolean;
  hash: RedisHashEntry[];
  listEntries: RedisListEntry[];
  setMembers: string[];
  zsetEntries: RedisZsetEntry[];
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
