<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import AppMark from "./components/AppMark.vue";
import ConnectionDialog from "./components/ConnectionDialog.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import CreateTableDialog from "./components/CreateTableDialog.vue";
import DataSyncDialog from "./components/DataSyncDialog.vue";
import ErDiagram from "./components/ErDiagram.vue";
import HistoryPanel from "./components/HistoryPanel.vue";
import Icon from "./components/Icon.vue";
import ImportDataDialog from "./components/ImportDataDialog.vue";
import ObjectBrowser from "./components/ObjectBrowser.vue";
import PromptDialog from "./components/PromptDialog.vue";
import QueryWorkspace from "./components/QueryWorkspace.vue";
import RedisWorkspace from "./components/RedisWorkspace.vue";
import SchemaSyncDialog from "./components/SchemaSyncDialog.vue";
import StatusBar from "./components/StatusBar.vue";
import TableDataView from "./components/TableDataView.vue";
import TableStructureView from "./components/TableStructureView.vue";
import * as api from "./lib/api";
import type { ConnectionInfoFormat } from "./lib/api";
import { readClipboard, writeClipboard } from "./lib/clipboard";
import type {
  ConnectionConfig,
  ConnectionGroupInfo,
  Driver,
  HistoryEntry,
  QueryResult,
  TableInfo,
} from "./lib/types";

interface SavePayload {
  config: ConnectionConfig;
  password?: string;
  sshPassword?: string;
}

interface ConnectionGroup {
  name: string;
  connections: ConnectionConfig[];
  color?: string;
  order?: number;
}

type ConnectionStatus = "idle" | "connecting" | "connected" | "failed";

interface AppContextMenu {
  x: number;
  y: number;
  kind: "edit" | "app";
  target: HTMLElement | null;
  canCopy: boolean;
}

const TEXT_INPUT_TYPES = new Set([
  "text",
  "search",
  "url",
  "tel",
  "email",
  "password",
  "number",
  "date",
  "datetime-local",
  "month",
  "week",
  "time",
]);

const unlisteners: UnlistenFn[] = [];
const appContextMenu = ref<AppContextMenu | null>(null);
const appMenuRef = ref<HTMLElement | null>(null);

interface QueryTab {
  id: string;
  title: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  sql: string;
  /// Currently selected editor text; run uses it instead of the whole tab.
  selectedSql: string;
  result: QueryResult | null;
  error: string | null;
  running: boolean;
  inTransaction: boolean;
  transactionBusy: boolean;
}

const connections = ref<ConnectionConfig[]>([]);
const connectionSearch = ref("");
const connectionGroups = ref<ConnectionGroupInfo[]>([]);
const newConnectionGroup = ref("");
const connectionMenu = ref<{
  x: number;
  y: number;
  connection: ConnectionConfig;
} | null>(null);
const connectionMenuRef = ref<HTMLElement | null>(null);
const connectionMenuInfoOpen = ref(false);
const connectionMenuGroupOpen = ref(false);
const groupMenu = ref<{ x: number; y: number; group: string } | null>(null);
const groupMenuRef = ref<HTMLElement | null>(null);
const duplicateTarget = ref<ConnectionConfig | null>(null);
const duplicateOpen = ref(false);
const groupPromptOpen = ref(false);
const groupPromptMode = ref<"new" | "rename" | null>(null);
const groupPromptOld = ref("");
const groupPromptValue = ref("");
const tables = ref<TableInfo[]>([]);
const tablesLoading = ref(false);
const tablesConnectionId = ref<string | null>(null);
const databases = ref<string[]>([]);
const activeDatabase = ref<string | null>(null);
const databaseByConnection = reactive<Record<string, string>>({});
const schemas = ref<string[]>([]);
const activeSchema = ref<string | null>(null);
const scopeRevision = ref(0);
const schemaByConnection = reactive<Record<string, string>>({});
const databaseObjects = reactive<Record<string, TableInfo[]>>({});
const databaseCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {};
  for (const [database, objects] of Object.entries(databaseObjects)) {
    counts[database] = objects.length;
  }
  return counts;
});
let tablesRequest = 0;

interface TableTab {
  id: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  table: TableInfo;
  mode: "data" | "structure";
  focusedIndex: string | null;
}

interface ErTab {
  id: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
}

let tableSequence = 0;
let erSequence = 0;
const tableTabs = ref<TableTab[]>([]);
const erTabs = ref<ErTab[]>([]);
const tabStrip = ref<HTMLElement | null>(null);
const tabScrollLeft = ref(false);
const tabScrollRight = ref(false);
const syncOpen = ref(false);
const dataSyncOpen = ref(false);

let querySequence = 0;

function normalizeScopeValue(
  value: string | null | undefined,
): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function createQueryTab(
  connectionId: string | null,
  database: string | null = null,
  schema: string | null = null,
): QueryTab {
  querySequence += 1;
  return {
    id: `query-${Date.now()}-${querySequence}`,
    title: `查询 ${querySequence}`,
    connectionId,
    database: normalizeScopeValue(database),
    schema: normalizeScopeValue(schema),
    sql: "SELECT 1;",
    selectedSql: "",
    result: null,
    error: null,
    running: false,
    inTransaction: false,
    transactionBusy: false,
  };
}

const QUERY_TABS_STORAGE_KEY = "mydb.queryTabs.v1";
let tabsRestored = false;
let pendingRestoredActiveTabId: string | null = null;
let tabsSaveTimer = 0;

const firstTab = createQueryTab(null);
const tabs = ref<QueryTab[]>([firstTab]);
const activeTabId = ref(firstTab.id);
const selectedConnectionId = ref<string | null>(firstTab.connectionId);
const activeTab = computed(
  () =>
    tabs.value.find((tab) => tab.id === activeTabId.value) ?? tabs.value[0],
);

const tabOrder = ref<string[]>([firstTab.id]);
const activeWorkspaceTabId = ref<string>(firstTab.id);

const activeTableTab = computed(
  () =>
    tableTabs.value.find((tab) => tab.id === activeWorkspaceTabId.value) ??
    null,
);
const activeErTab = computed(
  () =>
    erTabs.value.find((tab) => tab.id === activeWorkspaceTabId.value) ?? null,
);
const activeTable = computed(() => activeTableTab.value?.table ?? null);
const activeTableConnectionId = computed(
  () => activeTableTab.value?.connectionId ?? null,
);
const tableMode = computed<"data" | "structure">({
  get: () => activeTableTab.value?.mode ?? "data",
  set: (value) => {
    if (activeTableTab.value) activeTableTab.value.mode = value;
  },
});
const viewMode = computed<"sql" | "table" | "er">(() =>
  activeTableTab.value ? "table" : activeErTab.value ? "er" : "sql",
);

const selectedId = computed<string | null>({
  get: () => selectedConnectionId.value,
  set: (value) => {
    selectedConnectionId.value = value;
  },
});

const sql = computed<string>({
  get: () => activeTab.value?.sql ?? "",
  set: (value) => {
    if (activeTab.value) activeTab.value.sql = value;
  },
});

const result = computed<QueryResult | null>({
  get: () => activeTab.value?.result ?? null,
  set: (value) => {
    if (activeTab.value) activeTab.value.result = value;
  },
});

const queryError = computed<string | null>({
  get: () => activeTab.value?.error ?? null,
  set: (value) => {
    if (activeTab.value) activeTab.value.error = value;
  },
});

const running = computed<boolean>({
  get: () => activeTab.value?.running ?? false,
  set: (value) => {
    if (activeTab.value) activeTab.value.running = value;
  },
});

const status = ref("就绪");
const dialogOpen = ref(false);
const editing = ref<ConnectionConfig | null>(null);
const connectionSaveError = ref("");
const connectionsLoaded = ref(false);
const collapsedGroups = ref<Record<string, boolean>>({});
const createOpen = ref(false);
const importOpen = ref(false);
const importTable = ref("");
const historyOpen = ref(false);
const historyLoading = ref(false);
const historyEntries = ref<HistoryEntry[]>([]);
const connectionStatus = ref<ConnectionStatus>("idle");
const connectionError = ref("");

interface ConfirmRequest {
  title: string;
  message: string;
  detail?: string | null;
  confirmText?: string;
  danger?: boolean;
  action: () => void | Promise<void>;
}

const confirmRequest = ref<ConfirmRequest | null>(null);

function askConfirm(request: ConfirmRequest) {
  confirmRequest.value = request;
}

async function runConfirmed() {
  const request = confirmRequest.value;
  confirmRequest.value = null;
  if (request) await request.action();
}

async function copyConnectionError() {
  if (!connectionError.value) return;
  try {
    await writeClipboard(connectionError.value);
    status.value = "已复制错误信息";
  } catch {
    status.value = connectionError.value;
  }
}

const selected = computed(
  () => connections.value.find((item) => item.id === selectedId.value) ?? null,
);
const isRedisConnection = computed(
  () => selected.value?.driver === "redis",
);

interface TabSource {
  connectionId: string | null;
  database: string | null;
  schema: string | null;
}

function scopeLabelFor(tab: TabSource): string {
  const connection = connectionFor(tab.connectionId);
  if (!connection) return "";
  if (connection.driver === "sqlite") return "main";
  if (connection.driver === "redis") return "";
  const database = tab.database ?? connection.database;
  if (connection.driver === "postgres" && tab.schema) {
    return `${database} · ${tab.schema}`;
  }
  return database ?? "";
}

function driverForConnection(connectionId: string | null): Driver {
  return connectionFor(connectionId)?.driver ?? "sqlite";
}

const activeTableScopeLabel = computed(() =>
  activeTableTab.value ? scopeLabelFor(activeTableTab.value) : "",
);

function tableTabTitleOf(tab: TableTab): string {
  const scope = scopeLabelFor(tab);
  return scope ? `${scope}.${tab.table.name}` : tab.table.name;
}

const treeActiveTableName = computed(() => {
  const tab = activeTableTab.value;
  if (!tab) return null;
  if (
    !sourceMatches(
      tab,
      selectedId.value,
      activeDatabase.value,
      activeSchema.value,
    )
  ) {
    return null;
  }
  return tab.table.name;
});

const isMacPlatform = navigator.platform.toUpperCase().includes("MAC");
const runShortcutLabel = computed(() => (isMacPlatform ? "⌘↩" : "Ctrl+Enter"));
const runDisabledReason = computed(() => {
  if (!selectedId.value) return "请先选择一个数据库连接";
  if (isRedisConnection.value) return "Redis 连接请使用命令控制台";
  if (connectionStatus.value !== "connected") {
    return connectionError.value || "连接不可用，请先刷新对象";
  }
  return "";
});
const menuShortcuts = computed(() => ({
  undo: isMacPlatform ? "⌘Z" : "Ctrl+Z",
  redo: isMacPlatform ? "⇧⌘Z" : "Ctrl+Shift+Z",
  cut: isMacPlatform ? "⌘X" : "Ctrl+X",
  copy: isMacPlatform ? "⌘C" : "Ctrl+C",
  paste: isMacPlatform ? "⌘V" : "Ctrl+V",
  selectAll: isMacPlatform ? "⌘A" : "Ctrl+A",
}));

const connectionStatusLabel = computed(() => {
  switch (connectionStatus.value) {
    case "connecting":
      return "连接中…";
    case "connected":
      return "已连接";
    case "failed":
      return "连接失败";
    default:
      return "未连接";
  }
});

const groupedConnections = computed<ConnectionGroup[]>(() => {
  const groups = new Map<string, ConnectionConfig[]>();

  for (const connection of connections.value) {
    const name = connection.group.trim() || "未分组";
    const items = groups.get(name) ?? [];
    items.push(connection);
    groups.set(name, items);
  }

  const metadata = new Map(
    connectionGroups.value.map((group) => [group.name, group]),
  );
  for (const name of metadata.keys()) {
    if (!groups.has(name) && name !== "未分组") {
      groups.set(name, []);
    }
  }

  return [...groups.entries()]
    .sort(([left], [right]) => {
      const leftMeta = metadata.get(left);
      const rightMeta = metadata.get(right);
      if (leftMeta && rightMeta) {
        return (
          leftMeta.order - rightMeta.order ||
          left.localeCompare(right, "zh-CN")
        );
      }
      if (leftMeta) return -1;
      if (rightMeta) return 1;
      if (left === "未分组") return 1;
      if (right === "未分组") return -1;
      return left.localeCompare(right, "zh-CN");
    })
    .map(([name, items]) => ({
      name,
      connections: items,
      color: metadata.get(name)?.color,
      order: metadata.get(name)?.order,
    }));
});

const filteredGroups = computed<ConnectionGroup[]>(() => {
  const query = connectionSearch.value.trim().toLowerCase();
  if (!query) return groupedConnections.value;

  return groupedConnections.value
    .map((group) => ({
      name: group.name,
      connections: group.connections.filter((connection) =>
        [
          connection.name,
          connection.driver,
          connection.group,
          connectionTarget(connection),
        ]
          .join(" ")
          .toLowerCase()
          .includes(query),
      ),
    }))
    .filter((group) => group.connections.length > 0);
});

function connectionTarget(connection: ConnectionConfig): string {
  if (connection.driver === "sqlite") return connection.file;
  if (connection.driver === "redis") {
    const value = connection.database || "0";
    const label = /^\d+$/.test(value)
      ? `db${value.padStart(2, "0")}`
      : `db${value}`;
    return `${connection.host}:${connection.port} · ${label}`;
  }
  return `${connection.username}@${connection.host}:${connection.port}/${connection.database}`;
}

function connectionFor(id: string | null): ConnectionConfig | null {
  return connections.value.find((connection) => connection.id === id) ?? null;
}

function connectionDatabase(connectionId: string | null): string | null {
  if (!connectionId) return null;
  return (
    databaseByConnection[connectionId] ??
    connectionFor(connectionId)?.database ??
    null
  );
}

function queryTabTitle(tab: QueryTab): string {
  return `${tab.title} · ${tabSourceLabel(tab)}`;
}

function tabSourceLabel(tab: TabSource): string {
  const connection = connectionFor(tab.connectionId);
  const parts = [connection?.name ?? "未连接"];
  if (tab.database) parts.push(tab.database);
  if (tab.schema && tab.schema !== tab.database) parts.push(tab.schema);
  return parts.join(" · ");
}

function queryTabSource(tab: QueryTab): string {
  return tabSourceLabel(tab);
}

function sourceMatches(
  tab: TabSource,
  connectionId: string | null,
  database: string | null,
  schema: string | null,
): boolean {
  return (
    tab.connectionId === connectionId &&
    normalizeScopeValue(tab.database) === normalizeScopeValue(database) &&
    normalizeScopeValue(tab.schema) === normalizeScopeValue(schema)
  );
}

function createTableTab(
  table: TableInfo,
  connectionId: string | null,
  database: string | null,
  schema: string | null,
  mode: "data" | "structure",
  focusedIndex: string | null = null,
): TableTab {
  tableSequence += 1;
  return {
    id: `table-${Date.now()}-${tableSequence}`,
    connectionId,
    database: normalizeScopeValue(database),
    schema: normalizeScopeValue(schema),
    table,
    mode,
    focusedIndex,
  };
}

function createErTab(
  connectionId: string | null,
  database: string | null,
  schema: string | null,
): ErTab {
  erSequence += 1;
  return {
    id: `er-${Date.now()}-${erSequence}`,
    connectionId,
    database: normalizeScopeValue(database),
    schema: normalizeScopeValue(schema),
  };
}

function removeTabFromOrder(id: string) {
  tabOrder.value = tabOrder.value.filter((item) => item !== id);
}

function neighborTabId(id: string): string | null {
  const index = tabOrder.value.indexOf(id);
  if (index < 0) return null;
  return tabOrder.value[index + 1] ?? tabOrder.value[index - 1] ?? null;
}

async function activateWorkspaceTabById(id: string) {
  const query = tabs.value.find((tab) => tab.id === id);
  if (query) {
    await showQueryTab(id);
    return;
  }
  const table = tableTabs.value.find((tab) => tab.id === id);
  if (table) {
    await selectTableTab(table);
    return;
  }
  const er = erTabs.value.find((tab) => tab.id === id);
  if (er) await showErTab(er);
}

interface WorkspaceTabView {
  id: string;
  kind: "query" | "table" | "er";
  label: string;
  title: string;
  source: string;
  icon: "code" | "table" | "diagram";
  running: boolean;
  closable: boolean;
}

const workspaceTabViews = computed<WorkspaceTabView[]>(() => {
  const views: WorkspaceTabView[] = [];
  for (const id of tabOrder.value) {
    const query = tabs.value.find((tab) => tab.id === id);
    if (query) {
      views.push({
        id,
        kind: "query",
        label: query.title,
        title: queryTabTitle(query),
        source: queryTabSource(query),
        icon: "code",
        running: query.running,
        closable: tabs.value.length > 1,
      });
      continue;
    }
    const table = tableTabs.value.find((tab) => tab.id === id);
    if (table) {
      views.push({
        id,
        kind: "table",
        label: table.table.name,
        title: tableTabTitleOf(table),
        source: tabSourceLabel(table),
        icon: "table",
        running: false,
        closable: true,
      });
      continue;
    }
    const er = erTabs.value.find((tab) => tab.id === id);
    if (er) {
      views.push({
        id,
        kind: "er",
        label: "ER 图",
        title: `ER 图 · ${tabSourceLabel(er)}`,
        source: tabSourceLabel(er),
        icon: "diagram",
        running: false,
        closable: true,
      });
    }
  }
  return views;
});

function ensureTabForSource(activate = true): QueryTab {
  const connectionId = selectedId.value;
  const database = activeDatabase.value;
  const schema = activeSchema.value;
  let tab = tabs.value.find((item) =>
    sourceMatches(item, connectionId, database, schema),
  );
  if (!tab) {
    const placeholder =
      tabs.value.length === 1 &&
      tabs.value[0].connectionId === null &&
      tabs.value[0].sql === "SELECT 1;" &&
      !tabs.value[0].inTransaction
        ? tabs.value[0]
        : null;
    if (placeholder) {
      placeholder.connectionId = connectionId;
      placeholder.database = normalizeScopeValue(database);
      placeholder.schema = normalizeScopeValue(schema);
      tab = placeholder;
    } else {
      tab = createQueryTab(connectionId, database, schema);
      tabs.value.push(tab);
      tabOrder.value.push(tab.id);
    }
  }
  if (activate) {
    activeTabId.value = tab.id;
    activeWorkspaceTabId.value = tab.id;
  }
  return tab;
}

interface StoredQueryTab {
  id: string;
  title: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  sql: string;
}

function restoreQueryTabs(list: ConnectionConfig[]) {
  if (tabsRestored) return;
  tabsRestored = true;
  try {
    const raw = window.localStorage.getItem(QUERY_TABS_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as {
      tabs?: StoredQueryTab[];
      activeTabId?: string;
    };
    if (!Array.isArray(parsed.tabs) || !parsed.tabs.length) return;
    const byId = new Map(list.map((connection) => [connection.id, connection]));
    const restored: QueryTab[] = [];
    for (const stored of parsed.tabs) {
      if (!stored || typeof stored.sql !== "string") continue;
      const connection = stored.connectionId
        ? byId.get(stored.connectionId)
        : undefined;
      if (!connection || connection.driver === "redis") continue;
      const tab = createQueryTab(
        connection.id,
        stored.database ?? null,
        stored.schema ?? null,
      );
      tab.id = stored.id || tab.id;
      tab.title = stored.title || tab.title;
      tab.sql = stored.sql;
      restored.push(tab);
    }
    if (!restored.length) return;
    tabs.value = restored;
    tabOrder.value = restored.map((tab) => tab.id);
    const active =
      restored.find((tab) => tab.id === parsed.activeTabId) ?? restored[0];
    activeTabId.value = active.id;
    activeWorkspaceTabId.value = active.id;
    selectedConnectionId.value = active.connectionId;
    if (active.database) {
      databaseByConnection[active.connectionId!] = active.database;
    }
    if (active.schema) {
      schemaByConnection[active.connectionId!] = active.schema;
    }
    pendingRestoredActiveTabId = active.id;
    const highest = restored.reduce((max, tab) => {
      const match = tab.title.match(/(\d+)\s*$/);
      return Math.max(max, match ? Number(match[1]) : 0);
    }, 0);
    querySequence = Math.max(querySequence, highest);
  } catch {
    // Ignore malformed local state and keep the default tab.
  }
}

const queryTabsSnapshot = computed(() =>
  JSON.stringify({
    tabs: tabs.value.map((tab) => ({
      id: tab.id,
      title: tab.title,
      connectionId: tab.connectionId,
      database: tab.database,
      schema: tab.schema,
      sql: tab.sql,
    })),
    activeTabId: activeTabId.value,
  }),
);

watch(queryTabsSnapshot, (value) => {
  if (!tabsRestored) return;
  window.clearTimeout(tabsSaveTimer);
  tabsSaveTimer = window.setTimeout(() => {
    try {
      window.localStorage.setItem(QUERY_TABS_STORAGE_KEY, value);
    } catch {
      // Local storage may be unavailable; tabs still work in memory.
    }
  }, 250);
});

function closeStaleTableTab() {
  const connectionId = selectedId.value;
  const database = normalizeScopeValue(activeDatabase.value);
  const schema = normalizeScopeValue(activeSchema.value);
  const stale = tableTabs.value.filter(
    (tab) =>
      sourceMatches(tab, connectionId, database, schema) &&
      !tables.value.some(
        (item) =>
          item.name === tab.table.name && item.kind === tab.table.kind,
      ),
  );
  if (!stale.length) return;

  const staleIds = new Set(stale.map((tab) => tab.id));
  const wasActive = staleIds.has(activeWorkspaceTabId.value);
  const neighbor = wasActive
    ? neighborTabId(activeWorkspaceTabId.value)
    : null;
  tableTabs.value = tableTabs.value.filter((tab) => !staleIds.has(tab.id));
  tabOrder.value = tabOrder.value.filter((id) => !staleIds.has(id));
  if (wasActive) {
    if (neighbor && !staleIds.has(neighbor)) {
      void activateWorkspaceTabById(neighbor);
    } else {
      fallbackToQueryTab();
    }
  }
}

async function refreshTables() {
  const connectionId = selectedId.value;
  if (!connectionId) {
    tables.value = [];
    tablesConnectionId.value = null;
    databases.value = [];
    activeDatabase.value = null;
    schemas.value = [];
    activeSchema.value = null;
    connectionStatus.value = "idle";
    connectionError.value = "";
    tablesLoading.value = false;
    return;
  }

  const request = ++tablesRequest;
  if (selected.value?.driver === "redis") {
    tables.value = [];
    tablesConnectionId.value = null;
    databases.value = [];
    activeDatabase.value = null;
    schemas.value = [];
    activeSchema.value = null;
    for (const key of Object.keys(databaseObjects)) {
      delete databaseObjects[key];
    }
    tablesLoading.value = true;
    connectionStatus.value = "connecting";
    connectionError.value = "";
    try {
      await api.redisPing(connectionId);
      if (request !== tablesRequest) return;
      connectionStatus.value = "connected";
      status.value = `${selected.value?.name ?? "Redis"} · Redis`;
    } catch (reason) {
      if (request !== tablesRequest) return;
      connectionStatus.value = "failed";
      connectionError.value = String(reason);
      status.value = connectionError.value;
    } finally {
      if (request === tablesRequest) {
        tablesLoading.value = false;
      }
    }
    return;
  }

  if (selected.value?.driver === "mysql") {
    tablesLoading.value = true;
    connectionStatus.value = "connecting";
    connectionError.value = "";
    if (tablesConnectionId.value !== connectionId) {
      tables.value = [];
    }
    try {
      const names = await api.listDatabases(connectionId);
      if (request !== tablesRequest) return;
      databases.value = names;
      // Seed every database with an empty list so counts exist even for
      // databases that contain no objects.
      for (const key of Object.keys(databaseObjects)) {
        delete databaseObjects[key];
      }
      for (const name of names) {
        databaseObjects[name] = [];
      }
      if (!names.length) {
        activeDatabase.value = null;
        tables.value = [];
        connectionStatus.value = "connected";
        return;
      }
      const preferred = connectionDatabase(connectionId);
      activeDatabase.value = names.includes(preferred ?? "")
        ? preferred
        : names[0];
      if (activeDatabase.value) {
        databaseByConnection[connectionId] = activeDatabase.value;
        await api.switchDatabase(connectionId, activeDatabase.value);
        scopeRevision.value += 1;
        if (request !== tablesRequest) return;
      }

      // One metadata pass gives every database its objects and counts; the
      // tree no longer waits for a database to be opened.
      const grouped = await api.listDatabaseObjects(connectionId);
      if (request !== tablesRequest) return;
      for (const entry of grouped) {
        databaseObjects[entry.database] = entry.objects;
      }
      tables.value = activeDatabase.value
        ? databaseObjects[activeDatabase.value] ?? []
        : [];
      tablesConnectionId.value = connectionId;
      connectionStatus.value = "connected";
      closeStaleTableTab();
      status.value = `${selected.value?.name ?? "当前连接"} · ${databases.value.length} 个数据库 · ${tables.value.length} 个对象`;
    } catch (reason) {
      if (request !== tablesRequest) return;
      databases.value = [];
      tables.value = [];
      connectionStatus.value = "failed";
      connectionError.value = String(reason);
      status.value = connectionError.value;
    } finally {
      if (request === tablesRequest) {
        tablesLoading.value = false;
      }
    }
    return;
  } else if (selected.value?.driver === "postgres") {
    for (const key of Object.keys(databaseObjects)) {
      delete databaseObjects[key];
    }
    try {
      const names = await api.listDatabases(connectionId);
      if (request !== tablesRequest) return;
      databases.value = names;
      const preferred = connectionDatabase(connectionId) ?? "postgres";
      activeDatabase.value = names.includes(preferred)
        ? preferred
        : names[0] ?? preferred;
      if (activeDatabase.value && names.length) {
        databaseByConnection[connectionId] = activeDatabase.value;
      }

      const schemaNames = await api.listSchemas(connectionId);
      if (request !== tablesRequest) return;
      schemas.value = schemaNames;
      if (!schemaNames.length) {
        activeSchema.value = null;
        tables.value = [];
        connectionStatus.value = "connected";
        tablesLoading.value = false;
        return;
      }
      let schema = schemaByConnection[connectionId] ?? "public";
      if (!schemaNames.includes(schema)) {
        schema = schemaNames.includes("public") ? "public" : schemaNames[0];
      }
      activeSchema.value = schema;
      schemaByConnection[connectionId] = schema;
      await api.switchSchema(connectionId, schema);
      scopeRevision.value += 1;
      if (request !== tablesRequest) return;
    } catch (reason) {
      if (request !== tablesRequest) return;
      databases.value = [];
      schemas.value = [];
      activeSchema.value = null;
      tables.value = [];
      connectionStatus.value = "failed";
      connectionError.value = String(reason);
      status.value = connectionError.value;
      tablesLoading.value = false;
      return;
    }
  } else {
    databases.value = [];
    activeDatabase.value = null;
    schemas.value = [];
    activeSchema.value = null;
    for (const key of Object.keys(databaseObjects)) {
      delete databaseObjects[key];
    }
  }

  if (tablesConnectionId.value !== connectionId) {
    tables.value = [];
  }
  tablesLoading.value = true;
  connectionStatus.value = "connecting";
  connectionError.value = "";
  try {
    const list = await api.listTables(connectionId);
    if (request !== tablesRequest) return;
    tables.value = list;
    tablesConnectionId.value = connectionId;
    connectionStatus.value = "connected";
    closeStaleTableTab();
    status.value = `${selected.value?.name ?? "当前连接"} · ${list.length} 个对象`;
  } catch (reason) {
    if (request !== tablesRequest) return;
    tables.value = [];
    tablesConnectionId.value = connectionId;
    connectionStatus.value = "failed";
    connectionError.value = String(reason);
    status.value = connectionError.value;
  } finally {
    if (request === tablesRequest) {
      tablesLoading.value = false;
    }
  }
}

async function refreshConnections(selectId?: string) {
  await loadConnectionGroups();
  const list = await api.listConnections();
  connections.value = list;
  connectionsLoaded.value = true;
  const ids = new Set(list.map((connection) => connection.id));
  restoreQueryTabs(list);

  for (const tab of tabs.value) {
    if (tab.connectionId && !ids.has(tab.connectionId)) {
      tab.result = null;
      tab.error = null;
    }
  }
  const removedTabIds = new Set(
    tabs.value
      .filter((tab) => tab.connectionId && !ids.has(tab.connectionId))
      .map((tab) => tab.id),
  );
  if (removedTabIds.size) {
    tabs.value = tabs.value.filter((tab) => !removedTabIds.has(tab.id));
    tabOrder.value = tabOrder.value.filter((id) => !removedTabIds.has(id));
    if (!tabs.value.some((tab) => tab.id === activeTabId.value)) {
      activeTabId.value = tabs.value[0]?.id ?? "";
    }
    if (removedTabIds.has(activeWorkspaceTabId.value)) {
      activeWorkspaceTabId.value = activeTabId.value;
    }
  }
  for (const id of Object.keys(databaseByConnection)) {
    if (!ids.has(id)) {
      delete databaseByConnection[id];
    }
  }
  for (const id of Object.keys(schemaByConnection)) {
    if (!ids.has(id)) {
      delete schemaByConnection[id];
    }
  }

  const removedWorkspaceTabs = new Set<string>();
  for (const tab of tableTabs.value) {
    if (tab.connectionId && !ids.has(tab.connectionId)) {
      removedWorkspaceTabs.add(tab.id);
    }
  }
  for (const tab of erTabs.value) {
    if (tab.connectionId && !ids.has(tab.connectionId)) {
      removedWorkspaceTabs.add(tab.id);
    }
  }
  if (removedWorkspaceTabs.size) {
    tableTabs.value = tableTabs.value.filter(
      (tab) => !removedWorkspaceTabs.has(tab.id),
    );
    erTabs.value = erTabs.value.filter(
      (tab) => !removedWorkspaceTabs.has(tab.id),
    );
    tabOrder.value = tabOrder.value.filter(
      (id) => !removedWorkspaceTabs.has(id),
    );
    if (removedWorkspaceTabs.has(activeWorkspaceTabId.value)) {
      activeWorkspaceTabId.value = activeTabId.value;
    }
  }

  if (selectId && ids.has(selectId)) {
    selectedId.value = selectId;
  }
  if (!selectedId.value && list.length) {
    selectedId.value = list[0].id;
  }
  if (selectedId.value && !ids.has(selectedId.value)) {
    selectedId.value = list[0]?.id ?? null;
  }

  queryError.value = null;
  activeDatabase.value = connectionDatabase(selectedId.value);
  if (selectedId.value) {
    await refreshTables();
    const connection = connectionFor(selectedId.value);
    if (connection && connection.driver !== "redis") {
      ensureTabForSource(true);
      if (pendingRestoredActiveTabId) {
        const restoredTab = tabs.value.find(
          (tab) => tab.id === pendingRestoredActiveTabId,
        );
        if (
          restoredTab &&
          sourceMatches(
            restoredTab,
            selectedConnectionId.value,
            activeDatabase.value,
            activeSchema.value,
          )
        ) {
          activeTabId.value = restoredTab.id;
          activeWorkspaceTabId.value = restoredTab.id;
          if (restoredTab.database) {
            activeDatabase.value = restoredTab.database;
          }
          if (restoredTab.schema) {
            activeSchema.value = restoredTab.schema;
          }
        }
        pendingRestoredActiveTabId = null;
      }
    }
  } else {
    tables.value = [];
    tablesConnectionId.value = null;
    databases.value = [];
    activeDatabase.value = null;
    schemas.value = [];
    activeSchema.value = null;
    result.value = null;
    connectionStatus.value = "idle";
    connectionError.value = "";
  }
}

async function selectConnection(id: string) {
  selectedId.value = id;
  activeDatabase.value = connectionDatabase(id);
  activeSchema.value = schemaByConnection[id] ?? null;
  connectionStatus.value = "connecting";
  connectionError.value = "";
  await refreshTables();
  const connection = connectionFor(id);
  if (connection && connection.driver !== "redis") {
    ensureTabForSource(true);
  }
}

async function selectDatabase(database: string) {
  const connectionId = selectedId.value;
  const driver = selected.value?.driver;
  if (!connectionId || (driver !== "mysql" && driver !== "postgres")) return;
  if (database === activeDatabase.value) return;
  if (connectionHasTransaction(connectionId)) {
    status.value = "该连接有未提交事务，请先提交或回滚后再切换数据库";
    return;
  }

  const previous = activeDatabase.value;
  activeDatabase.value = database;
  if (driver === "postgres") {
    schemas.value = [];
    activeSchema.value = null;
  }
  tables.value = [];
  connectionStatus.value = "connecting";
  connectionError.value = "";
  status.value = `切换到数据库：${database}`;
  try {
    await api.switchDatabase(connectionId, database);
    scopeRevision.value += 1;
    databaseByConnection[connectionId] = database;
    if (driver === "mysql") {
      tables.value = databaseObjects[database] ?? [];
      tablesConnectionId.value = connectionId;
      connectionStatus.value = "connected";
      status.value = `已切换到数据库：${database} · ${tables.value.length} 个对象`;
      ensureTabForSource(true);
      return;
    }
    await refreshTables();
    ensureTabForSource(true);
    status.value = `已切换到数据库：${database}`;
  } catch (reason) {
    activeDatabase.value = previous;
    connectionStatus.value = "failed";
    connectionError.value = String(reason);
    status.value = connectionError.value;
  }
}

async function selectSchema(schema: string) {
  const connectionId = selectedId.value;
  if (!connectionId || selected.value?.driver !== "postgres") return;
  if (schema === activeSchema.value) return;
  if (connectionHasTransaction(connectionId)) {
    status.value = "该连接有未提交事务，请先提交或回滚后再切换模式";
    return;
  }

  const previous = activeSchema.value;
  activeSchema.value = schema;
  tables.value = [];
  connectionStatus.value = "connecting";
  connectionError.value = "";
  status.value = `切换到模式：${schema}`;
  try {
    await api.switchSchema(connectionId, schema);
    scopeRevision.value += 1;
    schemaByConnection[connectionId] = schema;
    await refreshTables();
    ensureTabForSource(true);
    status.value = `已切换到模式：${schema}`;
  } catch (reason) {
    activeSchema.value = previous;
    connectionStatus.value = "failed";
    connectionError.value = String(reason);
    status.value = connectionError.value;
  }
}

function toggleGroup(name: string) {
  collapsedGroups.value[name] = !collapsedGroups.value[name];
}

function newQuery() {
  if (isRedisConnection.value) {
    status.value = "Redis 连接请使用命令控制台";
    return;
  }
  let connectionId = selectedId.value;
  if (!connectionId && connections.value.length) {
    connectionId = connections.value[0].id;
    selectedId.value = connectionId;
    activeDatabase.value = connectionDatabase(connectionId);
    activeSchema.value = schemaByConnection[connectionId] ?? null;
    void refreshTables();
  }
  const tab = createQueryTab(
    connectionId,
    activeDatabase.value,
    activeSchema.value,
  );
  tabs.value.push(tab);
  tabOrder.value.push(tab.id);
  activeTabId.value = tab.id;
  activeWorkspaceTabId.value = tab.id;
  status.value = `新建查询：${tab.title}`;
  if (connectionId && tablesConnectionId.value !== connectionId) {
    void refreshTables();
  }
}

function openGeneratedSql(payload: { sql: string; title: string }) {
  const connectionId = selectedId.value ?? activeTableConnectionId.value;
  const tab = createQueryTab(
    connectionId,
    connectionDatabase(connectionId),
    connectionId ? schemaByConnection[connectionId] ?? null : null,
  );
  tab.title = payload.title;
  tab.sql = payload.sql;
  tabs.value.push(tab);
  tabOrder.value.push(tab.id);
  activeTabId.value = tab.id;
  activeWorkspaceTabId.value = tab.id;
  status.value = `已生成 SQL：${payload.title}`;
}

async function activateTabSource(tab: TabSource): Promise<boolean> {
  const connectionId = tab.connectionId;
  if (!connectionId) return true;
  const connection = connectionFor(connectionId);
  if (!connection) {
    status.value = "标签所属连接不存在";
    return false;
  }
  if (connection.driver === "redis") {
    status.value = "Redis 连接请使用命令控制台";
    return false;
  }

  const targetDatabase = tab.database ?? connectionDatabase(connectionId);
  const targetSchema =
    tab.schema ?? schemaByConnection[connectionId] ?? null;
  const sameConnection = connectionId === selectedConnectionId.value;
  const currentDatabase = sameConnection
    ? activeDatabase.value
    : databaseByConnection[connectionId] ?? connection.database ?? null;
  const currentSchema = sameConnection
    ? activeSchema.value
    : schemaByConnection[connectionId] ?? null;
  const needsDatabase =
    (targetDatabase ?? null) !== (currentDatabase ?? null);
  const needsSchema =
    connection.driver === "postgres" &&
    (targetSchema ?? null) !== (currentSchema ?? null);

  if (
    (needsDatabase || needsSchema) &&
    connectionHasTransaction(connectionId)
  ) {
    status.value =
      "该连接有未提交事务，请先提交或回滚后再切换查询标签";
    return false;
  }

  try {
    if (needsDatabase && targetDatabase) {
      await api.switchDatabase(connectionId, targetDatabase);
      databaseByConnection[connectionId] = targetDatabase;
    }
    if (
      connection.driver === "postgres" &&
      targetSchema &&
      (needsSchema || needsDatabase)
    ) {
      await api.switchSchema(connectionId, targetSchema);
      schemaByConnection[connectionId] = targetSchema;
    }
  } catch (reason) {
    status.value = String(reason);
    return false;
  }

  selectedConnectionId.value = connectionId;
  activeDatabase.value = targetDatabase ?? null;
  if (connection.driver === "postgres") {
    activeSchema.value = targetSchema;
  } else {
    activeSchema.value = null;
  }

  if (!sameConnection || needsDatabase || needsSchema) {
    connectionStatus.value = "connecting";
    connectionError.value = "";
    await refreshTables();
  }
  return true;
}

async function showQueryTab(id: string) {
  const tab = tabs.value.find((item) => item.id === id);
  if (!tab) return;
  if (!tab.connectionId && connections.value.length) {
    tab.connectionId = connections.value[0].id;
    tab.database = connectionDatabase(tab.connectionId);
    tab.schema = schemaByConnection[tab.connectionId] ?? null;
  }
  if (!(await activateTabSource(tab))) return;

  activeTabId.value = id;
  activeWorkspaceTabId.value = id;
  activeDatabase.value = tab.database ?? activeDatabase.value;
  if (tab.schema) activeSchema.value = tab.schema;
  status.value = `切换到：${tab.title}`;
}

async function selectTableTab(tab: TableTab) {
  if (!tableTabs.value.some((item) => item.id === tab.id)) return;
  if (!(await activateTabSource(tab))) return;
  if (!tableTabs.value.some((item) => item.id === tab.id)) return;
  activeWorkspaceTabId.value = tab.id;
  status.value =
    tab.mode === "structure"
      ? `查看表结构：${tab.table.name}`
      : `查看表数据：${tab.table.name}`;
}

async function showErTab(tab: ErTab) {
  if (!erTabs.value.some((item) => item.id === tab.id)) return;
  if (!(await activateTabSource(tab))) return;
  if (!erTabs.value.some((item) => item.id === tab.id)) return;
  activeWorkspaceTabId.value = tab.id;
  status.value = "ER 图";
}

function connectionHasTransaction(connectionId: string): boolean {
  return tabs.value.some(
    (tab) =>
      tab.connectionId === connectionId &&
      (tab.inTransaction || tab.transactionBusy),
  );
}

function resetConnectionTransactions(connectionId: string) {
  for (const tab of tabs.value) {
    if (
      tab.connectionId === connectionId &&
      (tab.inTransaction || tab.transactionBusy)
    ) {
      void api.closeTransactionSession(tab.id);
      tab.inTransaction = false;
      tab.transactionBusy = false;
    }
  }
}

function updateTabScroll() {
  const strip = tabStrip.value;
  if (!strip) {
    tabScrollLeft.value = false;
    tabScrollRight.value = false;
    return;
  }
  tabScrollLeft.value = strip.scrollLeft > 2;
  tabScrollRight.value =
    strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 2;
}

function scrollTabs(direction: number) {
  const strip = tabStrip.value;
  if (!strip) return;
  strip.scrollBy({
    left: direction * Math.max(160, strip.clientWidth * 0.6),
    behavior: "smooth",
  });
}

function onTabWheel(event: WheelEvent) {
  const strip = tabStrip.value;
  if (!strip || strip.scrollWidth <= strip.clientWidth + 2) return;
  const delta =
    Math.abs(event.deltaY) > Math.abs(event.deltaX)
      ? event.deltaY
      : event.deltaX;
  if (!delta) return;
  strip.scrollLeft += delta;
  event.preventDefault();
  updateTabScroll();
}

async function scrollActiveTabIntoView() {
  await nextTick();
  const strip = tabStrip.value;
  if (!strip) {
    updateTabScroll();
    return;
  }
  const active = strip.querySelector<HTMLElement>(
    ".workspace-tab.active",
  );
  if (active) {
    const stripRect = strip.getBoundingClientRect();
    const activeRect = active.getBoundingClientRect();
    if (activeRect.left < stripRect.left) {
      strip.scrollLeft -= stripRect.left - activeRect.left;
    } else if (activeRect.right > stripRect.right) {
      strip.scrollLeft += activeRect.right - stripRect.right;
    }
  }
  updateTabScroll();
}

watch(
  [() => tabOrder.value.length, activeWorkspaceTabId],
  () => {
    void scrollActiveTabIntoView();
  },
  { immediate: true },
);

function closeQueryTab(id: string) {
  if (tabs.value.length <= 1) return;

  const index = tabs.value.findIndex((tab) => tab.id === id);
  if (index < 0) return;

  const closing = tabs.value[index];
  if (closing.inTransaction || closing.transactionBusy) {
    void api.closeTransactionSession(id);
  }

  const wasActive = activeWorkspaceTabId.value === id;
  const neighbor = neighborTabId(id);
  tabs.value.splice(index, 1);
  removeTabFromOrder(id);
  if (!wasActive) return;

  if (neighbor) {
    void activateWorkspaceTabById(neighbor);
    return;
  }
  const replacement = tabs.value[0];
  if (replacement) {
    activeTabId.value = replacement.id;
    activeWorkspaceTabId.value = replacement.id;
  }
}

function closeTableTab(id: string) {
  const tab = tableTabs.value.find((item) => item.id === id);
  if (!tab) return;
  const wasActive = activeWorkspaceTabId.value === id;
  const neighbor = neighborTabId(id);
  tableTabs.value = tableTabs.value.filter((item) => item.id !== id);
  removeTabFromOrder(id);
  if (wasActive) {
    if (neighbor) {
      void activateWorkspaceTabById(neighbor);
    } else {
      fallbackToQueryTab();
    }
  }
  status.value = `已关闭表标签：${tab.table.name}`;
}

function closeErTab(id: string) {
  const tab = erTabs.value.find((item) => item.id === id);
  if (!tab) return;
  const wasActive = activeWorkspaceTabId.value === id;
  const neighbor = neighborTabId(id);
  erTabs.value = erTabs.value.filter((item) => item.id !== id);
  removeTabFromOrder(id);
  if (wasActive) {
    if (neighbor) {
      void activateWorkspaceTabById(neighbor);
    } else {
      fallbackToQueryTab();
    }
  }
  status.value = "已关闭 ER 图标签";
}

function closeWorkspaceTab(id: string) {
  if (tabs.value.some((tab) => tab.id === id)) {
    closeQueryTab(id);
    return;
  }
  if (tableTabs.value.some((tab) => tab.id === id)) {
    closeTableTab(id);
    return;
  }
  closeErTab(id);
}

function fallbackToQueryTab() {
  const id = activeTabId.value;
  if (!id) return;
  activeWorkspaceTabId.value = id;
  if (tabs.value.some((tab) => tab.id === id)) {
    void showQueryTab(id);
  }
}

function closeTabsForConnection(connectionId: string) {
  const removed = new Set(
    [...tableTabs.value, ...erTabs.value]
      .filter((tab) => tab.connectionId === connectionId)
      .map((tab) => tab.id),
  );
  if (!removed.size) return;
  tableTabs.value = tableTabs.value.filter((tab) => !removed.has(tab.id));
  erTabs.value = erTabs.value.filter((tab) => !removed.has(tab.id));
  tabOrder.value = tabOrder.value.filter((id) => !removed.has(id));
  if (removed.has(activeWorkspaceTabId.value)) {
    fallbackToQueryTab();
  }
}

function openNewConnection() {
  newConnectionGroup.value = "";
  editing.value = null;
  connectionSaveError.value = "";
  dialogOpen.value = true;
}

function openNewConnectionInGroup(group: string) {
  closeGroupMenu();
  openNewConnection();
  newConnectionGroup.value = group;
}

function openEditConnection(connection: ConnectionConfig) {
  editing.value = connection;
  connectionSaveError.value = "";
  dialogOpen.value = true;
}

function editSelectedConnection() {
  if (selected.value) {
    openEditConnection(selected.value);
  }
}

async function handleSave(payload: SavePayload) {
  connectionSaveError.value = "";
  try {
    const saved = await api.saveConnection(
      payload.config,
      payload.password,
      payload.sshPassword,
    );
    resetConnectionTransactions(saved.id);
    dialogOpen.value = false;
    delete databaseByConnection[saved.id];
    closeTabsForConnection(saved.id);
    await refreshConnections(saved.id);
    status.value = `已保存连接：${saved.name}`;
  } catch (reason) {
    connectionSaveError.value = String(reason);
    status.value = connectionSaveError.value;
  }
}

function handleDelete(connection: ConnectionConfig) {
  askConfirm({
    title: "删除连接",
    message: `确定删除连接「${connection.name}」？`,
    detail: `${connectionTarget(connection)}\n只会删除本机连接配置，不会影响数据库。`,
    confirmText: "删除连接",
    action: async () => {
      try {
        await api.deleteConnection(connection.id);
        resetConnectionTransactions(connection.id);
        delete databaseByConnection[connection.id];
        delete schemaByConnection[connection.id];
        for (const tab of tabs.value) {
          if (tab.connectionId === connection.id) {
            tab.connectionId = null;
            tab.result = null;
            tab.error = null;
          }
        }
        closeTabsForConnection(connection.id);
        await refreshConnections();
        status.value = `已删除连接：${connection.name}`;
      } catch (reason) {
        status.value = String(reason);
      }
    },
  });
}

async function loadConnectionGroups() {
  try {
    connectionGroups.value = await api.listConnectionGroups();
  } catch {
    connectionGroups.value = [];
  }
}

const connectionGroupsForMenu = computed(() =>
  groupedConnections.value
    .map((group) => group.name)
    .filter((name) => name !== "未分组"),
);

function openConnectionMenu(
  event: MouseEvent,
  connection: ConnectionConfig,
) {
  event.preventDefault();
  event.stopPropagation();
  connectionMenuInfoOpen.value = false;
  connectionMenuGroupOpen.value = false;
  connectionMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 240)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 320)),
    connection,
  };
}

function closeConnectionMenu() {
  connectionMenu.value = null;
  connectionMenuInfoOpen.value = false;
  connectionMenuGroupOpen.value = false;
}

function openGroupMenu(event: MouseEvent, group: string) {
  event.preventDefault();
  event.stopPropagation();
  groupMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 200)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 260)),
    group,
  };
}

function closeGroupMenu() {
  groupMenu.value = null;
}

function menuRefreshConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  if (selectedId.value === connection.id) {
    void refreshTables();
  } else {
    void selectConnection(connection.id);
  }
}

function openQueryForConnection(connectionId: string) {
  if (connectionFor(connectionId)?.driver === "redis") {
    status.value = "Redis 连接请使用命令控制台";
    return;
  }
  selectedId.value = connectionId;
  const database = connectionDatabase(connectionId);
  const schema = schemaByConnection[connectionId] ?? null;
  activeDatabase.value = database;
  activeSchema.value = schema;
  const tab = createQueryTab(connectionId, database, schema);
  tabs.value.push(tab);
  tabOrder.value.push(tab.id);
  activeTabId.value = tab.id;
  activeWorkspaceTabId.value = tab.id;
  status.value = `新建查询：${tab.title}`;
  void refreshTables();
}

function menuConnectionNewQuery() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (connection) {
    openQueryForConnection(connection.id);
  }
}

async function menuCloseConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  try {
    await api.closeConnection(connection.id);
    resetConnectionTransactions(connection.id);
    if (selectedId.value === connection.id) {
      connectionStatus.value = "idle";
      tables.value = [];
      databases.value = [];
      schemas.value = [];
      for (const key of Object.keys(databaseObjects)) {
        delete databaseObjects[key];
      }
    }
    status.value = `已关闭连接：${connection.name}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

function menuEditConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (connection) {
    openEditConnection(connection);
  }
}

function menuDeleteConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (connection) {
    handleDelete(connection);
  }
}

function menuDuplicateConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  duplicateTarget.value = connection;
  duplicateOpen.value = true;
}

async function confirmDuplicateConnection(
  name: string,
  copySecrets: boolean,
) {
  const connection = duplicateTarget.value;
  duplicateOpen.value = false;
  duplicateTarget.value = null;
  if (!connection) return;
  try {
    const created = await api.duplicateConnection(
      connection.id,
      name,
      copySecrets,
    );
    await refreshConnections();
    status.value = `已复制连接：${created.name}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

async function menuToggleReadOnly() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  try {
    await api.setConnectionReadOnly(connection.id, !connection.readOnly);
    resetConnectionTransactions(connection.id);
    await refreshConnections();
    status.value = connection.readOnly
      ? `已取消只读：${connection.name}`
      : `已设为只读：${connection.name}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

async function copyConnectionInfo(format: ConnectionInfoFormat) {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;

  if (format === "urlWithPassword") {
    askConfirm({
      title: "复制含密码的连接信息",
      message: `将把「${connection.name}」的密码写入剪贴板，确定继续？`,
      detail: "剪贴板内容可能被其他应用读取，使用后请及时清除。",
      confirmText: "复制",
      action: async () => {
        try {
          const info = await api.connectionInfo(connection.id, format, true);
          await writeClipboard(info);
          status.value = "已复制含密码的连接信息";
        } catch (reason) {
          status.value = String(reason);
        }
      },
    });
    return;
  }

  try {
    const info = await api.connectionInfo(connection.id, format, false);
    await writeClipboard(info);
    status.value = "已复制连接信息";
  } catch (reason) {
    status.value = String(reason);
  }
}

async function moveConnectionToGroup(group: string | null) {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  try {
    await api.moveConnectionToGroup(connection.id, group);
    await refreshConnections();
    status.value = group ? `已移动到分组：${group}` : "已移到未分组";
  } catch (reason) {
    status.value = String(reason);
  }
}

function openNewGroupPrompt() {
  closeConnectionMenu();
  closeGroupMenu();
  groupPromptMode.value = "new";
  groupPromptOld.value = "";
  groupPromptValue.value = "";
  groupPromptOpen.value = true;
}

function openRenameGroupPrompt(name: string) {
  closeGroupMenu();
  if (name === "未分组") return;
  groupPromptMode.value = "rename";
  groupPromptOld.value = name;
  groupPromptValue.value = name;
  groupPromptOpen.value = true;
}

async function confirmGroupPrompt(value: string) {
  const mode = groupPromptMode.value;
  const oldName = groupPromptOld.value;
  groupPromptOpen.value = false;
  groupPromptMode.value = null;
  if (!mode) return;
  try {
    if (mode === "new") {
      await api.saveConnectionGroup({
        name: value,
        color: "",
        order: connectionGroups.value.length,
        collapsed: false,
      });
      await loadConnectionGroups();
      status.value = `已新建分组：${value}`;
    } else {
      await api.renameConnectionGroup(oldName, value);
      await refreshConnections();
      status.value = `已重命名分组：${value}`;
    }
  } catch (reason) {
    status.value = String(reason);
  }
}

async function connectGroup(name: string) {
  const group = groupedConnections.value.find((item) => item.name === name);
  closeGroupMenu();
  if (!group) return;
  let connected = 0;
  for (const connection of group.connections) {
    try {
      if (connection.driver === "redis") {
        await api.redisPing(connection.id);
      } else {
        await api.listDatabases(connection.id);
      }
      connected += 1;
    } catch {
      // Report the successful count; failures stay unconnected.
    }
  }
  status.value = `分组「${name}」已连接 ${connected}/${group.connections.length} 个连接`;
}

async function disconnectGroup(name: string) {
  const group = groupedConnections.value.find((item) => item.name === name);
  closeGroupMenu();
  if (!group) return;
  for (const connection of group.connections) {
    try {
      await api.closeConnection(connection.id);
      resetConnectionTransactions(connection.id);
    } catch {
      // Ignore individual disconnects.
    }
  }
  if (
    selected.value &&
    group.connections.some((connection) => connection.id === selectedId.value)
  ) {
    connectionStatus.value = "idle";
  }
  status.value = `已断开分组「${name}」`;
}

function menuDeleteGroup() {
  const name = groupMenu.value?.group;
  closeGroupMenu();
  if (!name || name === "未分组") return;
  askConfirm({
    title: "删除分组",
    message: `确定删除分组「${name}」？`,
    detail: "只会删除分组，组内连接会移到「未分组」，不会删除连接。",
    confirmText: "删除分组",
    action: async () => {
      try {
        await api.deleteConnectionGroup(name);
        await refreshConnections();
        status.value = `已删除分组：${name}`;
      } catch (reason) {
        status.value = String(reason);
      }
    },
  });
}

function isViewableTable(table: TableInfo): boolean {
  return (
    table.kind === "table" ||
    table.kind === "view" ||
    table.kind === "matview"
  );
}

async function openTableTabAt(
  table: TableInfo,
  mode: "data" | "structure",
  focusedIndex: string | null = null,
  forceNew = false,
) {
  if (!isViewableTable(table)) {
    const labels: Record<string, string> = {
      procedure: "存储过程",
      function: "函数",
      trigger: "触发器",
      event: "事件",
      sequence: "序列",
      index: "索引",
      matview: "物化视图",
    };
    status.value = `${labels[table.kind] ?? "对象"}「${table.name}」暂不支持查看`;
    return;
  }
  const connectionId = selectedId.value;
  if (!connectionId || isRedisConnection.value) return;
  const database = normalizeScopeValue(activeDatabase.value);
  const schema = normalizeScopeValue(activeSchema.value);
  let tab = forceNew
    ? null
    : (tableTabs.value.find(
        (item) =>
          sourceMatches(item, connectionId, database, schema) &&
          item.table.name === table.name &&
          item.table.kind === table.kind,
      ) ?? null);
  if (tab) {
    tab.mode = mode;
    tab.focusedIndex = focusedIndex;
  } else {
    tab = createTableTab(
      table,
      connectionId,
      database,
      schema,
      mode,
      focusedIndex,
    );
    tableTabs.value.push(tab);
    tabOrder.value.push(tab.id);
  }
  await selectTableTab(tab);
}

function openTable(table: TableInfo) {
  void openTableTabAt(table, "data");
}

function openTableInNewTab(table: TableInfo) {
  void openTableTabAt(table, "data", null, true);
}

async function openIndex(table: TableInfo) {
  if (!selectedId.value || isRedisConnection.value) return;
  const connectionId = selectedId.value;
  try {
    const owner = await api.getIndexTable(connectionId, table.name);
    await openTableTabAt(
      { name: owner, kind: "table" },
      "structure",
      table.name,
    );
    status.value = `查看索引：${table.name} · ${owner}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

async function openErDiagram() {
  const connectionId = selectedId.value;
  if (!connectionId || isRedisConnection.value) return;
  const database = normalizeScopeValue(activeDatabase.value);
  const schema = normalizeScopeValue(activeSchema.value);
  let tab = erTabs.value.find((item) =>
    sourceMatches(item, connectionId, database, schema),
  );
  if (!tab) {
    tab = createErTab(connectionId, database, schema);
    erTabs.value.push(tab);
    tabOrder.value.push(tab.id);
  }
  await showErTab(tab);
}

function openTableStructure(table: TableInfo) {
  void openTableTabAt(table, "structure");
}

function setTableMode(mode: "data" | "structure") {
  const tab = activeTableTab.value;
  if (!tab) return;
  tab.mode = mode;
  if (mode !== "structure") {
    tab.focusedIndex = null;
  }
  status.value =
    mode === "data"
      ? `查看表数据：${tab.table.name}`
      : `查看表结构：${tab.table.name}`;
}

function openCreateTable() {
  if (!selectedId.value || isRedisConnection.value) return;
  createOpen.value = true;
}

function openImportData(table: TableInfo) {
  if (!selectedId.value || isRedisConnection.value) return;
  importTable.value = table.name;
  importOpen.value = true;
}

async function handleImportedData(table: string, count: number) {
  await refreshTables();
  status.value = `已导入 ${count} 行到 ${table}`;
}

async function handleTableCreated(table: string, warning?: string) {
  createOpen.value = false;
  await refreshTables();
  await openTableTabAt({ name: table, kind: "table" }, "structure");
  status.value = warning
    ? `已创建表：${table} · ${warning}`
    : `已创建表：${table}`;
}

async function toggleHistory() {
  if (isRedisConnection.value) {
    status.value = "Redis 连接暂不支持 SQL 历史";
    return;
  }
  if (historyOpen.value) {
    historyOpen.value = false;
    return;
  }

  historyOpen.value = true;
  historyLoading.value = true;
  try {
    historyEntries.value = await api.listHistory();
  } catch (reason) {
    status.value = String(reason);
  } finally {
    historyLoading.value = false;
  }
}

function useHistory(entry: HistoryEntry) {
  const tab = activeTab.value;
  if (!tab) return;

  activeTabId.value = tab.id;
  activeWorkspaceTabId.value = tab.id;
  if (connections.value.some((item) => item.id === entry.connectionId)) {
    if (tab.inTransaction) {
      void api.closeTransactionSession(tab.id);
      tab.inTransaction = false;
      tab.transactionBusy = false;
    }
    tab.connectionId = entry.connectionId;
    tab.database = connectionDatabase(entry.connectionId);
    tab.schema = schemaByConnection[entry.connectionId] ?? null;
    selectedConnectionId.value = entry.connectionId;
    activeDatabase.value = tab.database;
    activeSchema.value = tab.schema;
    void refreshTables();
  }
  tab.sql = entry.sql;
  tab.error = null;
  status.value = `已载入历史 SQL：${tab.title}`;
}

function handleClearHistory() {
  const count = historyEntries.value.length;
  askConfirm({
    title: "清空执行历史",
    message: `确定清空全部 ${count} 条执行历史？`,
    detail: "此操作不可撤销。",
    confirmText: "清空",
    action: async () => {
      try {
        await api.clearHistory();
        historyEntries.value = [];
        status.value = "已清空执行历史";
      } catch (reason) {
        status.value = String(reason);
      }
    },
  });
}

function handleSqlSelection(value: string) {
  if (activeTab.value) activeTab.value.selectedSql = value;
}

async function runQuery(selectedSql?: string) {
  const tab = activeTab.value;
  if (!tab || tab.running) return;
  if (!tab.connectionId) {
    status.value = "请先选择一个数据库连接";
    return;
  }
  if (connectionStatus.value !== "connected") {
    status.value = connectionError.value || "连接不可用，请先刷新对象";
    return;
  }

  const tabId = tab.id;
  const connectionId = tab.connectionId;
  const candidate = (selectedSql !== undefined ? selectedSql : tab.selectedSql) ?? "";
  const usingSelection = candidate.trim().length > 0;
  const sqlText = usingSelection ? candidate : tab.sql;
  tab.running = true;
  tab.error = null;
  status.value = usingSelection ? "执行选中 SQL…" : "执行中…";

  try {
    const queryResult = tab.inTransaction
      ? await api.executeTransactionQuery(connectionId, tabId, sqlText)
      : await api.executeQuery(connectionId, sqlText);
    if (!tabs.value.some((item) => item.id === tabId)) return;
    tab.result = queryResult;
    if (activeTabId.value === tabId) {
      status.value = queryResult.isQuery
        ? `${queryResult.rows.length} 行 · ${queryResult.elapsedMs} ms${
            queryResult.truncated ? " · 已截断" : ""
          }${tab.inTransaction ? " · 事务未提交" : ""}`
        : `执行完成 · 影响 ${queryResult.rowsAffected} 行 · ${queryResult.elapsedMs} ms${
            tab.inTransaction ? " · 事务未提交" : ""
          }`;
    }
  } catch (reason) {
    if (!tabs.value.some((item) => item.id === tabId)) return;
    tab.result = null;
    tab.error = String(reason);
    if (activeTabId.value === tabId) {
      status.value = "查询失败";
    }
  } finally {
    tab.running = false;
  }
}

async function beginTransaction() {
  const tab = activeTab.value;
  if (!tab || tab.inTransaction || tab.transactionBusy) return;
  if (!tab.connectionId) {
    status.value = "请先选择一个数据库连接";
    return;
  }
  if (isRedisConnection.value) {
    status.value = "Redis 连接不支持事务";
    return;
  }
  if (connectionStatus.value !== "connected") {
    status.value = connectionError.value || "连接不可用，请先刷新对象";
    return;
  }
  tab.transactionBusy = true;
  try {
    await api.beginTransaction(tab.connectionId, tab.id);
    tab.inTransaction = true;
    status.value = "事务已开始 · 未提交";
  } catch (reason) {
    status.value = String(reason);
  } finally {
    tab.transactionBusy = false;
  }
}

async function commitTransaction() {
  const tab = activeTab.value;
  if (!tab || !tab.inTransaction || tab.transactionBusy || !tab.connectionId) {
    return;
  }
  tab.transactionBusy = true;
  try {
    await api.commitTransaction(tab.connectionId, tab.id);
    status.value = "事务已提交";
  } catch (reason) {
    status.value = String(reason);
  } finally {
    tab.inTransaction = false;
    tab.transactionBusy = false;
  }
}

async function rollbackTransaction() {
  const tab = activeTab.value;
  if (!tab || !tab.inTransaction || tab.transactionBusy || !tab.connectionId) {
    return;
  }
  tab.transactionBusy = true;
  try {
    await api.rollbackTransaction(tab.connectionId, tab.id);
    status.value = "事务已回滚";
  } catch (reason) {
    status.value = String(reason);
  } finally {
    tab.inTransaction = false;
    tab.transactionBusy = false;
  }
}

function clampMenuX(x: number, width: number): number {
  return Math.max(8, Math.min(x, window.innerWidth - width - 8));
}

function clampMenuY(y: number, height: number): number {
  return Math.max(8, Math.min(y, window.innerHeight - height - 8));
}

function editableTarget(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;

  const field = target.closest("input, textarea");
  if (field) {
    if (
      field instanceof HTMLInputElement &&
      !TEXT_INPUT_TYPES.has(field.type)
    ) {
      return null;
    }
    return field as HTMLElement;
  }

  const content = target.closest('[contenteditable="true"]');
  return content instanceof HTMLElement ? content : null;
}

function openAppContextMenu(event: MouseEvent) {
  if (appMenuRef.value?.contains(event.target as Node | null)) return;

  if (event.defaultPrevented) {
    closeAppContextMenu();
    return;
  }

  event.preventDefault();
  const target = editableTarget(event.target);
  const selection = window.getSelection()?.toString() ?? "";

  if (target) {
    if (document.activeElement !== target) {
      target.focus({ preventScroll: true });
    }
    appContextMenu.value = {
      x: clampMenuX(event.clientX, 196),
      y: clampMenuY(event.clientY, 252),
      kind: "edit",
      target,
      canCopy: true,
    };
  } else {
    appContextMenu.value = {
      x: clampMenuX(event.clientX, 208),
      y: clampMenuY(event.clientY, 268),
      kind: "app",
      target: null,
      canCopy: selection.length > 0,
    };
  }

  void nextTick(() => {
    appMenuRef.value
      ?.querySelector<HTMLElement>(".context-menu-item:not(:disabled)")
      ?.focus();
  });
}

function closeAppContextMenu() {
  appContextMenu.value = null;
  connectionMenu.value = null;
  connectionMenuInfoOpen.value = false;
  connectionMenuGroupOpen.value = false;
  groupMenu.value = null;
}

function onWindowMouseDown(event: MouseEvent) {
  const target = event.target as Node | null;
  if (connectionMenu.value && !connectionMenuRef.value?.contains(target)) {
    closeConnectionMenu();
  }
  if (groupMenu.value && !groupMenuRef.value?.contains(target)) {
    closeGroupMenu();
  }
  if (!appContextMenu.value) return;
  if (appMenuRef.value?.contains(target)) return;
  closeAppContextMenu();
}

function cycleQueryTab(direction: number) {
  if (tabOrder.value.length < 2) return;
  const index = tabOrder.value.indexOf(activeWorkspaceTabId.value);
  const next =
    tabOrder.value[
      (index + direction + tabOrder.value.length) % tabOrder.value.length
    ];
  void activateWorkspaceTabById(next);
}

function onWindowKeydown(event: KeyboardEvent) {
  if (event.ctrlKey && !event.metaKey && event.key === "Tab") {
    event.preventDefault();
    cycleQueryTab(event.shiftKey ? -1 : 1);
    return;
  }
  if (event.key === "Escape") {
    if (connectionMenu.value || groupMenu.value) {
      closeConnectionMenu();
      closeGroupMenu();
      return;
    }
    if (appContextMenu.value) {
      const trigger = appContextMenu.value.target;
      closeAppContextMenu();
      trigger?.focus({ preventScroll: true });
    }
    return;
  }
  if (
    appContextMenu.value &&
    (event.key === "ArrowDown" || event.key === "ArrowUp")
  ) {
    const items = Array.from(
      appMenuRef.value?.querySelectorAll<HTMLElement>(
        ".context-menu-item:not(:disabled)",
      ) ?? [],
    );
    if (!items.length) return;
    event.preventDefault();
    const index = items.indexOf(document.activeElement as HTMLElement);
    const next =
      event.key === "ArrowDown"
        ? (index + 1) % items.length
        : (index - 1 + items.length) % items.length;
    items[next].focus();
  }
}

function editAction(command: "cut" | "copy" | "selectAll") {
  const menu = appContextMenu.value;
  if (!menu?.target) return;
  menu.target.focus({ preventScroll: true });
  document.execCommand(command);
  closeAppContextMenu();
}

function editorHistory(redo: boolean) {
  const menu = appContextMenu.value;
  if (!menu?.target) return;

  const editor = menu.target.closest(".cm-editor");
  const content = editor?.querySelector(".cm-content");
  if (content instanceof HTMLElement) {
    content.focus();
    content.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: redo ? "Z" : "z",
        code: "KeyZ",
        metaKey: isMacPlatform,
        ctrlKey: !isMacPlatform,
        shiftKey: redo,
        bubbles: true,
        cancelable: true,
      }),
    );
    closeAppContextMenu();
    return;
  }

  menu.target.focus({ preventScroll: true });
  document.execCommand(redo ? "redo" : "undo");
  closeAppContextMenu();
}

async function pasteFromClipboard() {
  const menu = appContextMenu.value;
  if (!menu?.target) return;

  const target = menu.target;
  closeAppContextMenu();
  target.focus({ preventScroll: true });

  try {
    const text = await readClipboard();
    if (!text) return;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      const start = target.selectionStart ?? target.value.length;
      const end = target.selectionEnd ?? start;
      target.setRangeText(text, start, end, "end");
      target.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      document.execCommand("insertText", false, text);
    }
  } catch {
    document.execCommand("paste");
  }
}

async function copySelectedText() {
  const text = window.getSelection()?.toString() ?? "";
  closeAppContextMenu();
  if (!text) return;

  try {
    await writeClipboard(text);
  } catch {
    document.execCommand("copy");
  }
}

function menuNewConnection() {
  closeAppContextMenu();
  openNewConnection();
}

function menuNewQuery() {
  closeAppContextMenu();
  newQuery();
}

function menuNewTable() {
  closeAppContextMenu();
  openCreateTable();
}

function menuRefresh() {
  closeAppContextMenu();
  if (selectedId.value) void refreshTables();
}

function menuHistory() {
  closeAppContextMenu();
  void toggleHistory();
}

async function bindMenuEvents() {
  const handlers: Array<[string, () => void]> = [
    ["menu:new-connection", openNewConnection],
    ["menu:new-query", newQuery],
    ["menu:new-table", openCreateTable],
    [
      "menu:refresh",
      () => {
        if (selectedId.value) void refreshTables();
      },
    ],
    ["menu:history", () => void toggleHistory()],
  ];

  try {
    for (const [name, handler] of handlers) {
      unlisteners.push(await listen(name, handler));
    }
  } catch (reason) {
    console.error("无法绑定应用菜单事件", reason);
  }
}

onMounted(async () => {
  window.addEventListener("contextmenu", openAppContextMenu);
  window.addEventListener("mousedown", onWindowMouseDown);
  window.addEventListener("keydown", onWindowKeydown);
  window.addEventListener("resize", closeAppContextMenu);
  window.addEventListener("resize", updateTabScroll);
  window.addEventListener("blur", closeAppContextMenu);

  await bindMenuEvents();
  await nextTick();
  updateTabScroll();

  try {
    await refreshConnections();
    if (!connections.value.length) {
      status.value = "还没有连接，先新建一个数据库连接";
    }
  } catch (reason) {
    status.value = String(reason);
  }
});

onBeforeUnmount(() => {
  if (tabsRestored) {
    try {
      window.localStorage.setItem(
        QUERY_TABS_STORAGE_KEY,
        queryTabsSnapshot.value,
      );
    } catch {
      // Ignore storage failures on shutdown.
    }
  }
  window.clearTimeout(tabsSaveTimer);
  for (const unlisten of unlisteners) unlisten();
  window.removeEventListener("contextmenu", openAppContextMenu);
  window.removeEventListener("mousedown", onWindowMouseDown);
  window.removeEventListener("keydown", onWindowKeydown);
  window.removeEventListener("resize", closeAppContextMenu);
  window.removeEventListener("resize", updateTabScroll);
  window.removeEventListener("blur", closeAppContextMenu);
});
</script>

<template>
  <div class="app-shell">
    <header class="titlebar">
      <div class="titlebar-brand" data-tauri-drag-region>
        <span class="brand-mark"><AppMark :size="22" /></span>
        <span class="brand-name">mydb</span>
      </div>

      <div class="titlebar-tools">
        <button
          class="toolbar-action"
          title="新建连接"
          aria-label="新建连接"
          @click="openNewConnection"
        >
          <Icon name="plus" :size="14" />
          <span>新建连接</span>
        </button>
        <button
          class="toolbar-action"
          :disabled="isRedisConnection"
          title="新建查询"
          aria-label="新建查询"
          @click="newQuery"
        >
          <Icon name="code" :size="14" />
          <span>新建查询</span>
        </button>
        <button
          class="toolbar-action"
          :disabled="!selectedId || isRedisConnection || Boolean(selected?.readOnly)"
          title="新建表"
          aria-label="新建表"
          @click="openCreateTable"
        >
          <Icon name="structure" :size="14" />
          <span>新建表</span>
        </button>
        <button
          class="toolbar-action"
          :class="{ active: viewMode === 'er' }"
          :disabled="!selectedId || isRedisConnection"
          title="ER 图"
          aria-label="ER 图"
          @click="openErDiagram"
        >
          <Icon name="diagram" :size="14" />
          <span>ER 图</span>
        </button>
        <button
          class="toolbar-action"
          :disabled="!selectedId || isRedisConnection"
          title="结构同步"
          aria-label="结构同步"
          @click="syncOpen = true"
        >
          <Icon name="structure" :size="14" />
          <span>结构同步</span>
        </button>
        <button
          class="toolbar-action"
          :disabled="!selectedId || isRedisConnection"
          title="数据同步"
          aria-label="数据同步"
          @click="dataSyncOpen = true"
        >
          <Icon name="sync" :size="14" />
          <span>数据同步</span>
        </button>
        <button
          class="toolbar-action"
          :disabled="!selectedId || tablesLoading"
          title="刷新对象列表"
          aria-label="刷新对象列表"
          @click="refreshTables"
        >
          <Icon name="refresh" :size="14" />
          <span>刷新</span>
        </button>
        <button
          class="toolbar-action"
          :class="{ active: historyOpen }"
          :disabled="isRedisConnection"
          title="执行历史"
          aria-label="执行历史"
          @click="toggleHistory"
        >
          <Icon name="history" :size="14" />
          <span>历史</span>
        </button>
      </div>

      <div class="titlebar-context" data-tauri-drag-region>
        <template v-if="selected">
          <span class="titlebar-connection" :title="selected.name">{{ selected.name }}</span>
          <span class="driver-badge">{{ selected.driver }}</span>
          <span class="connection-state" :class="connectionStatus">
            <span class="status-dot" :class="connectionStatus"></span>
            {{ connectionStatusLabel }}
          </span>
        </template>
        <span v-else class="titlebar-muted">未连接</span>
      </div>

      <div class="titlebar-actions">
        <button
          v-if="viewMode === 'sql' && !isRedisConnection"
          class="button primary"
          :disabled="
            !selectedId || running || connectionStatus !== 'connected'
          "
          :title="runDisabledReason || `运行 SQL（${runShortcutLabel}）`"
          aria-label="运行 SQL"
          @click="runQuery()"
        >
          <span v-if="running" class="spinner"></span>
          <Icon v-else name="play" :size="14" />
          {{ running ? "执行中…" : "运行" }}
        </button>
      </div>
    </header>

    <div class="app-body">
      <aside class="sidebar">
        <div class="sidebar-toolbar">
          <span class="sidebar-toolbar-title">连接</span>
          <button
            class="icon-button"
            title="新建分组"
            @click="openNewGroupPrompt"
          >
            <Icon name="structure" />
          </button>
          <button
            class="icon-button"
            title="新建连接"
            @click="openNewConnection"
          >
            <Icon name="plus" />
          </button>
        </div>

        <div class="sidebar-search">
          <Icon name="search" :size="14" />
          <input v-model="connectionSearch" placeholder="搜索连接…" />
          <button
            v-if="connectionSearch"
            class="icon-button search-clear"
            title="清空搜索"
            @click="connectionSearch = ''"
          >
            <Icon name="close" :size="12" />
          </button>
        </div>

        <div class="connection-list">
          <div
            v-for="group in filteredGroups"
            :key="group.name"
            class="connection-group"
          >
            <button
              class="group-header"
              @click="toggleGroup(group.name)"
              @contextmenu.prevent.stop="openGroupMenu($event, group.name)"
            >
              <Icon
                class="group-chevron"
                :name="
                  collapsedGroups[group.name] ? 'chevron-right' : 'chevron-down'
                "
                :size="12"
              />
              <span class="group-name">{{ group.name }}</span>
              <span class="group-count">{{ group.connections.length }}</span>
            </button>

            <div v-show="!collapsedGroups[group.name]">
              <div
                v-for="connection in group.connections"
                :key="connection.id"
                class="connection-item"
                :class="{ active: connection.id === selectedId }"
                role="button"
                tabindex="0"
                @click="selectConnection(connection.id)"
                @keydown.enter="selectConnection(connection.id)"
                @contextmenu.prevent.stop="
                  openConnectionMenu($event, connection)
                "
              >
                <span
                  class="connection-color"
                  :style="{ background: connection.color }"
                ></span>
                <span class="db-icon"><Icon name="database" :size="15" /></span>
                <span class="connection-meta">
                  <span class="connection-name-row">
                    <span class="connection-name" :title="connection.name">{{ connection.name }}</span>
                    <span
                      v-if="connection.readOnly"
                      class="connection-readonly"
                      title="只读连接"
                    >
                      只读
                    </span>
                    <span
                      class="connection-status-dot"
                      :class="
                        connection.id === selectedId
                          ? connectionStatus
                          : 'unknown'
                      "
                    ></span>
                  </span>
                  <span class="connection-sub">
                    {{ connection.driver }} · {{ connectionTarget(connection) }}
                  </span>
                </span>
                <span class="connection-actions">
                  <button
                    class="mini-action"
                    title="编辑连接"
                    @click.stop="openEditConnection(connection)"
                  >
                    编辑
                  </button>
                  <button
                    class="mini-action danger"
                    title="删除连接"
                    @click.stop="handleDelete(connection)"
                  >
                    删除
                  </button>
                </span>
              </div>
            </div>
          </div>

          <div v-if="!connections.length" class="sidebar-empty">
            还没有连接
            <br />
            <button class="link-button" @click="openNewConnection">
              新建一个
            </button>
          </div>
          <div
            v-else-if="!filteredGroups.length"
            class="sidebar-empty"
          >
            没有匹配的连接
          </div>
        </div>

        <div class="sidebar-footer">
          <span class="status-dot" :class="connectionStatus"></span>
          <span>{{ connectionStatusLabel }}</span>
          <span
            v-if="connectionError"
            class="sidebar-footer-error"
            :title="connectionError"
          >
            {{ connectionError }}
          </span>
        </div>
      </aside>

      <section class="main">
        <div v-if="isRedisConnection" class="workspace-tabs">
          <div class="workspace-tab active redis-tab">
            <Icon name="database" :size="14" />
            <span class="tab-label">Redis</span>
            <span v-if="selected" class="hint">{{ selected.name }}</span>
          </div>
        </div>
        <div v-else-if="connections.length" class="workspace-tabs">
          <button
            v-if="tabScrollLeft"
            class="tab-scroll"
            title="向左滚动标签"
            aria-label="向左滚动标签"
            @click="scrollTabs(-1)"
          >
            <Icon name="chevron-left" :size="14" />
          </button>
          <div
            ref="tabStrip"
            class="workspace-tabs-scroll"
            @scroll="updateTabScroll"
            @wheel="onTabWheel"
          >
          <div
            v-for="view in workspaceTabViews"
            :key="view.id"
            class="workspace-tab"
            :class="{
              active: view.id === activeWorkspaceTabId,
              'query-tab': view.kind === 'query',
              'table-tab': view.kind !== 'query',
              'er-tab': view.kind === 'er',
            }"
            role="tab"
            tabindex="0"
            :title="view.title"
            @click="activateWorkspaceTabById(view.id)"
            @keydown.enter="activateWorkspaceTabById(view.id)"
          >
            <Icon :name="view.icon" :size="14" />
            <span class="tab-label">{{ view.label }}</span>
            <span class="tab-source" :title="view.source">
              {{ view.source }}
            </span>
            <span
              v-if="view.running"
              class="tab-running"
              title="执行中"
            ></span>
            <button
              v-if="view.closable"
              class="tab-close"
              :title="`关闭 ${view.label}`"
              @click.stop="closeWorkspaceTab(view.id)"
            >
              <Icon name="close" :size="12" />
            </button>
          </div>
          </div>
          <button
            v-if="tabScrollRight"
            class="tab-scroll"
            title="向右滚动标签"
            aria-label="向右滚动标签"
            @click="scrollTabs(1)"
          >
            <Icon name="chevron-right" :size="14" />
          </button>
          <button
            class="workspace-tab-add"
            title="新建查询"
            @click="newQuery"
          >
            <Icon name="plus" :size="14" />
          </button>
        </div>

        <div
          v-if="selected && connectionStatus === 'failed' && connectionError"
          class="connection-error-bar"
        >
          <Icon name="warning" :size="14" />
          <span class="connection-error-text" :title="connectionError">
            {{ connectionError }}
          </span>
          <button class="button ghost" @click="copyConnectionError">
            复制
          </button>
          <button class="button ghost" @click="refreshTables">重试</button>
          <button class="button ghost" @click="editSelectedConnection">
            编辑连接
          </button>
        </div>

        <div class="workspace">
          <RedisWorkspace
            v-if="isRedisConnection && selectedId"
            :key="selectedId"
            :connection-id="selectedId"
            :connection-name="selected?.name"
            :database="selected?.database"
          />
          <div
            v-else-if="connectionsLoaded && !connections.length"
            class="welcome"
          >
            <div class="welcome-card">
              <span class="welcome-mark">M</span>
              <h1>连接你的第一个数据库</h1>
              <p>
                支持 SQLite、MySQL / MariaDB、PostgreSQL 与 Redis。
                连接配置保存在本机，密码可选择写入系统钥匙串。
              </p>
              <div class="welcome-drivers">
                <span class="welcome-driver">SQLite</span>
                <span class="welcome-driver">MySQL / MariaDB</span>
                <span class="welcome-driver">PostgreSQL</span>
                <span class="welcome-driver">Redis</span>
              </div>
              <div class="welcome-actions">
                <button class="button primary" @click="openNewConnection">
                  <Icon name="plus" :size="14" />
                  新建连接
                </button>
              </div>
            </div>
          </div>
          <template v-else>
            <ObjectBrowser
            :connection-id="selectedId ?? ''"
            :driver="selected?.driver ?? 'sqlite'"
            :tables="tables"
            :databases="databases"
            :active-database="activeDatabase"
            :schemas="schemas"
            :active-schema="activeSchema"
            :database-counts="databaseCounts"
            :loading="tablesLoading"
            :active-table="treeActiveTableName"
            @select="openTable"
            @select-structure="openTableStructure"
            @select-index="openIndex"
            @open-new-tab="openTableInNewTab"
            @import-data="openImportData"
            @select-database="selectDatabase"
            @select-schema="selectSchema"
            @refresh="refreshTables"
            @create="openCreateTable"
            @generate-sql="openGeneratedSql"
          />
          <QueryWorkspace
            v-show="viewMode === 'sql'"
            :sql="sql"
            :tab-id="activeTabId"
            :result="result"
            :running="running"
            :error="queryError"
            :in-transaction="activeTab?.inTransaction ?? false"
            :transaction-busy="activeTab?.transactionBusy ?? false"
            @update:sql="sql = $event"
            @selection="handleSqlSelection"
            @run="runQuery"
            @begin-transaction="beginTransaction"
            @commit-transaction="commitTransaction"
            @rollback-transaction="rollbackTransaction"
          />
          <div
            v-show="viewMode === 'table'"
            class="table-workspace"
          >
            <div class="table-subnav">
              <div class="segmented" role="tablist" aria-label="表视图">
                <button
                  class="table-subtab"
                  :class="{ active: tableMode === 'data' }"
                  role="tab"
                  :aria-selected="tableMode === 'data'"
                  @click="setTableMode('data')"
                >
                  数据
                </button>
                <button
                  class="table-subtab"
                  :class="{ active: tableMode === 'structure' }"
                  role="tab"
                  :aria-selected="tableMode === 'structure'"
                  @click="setTableMode('structure')"
                >
                  结构
                </button>
              </div>
              <span class="table-subnav-meta">
                {{ activeTableScopeLabel }}
              </span>
            </div>
            <div
              v-for="tab in tableTabs"
              :key="tab.id"
              v-show="tab.id === activeWorkspaceTabId"
              class="table-tab-pane"
            >
              <TableDataView
                v-if="tab.mode === 'data'"
                :connection-id="tab.connectionId ?? ''"
                :table="tab.table.name"
                :driver="driverForConnection(tab.connectionId)"
                :database-label="scopeLabelFor(tab)"
              />
              <TableStructureView
                v-else
                :connection-id="tab.connectionId ?? ''"
                :driver="driverForConnection(tab.connectionId)"
                :kind="tab.table.kind"
                :table="tab.table.name"
                :focus-index="tab.focusedIndex"
                :database-label="scopeLabelFor(tab)"
                @refresh="refreshTables"
              />
            </div>
          </div>
          <ErDiagram
            v-for="tab in erTabs"
            :key="`${tab.id}|${tab.connectionId}|${tab.database ?? ''}|${tab.schema ?? ''}|${scopeRevision}`"
            v-show="viewMode === 'er' && tab.id === activeWorkspaceTabId"
            :connection-id="tab.connectionId ?? ''"
            :scope-label="scopeLabelFor(tab)"
            @select="openTable"
          />
          </template>

          <HistoryPanel
            v-if="!isRedisConnection"
            :open="historyOpen"
            :entries="historyEntries"
            :loading="historyLoading"
            @close="historyOpen = false"
            @select="useHistory"
            @clear="handleClearHistory"
          />
        </div>

        <StatusBar
          :status="status"
          :connection="selected"
          :result="isRedisConnection ? null : result"
          :database="activeDatabase ?? selected?.database ?? null"
          :schema="activeSchema"
        />
      </section>
    </div>

    <ConnectionDialog
      :open="dialogOpen"
      :editing="editing"
      :save-error="connectionSaveError"
      :default-group="newConnectionGroup"
      @close="dialogOpen = false"
      @save="handleSave"
    />

    <div
      v-if="connectionMenu"
      ref="connectionMenuRef"
      class="context-menu app-context-menu"
      role="menu"
      :style="{
        left: `${connectionMenu.x}px`,
        top: `${connectionMenu.y}px`,
      }"
      @click.stop
    >
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuRefreshConnection"
      >
        <Icon name="refresh" :size="14" />
        <span>打开 / 刷新对象</span>
      </button>
      <button
        v-if="connectionMenu.connection.driver !== 'redis'"
        class="context-menu-item"
        role="menuitem"
        @click="menuConnectionNewQuery"
      >
        <Icon name="code" :size="14" />
        <span>新建查询</span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuCloseConnection"
      >
        <Icon name="close" :size="14" />
        <span>关闭连接</span>
      </button>

      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuEditConnection"
      >
        <Icon name="edit" :size="14" />
        <span>编辑连接…</span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuDuplicateConnection"
      >
        <Icon name="copy" :size="14" />
        <span>复制连接…</span>
      </button>

      <div
        class="context-submenu-host"
        @mouseenter="connectionMenuInfoOpen = true"
        @mouseleave="connectionMenuInfoOpen = false"
      >
        <button
          class="context-menu-item"
          role="menuitem"
          :aria-expanded="connectionMenuInfoOpen"
          @click.stop="connectionMenuInfoOpen = !connectionMenuInfoOpen"
        >
          <Icon name="copy" :size="14" />
          <span>复制连接信息</span>
          <span class="context-menu-shortcut">▸</span>
        </button>
        <div
          v-if="connectionMenuInfoOpen"
          class="context-menu context-submenu"
          role="menu"
        >
          <button class="context-menu-item" @click="copyConnectionInfo('url')">
            URL
          </button>
          <button
            class="context-menu-item"
            @click="copyConnectionInfo('hostPort')"
          >
            主机:端口
          </button>
          <button class="context-menu-item" @click="copyConnectionInfo('jdbc')">
            JDBC
          </button>
          <button class="context-menu-item" @click="copyConnectionInfo('dsn')">
            DSN
          </button>
          <button class="context-menu-item" @click="copyConnectionInfo('cli')">
            命令行
          </button>
          <div class="context-separator"></div>
          <button
            class="context-menu-item"
            @click="copyConnectionInfo('urlWithPassword')"
          >
            含密码 URL…
          </button>
        </div>
      </div>

      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuToggleReadOnly"
      >
        <Icon name="key" :size="14" />
        <span>{{
          connectionMenu.connection.readOnly ? "取消只读" : "设为只读"
        }}</span>
      </button>

      <div
        class="context-submenu-host"
        @mouseenter="connectionMenuGroupOpen = true"
        @mouseleave="connectionMenuGroupOpen = false"
      >
        <button
          class="context-menu-item"
          role="menuitem"
          :aria-expanded="connectionMenuGroupOpen"
          @click.stop="connectionMenuGroupOpen = !connectionMenuGroupOpen"
        >
          <Icon name="list" :size="14" />
          <span>移动到分组</span>
          <span class="context-menu-shortcut">▸</span>
        </button>
        <div
          v-if="connectionMenuGroupOpen"
          class="context-menu context-submenu"
          role="menu"
        >
          <button
            v-for="group in connectionGroupsForMenu"
            :key="group"
            class="context-menu-item"
            @click="moveConnectionToGroup(group)"
          >
            {{ group }}
          </button>
          <button
            class="context-menu-item"
            @click="moveConnectionToGroup(null)"
          >
            未分组
          </button>
          <div class="context-separator"></div>
          <button class="context-menu-item" @click="openNewGroupPrompt">
            新建分组…
          </button>
        </div>
      </div>

      <div class="context-separator"></div>
      <button
        class="context-menu-item danger"
        role="menuitem"
        @click="menuDeleteConnection"
      >
        <Icon name="trash" :size="14" />
        <span>删除连接…</span>
      </button>
    </div>

    <div
      v-if="groupMenu"
      ref="groupMenuRef"
      class="context-menu app-context-menu"
      role="menu"
      :style="{ left: `${groupMenu.x}px`, top: `${groupMenu.y}px` }"
      @click.stop
    >
      <button
        class="context-menu-item"
        role="menuitem"
        @click="openNewConnectionInGroup(groupMenu.group)"
      >
        <Icon name="plus" :size="14" />
        <span>新建连接</span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="openNewGroupPrompt"
      >
        <Icon name="plus" :size="14" />
        <span>新建分组…</span>
      </button>
      <button
        v-if="groupMenu.group !== '未分组'"
        class="context-menu-item"
        role="menuitem"
        @click="openRenameGroupPrompt(groupMenu.group)"
      >
        <Icon name="edit" :size="14" />
        <span>重命名分组…</span>
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="connectGroup(groupMenu.group)"
      >
        <Icon name="play" :size="14" />
        <span>连接全部</span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="disconnectGroup(groupMenu.group)"
      >
        <Icon name="close" :size="14" />
        <span>断开全部</span>
      </button>
      <template v-if="groupMenu.group !== '未分组'">
        <div class="context-separator"></div>
        <button
          class="context-menu-item danger"
          role="menuitem"
          @click="menuDeleteGroup"
        >
          <Icon name="trash" :size="14" />
          <span>删除分组…</span>
        </button>
      </template>
    </div>

    <PromptDialog
      :open="duplicateOpen"
      title="复制连接"
      :message="`基于「${duplicateTarget?.name ?? ''}」创建新连接：`"
      :value="duplicateTarget ? `${duplicateTarget.name} 副本` : ''"
      confirm-text="复制"
      checkbox-label="复制已保存的密码"
      :checkbox-default="false"
      @confirm="confirmDuplicateConnection"
      @cancel="
        duplicateOpen = false;
        duplicateTarget = null;
      "
    />

    <PromptDialog
      :open="groupPromptOpen"
      :title="groupPromptMode === 'rename' ? '重命名分组' : '新建分组'"
      :message="
        groupPromptMode === 'rename'
          ? `将分组「${groupPromptOld}」重命名为：`
          : '输入新分组名称：'
      "
      :value="groupPromptValue"
      confirm-text="确定"
      @confirm="confirmGroupPrompt"
      @cancel="
        groupPromptOpen = false;
        groupPromptMode = null;
      "
    />

    <ConfirmDialog
      :open="!!confirmRequest"
      :title="confirmRequest?.title ?? ''"
      :message="confirmRequest?.message ?? ''"
      :detail="confirmRequest?.detail ?? null"
      :confirm-text="confirmRequest?.confirmText"
      :danger="confirmRequest?.danger !== false"
      @confirm="runConfirmed"
      @cancel="confirmRequest = null"
    />

    <CreateTableDialog
      :open="createOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      @close="createOpen = false"
      @created="handleTableCreated"
    />

    <ImportDataDialog
      :open="importOpen"
      :connection-id="selectedId ?? ''"
      :tables="tables"
      :initial-table="importTable"
      :read-only="Boolean(selected?.readOnly)"
      @close="importOpen = false"
      @imported="handleImportedData"
    />

    <SchemaSyncDialog
      :open="syncOpen"
      :connections="connections"
      :current-connection-id="selectedId ?? ''"
      @close="syncOpen = false"
    />

    <DataSyncDialog
      :open="dataSyncOpen"
      :connections="connections"
      :current-connection-id="selectedId ?? ''"
      :initial-table="activeTable?.kind === 'table' ? activeTable.name : ''"
      @close="dataSyncOpen = false"
    />

    <div
      v-if="appContextMenu"
      ref="appMenuRef"
      class="context-menu app-context-menu"
      role="menu"
      :style="{
        left: `${appContextMenu.x}px`,
        top: `${appContextMenu.y}px`,
      }"
      @click.stop
    >
      <template v-if="appContextMenu.kind === 'edit'">
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="editorHistory(false)"
        >
          <Icon name="undo" :size="14" />
          <span>撤销</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.undo }}</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="editorHistory(true)"
        >
          <Icon name="redo" :size="14" />
          <span>重做</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.redo }}</span>
        </button>
        <div class="context-separator"></div>
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="editAction('cut')"
        >
          <Icon name="cut" :size="14" />
          <span>剪切</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.cut }}</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="editAction('copy')"
        >
          <Icon name="copy" :size="14" />
          <span>复制</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.copy }}</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="pasteFromClipboard"
        >
          <Icon name="paste" :size="14" />
          <span>粘贴</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.paste }}</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          @mousedown.prevent="editAction('selectAll')"
        >
          <Icon name="select-all" :size="14" />
          <span>全选</span>
          <span class="context-menu-shortcut">{{ menuShortcuts.selectAll }}</span>
        </button>
      </template>

      <template v-else>
        <button
          class="context-menu-item"
          role="menuitem"
          @click="menuNewConnection"
        >
          <Icon name="plus" :size="14" />
          <span>新建连接</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          @click="menuNewQuery"
        >
          <Icon name="code" :size="14" />
          <span>新建查询</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          :disabled="!selectedId || isRedisConnection"
          @click="menuNewTable"
        >
          <Icon name="structure" :size="14" />
          <span>新建表</span>
        </button>
        <div class="context-separator"></div>
        <button
          class="context-menu-item"
          role="menuitem"
          :disabled="!selectedId"
          @click="menuRefresh"
        >
          <Icon name="refresh" :size="14" />
          <span>刷新对象</span>
        </button>
        <button
          class="context-menu-item"
          role="menuitem"
          :disabled="isRedisConnection"
          @click="menuHistory"
        >
          <Icon name="history" :size="14" />
          <span>执行历史</span>
        </button>
        <template v-if="appContextMenu.canCopy">
          <div class="context-separator"></div>
          <button
            class="context-menu-item"
            role="menuitem"
            @click="copySelectedText"
          >
            <Icon name="copy" :size="14" />
            <span>复制</span>
          </button>
        </template>
      </template>
    </div>
  </div>
</template>
