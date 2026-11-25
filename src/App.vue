<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import AppMenuBar from "./components/AppMenuBar.vue";
import CommandPalette from "./components/CommandPalette.vue";
import ConnectionDialog from "./components/ConnectionDialog.vue";
import ConnectionTransferDialog from "./components/ConnectionTransferDialog.vue";
import BackupSettingsDialog from "./components/BackupSettingsDialog.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import CreateTableDialog from "./components/CreateTableDialog.vue";
import CreateDatabaseDialog from "./components/CreateDatabaseDialog.vue";
import CreateObjectDialog from "./components/CreateObjectDialog.vue";
import DangerousSqlDialog from "./components/DangerousSqlDialog.vue";
import DataSyncDialog from "./components/DataSyncDialog.vue";
import DataTransferDialog from "./components/DataTransferDialog.vue";
import DatabaseDocsDialog from "./components/DatabaseDocsDialog.vue";
import DatabaseExportDialog from "./components/DatabaseExportDialog.vue";
import DriverIcon from "./components/DriverIcon.vue";
import ErDiagram from "./components/ErDiagram.vue";
import GenerateDataDialog from "./components/GenerateDataDialog.vue";
import GlobalSearchDialog from "./components/GlobalSearchDialog.vue";
import HistoryPanel from "./components/HistoryPanel.vue";
import Icon from "./components/Icon.vue";
import ImportWizardDialog from "./components/ImportWizardDialog.vue";
import ObjectBrowser from "./components/ObjectBrowser.vue";
import ObjectSourceDialog from "./components/ObjectSourceDialog.vue";
import PromptDialog from "./components/PromptDialog.vue";
import QueryBuilderDialog from "./components/QueryBuilderDialog.vue";
import QueryWorkspace from "./components/QueryWorkspace.vue";
import RedisWorkspace from "./components/RedisWorkspace.vue";
import ResizeHandle from "./components/ResizeHandle.vue";
import SchemaSyncDialog from "./components/SchemaSyncDialog.vue";
import ServerActivityDialog from "./components/ServerActivityDialog.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import ShortcutHelpDialog from "./components/ShortcutHelpDialog.vue";
import SqlFileExecuteDialog from "./components/SqlFileExecuteDialog.vue";
import SqlFormatSettingsDialog from "./components/SqlFormatSettingsDialog.vue";
import SqlSnippetDialog from "./components/SqlSnippetDialog.vue";
import StatusBar from "./components/StatusBar.vue";
import TableDataView from "./components/TableDataView.vue";
import TableStructureView from "./components/TableStructureView.vue";
import WorkspaceStart from "./components/WorkspaceStart.vue";
import * as api from "./lib/api";
import type { ConnectionInfoFormat } from "./lib/api";
import {
  popClosedTab,
  pushClosedTab,
  type ClosedTabSnapshot,
} from "./lib/closed-tabs";
import {
  findDangerousSql,
  loadDangerConfirm,
  saveDangerConfirm,
  type DangerFinding,
} from "./lib/dangerous-sql";
import {
  clearPaletteRecent,
  loadPaletteRecent,
  recordPaletteRecent,
  savePaletteRecent,
  type PaletteItem,
  type PaletteRecentEntry,
} from "./lib/command-palette";
import { readClipboard, writeClipboard } from "./lib/clipboard";
import { splitSqlStatements } from "./lib/sql-statements";
import type {
  AppAction,
  MenuBarItem,
  MenuBarMenu,
} from "./lib/app-actions";
import {
  findDuplicateShortcuts,
} from "./lib/shortcuts";
import { openUrl } from "@tauri-apps/plugin-opener";
import packageInfo from "../package.json";
import {
  DEFAULT_LAYOUT,
  effectiveLayoutWidths,
  loadWorkspaceLayout,
  saveWorkspaceLayout,
  type WorkspaceLayout,
} from "./lib/layout";
import type {
  ConnectionConfig,
  ConnectionGroupInfo,
  Driver,
  ExplainResult,
  HistoryEntry,
  QueryResult,
  ScopeTableColumns,
  TableInfo,
} from "./lib/types";

interface SavePayload {
  config: ConnectionConfig;
  password?: string;
  sshPassword?: string;
  sshKeyPassphrase?: string;
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

const layout = reactive<WorkspaceLayout>(loadWorkspaceLayout());
const viewportWidth = ref(window.innerWidth);
const layoutMenuOpen = ref(false);
const layoutMenuRef = ref<HTMLElement | null>(null);
const layoutButtonRef = ref<HTMLElement | null>(null);
const layoutMenuTop = ref(0);
const layoutMenuRight = ref(0);
let layoutSaveTimer = 0;

const effectiveWidths = computed(() =>
  effectiveLayoutWidths(layout, viewportWidth.value),
);
const layoutStyle = computed<Record<string, string>>(() => ({
  "--sidebar-width": `${
    layout.sidebarCollapsed ? 32 : effectiveWidths.value.sidebar
  }px`,
  "--objects-width": `${
    layout.objectsCollapsed ? 32 : effectiveWidths.value.objects
  }px`,
  "--right-width": `${
    effectiveWidths.value.right || layout.rightWidth
  }px`,
  "--editor-ratio": String(layout.editorRatio),
}));
const layoutShortcuts = computed(() => {
  if (isMacPlatform) {
    return {
      sidebar: "⌘B",
      objects: "⌘⇧B",
      result: "⌘J",
      right: "⌘⇧H",
    };
  }
  return {
    sidebar: "Ctrl+B",
    objects: "Ctrl+Shift+B",
    result: "Ctrl+J",
    right: "Ctrl+Shift+H",
  };
});

function toggleSidebar() {
  layout.sidebarCollapsed = !layout.sidebarCollapsed;
}

function toggleObjects() {
  layout.objectsCollapsed = !layout.objectsCollapsed;
}

function toggleResultPanel() {
  layout.resultCollapsed = !layout.resultCollapsed;
}

function toggleRightPanel() {
  layout.rightOpen = !layout.rightOpen;
}

function resetLayout() {
  Object.assign(layout, DEFAULT_LAYOUT);
  layoutMenuOpen.value = false;
}

function closeLayoutMenu() {
  layoutMenuOpen.value = false;
}

function toggleLayoutMenu() {
  if (layoutMenuOpen.value) {
    closeLayoutMenu();
    return;
  }
  const rect = layoutButtonRef.value?.getBoundingClientRect();
  if (!rect) return;
  layoutMenuTop.value = rect.bottom + 6;
  layoutMenuRight.value = Math.max(8, window.innerWidth - rect.right);
  layoutMenuOpen.value = true;
}

function updateViewportWidth() {
  viewportWidth.value = window.innerWidth;
  if (layoutMenuOpen.value) closeLayoutMenu();
}

const unlisteners: UnlistenFn[] = [];
const appContextMenu = ref<AppContextMenu | null>(null);
const appMenuRef = ref<HTMLElement | null>(null);
const tabMenu = ref<{ x: number; y: number; tabId: string } | null>(null);
const tabMenuRef = ref<HTMLElement | null>(null);

interface QueryTab {
  id: string;
  title: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  sql: string;
  /// Set when the tab is bound to an on-disk SQL file (session only).
  filePath: string | null;
  /// SQL snapshot taken when the file was opened or last saved.
  savedSql: string | null;
  /// Currently selected editor text; run uses it instead of the whole tab.
  selectedSql: string;
  result: QueryResult | null;
  error: string | null;
  running: boolean;
  operationId: string | null;
  stopping: boolean;
  stopNote: string | null;
  explainResult: ExplainResult | null;
  explainError: string | null;
  explainLoading: boolean;
  showExplain: boolean;
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
const redisDatabaseByConnection = reactive<Record<string, string>>({});
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
const tableDataRefs = new Map<
  string,
  {
    hasPendingChanges: boolean;
    submitPendingChanges: () => Promise<boolean>;
    discardPendingChanges: () => void;
  }
>();

function setTableDataRef(tabId: string, instance: unknown) {
  if (instance) {
    tableDataRefs.set(
      tabId,
      instance as {
        hasPendingChanges: boolean;
        submitPendingChanges: () => Promise<boolean>;
        discardPendingChanges: () => void;
      },
    );
  } else {
    tableDataRefs.delete(tabId);
  }
}
const tabStrip = ref<HTMLElement | null>(null);
const tabScrollLeft = ref(false);
const tabScrollRight = ref(false);
const syncOpen = ref(false);
const dataSyncOpen = ref(false);
const dataTransferOpen = ref(false);
const createObjectOpen = ref(false);
const connectionTransferOpen = ref(false);
const globalSearchOpen = ref(false);
const createObjectKind = ref<"view" | "procedure" | "function" | "trigger">(
  "view",
);
const databaseExportOpen = ref(false);
const sqlScriptOpen = ref(false);
const sqlScriptInitialPath = ref<string | null>(null);
const snippetOpen = ref(false);
const snippetInitialBody = ref("");
const formatSettingsOpen = ref(false);
const settingsOpen = ref(false);
const queryBuilderOpen = ref(false);
const databaseDocsOpen = ref(false);
const dangerousConfirmEnabled = ref(loadDangerConfirm());
const dangerousSqlRequest = ref<{
  tabId: string;
  sql: string;
  usingSelection: boolean;
  finding: DangerFinding;
} | null>(null);
const dangerousTab = computed(
  () =>
    tabs.value.find(
      (tab) => tab.id === dangerousSqlRequest.value?.tabId,
    ) ?? null,
);
const dangerousConnectionName = computed(
  () =>
    connectionFor(dangerousTab.value?.connectionId ?? "")?.name ?? "",
);
const dangerousScope = computed(
  () =>
    [dangerousTab.value?.database, dangerousTab.value?.schema]
      .filter(Boolean)
      .join(" / ") || "当前库 / 模式",
);
const objectSource = ref<{ kind: string; name: string } | null>(null);
const createNamespaceOpen = ref(false);
const createNamespaceMode = ref<"database" | "schema">("database");
const createNamespaceRef = ref<InstanceType<typeof CreateDatabaseDialog> | null>(
  null,
);
const generateTarget = ref<{
  connectionId: string;
  table: string;
  tableKind: string;
} | null>(null);
const backupSettingsOpen = ref(false);
const serverActivityOpen = ref(false);
const databaseExportScope = ref<{
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
  database?: string;
  schema?: string | null;
} | null>(null);

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
    filePath: null,
    savedSql: null,
    selectedSql: "",
    result: null,
    error: null,
    running: false,
    operationId: null,
    stopping: false,
    stopNote: null,
    explainResult: null,
    explainError: null,
    explainLoading: false,
    showExplain: false,
    inTransaction: false,
    transactionBusy: false,
  };
}

const QUERY_TABS_STORAGE_KEY = "mydb.queryTabs.v1";
let tabsRestored = false;
let pendingRestoredActiveTabId: string | null = null;
let tabsSaveTimer = 0;

/// Standalone query windows are opened from a query tab ("在新窗口打开").
/// They load the same app shell but keep their own tabs, skip localStorage
/// persistence, and hide the connection sidebar / object tree.
const queryWindowParams = new URLSearchParams(window.location.search);
const standaloneQueryWindow = queryWindowParams.get("window") === "query";
const standaloneConnectionId = standaloneQueryWindow
  ? queryWindowParams.get("connectionId")
  : null;
const standaloneDatabase = standaloneQueryWindow
  ? queryWindowParams.get("database")
  : null;
const standaloneSchema = standaloneQueryWindow
  ? queryWindowParams.get("schema")
  : null;

const firstTab = createQueryTab(
  standaloneConnectionId,
  standaloneDatabase,
  standaloneSchema,
);
if (standaloneQueryWindow) {
  const initialSql = queryWindowParams.get("sql");
  const initialTitle = queryWindowParams.get("title");
  if (initialSql) firstTab.sql = initialSql;
  if (initialTitle) firstTab.title = initialTitle;
  if (firstTab.connectionId) {
    if (firstTab.database) {
      databaseByConnection[firstTab.connectionId] = firstTab.database;
    }
    if (firstTab.schema) {
      schemaByConnection[firstTab.connectionId] = firstTab.schema;
    }
  }
}
const tabs = ref<QueryTab[]>([firstTab]);
const activeTabId = ref(firstTab.id);
/// Session-only LIFO stack for Shift+Mod+T / 文件 → 重新打开关闭的标签.
const closedTabs = ref<ClosedTabSnapshot[]>([]);
const selectedConnectionId = ref<string | null>(firstTab.connectionId);
const activeTab = computed(
  () =>
    tabs.value.find((tab) => tab.id === activeTabId.value) ?? tabs.value[0],
);
const queryWorkspaceRef = ref<InstanceType<typeof QueryWorkspace> | null>(
  null,
);
const splitQueryWorkspaceRef =
  ref<InstanceType<typeof QueryWorkspace> | null>(null);

const completionTables = ref<ScopeTableColumns[]>([]);
const completionRevision = ref(0);
const completionCache = new Map<string, ScopeTableColumns[]>();
let completionRequest = 0;

const completionSource = computed(() => {
  const tab = activeTab.value;
  if (!tab?.connectionId) return null;
  const connection = connectionFor(tab.connectionId);
  if (!connection || connection.driver === "redis") return null;
  const database = normalizeScopeValue(tab.database);
  const schema = normalizeScopeValue(tab.schema);
  return {
    connectionId: tab.connectionId,
    database,
    schema,
    key: `${tab.connectionId}|${database ?? ""}|${schema ?? ""}`,
  };
});
const completionKey = computed(() => completionSource.value?.key ?? "");

async function loadCompletion() {
  const source = completionSource.value;
  if (!source) {
    completionTables.value = [];
    return;
  }
  const key = source.key;
  const cached = completionCache.get(key);
  if (cached) {
    completionTables.value = cached;
    return;
  }

  const request = ++completionRequest;
  try {
    const result = await api.listScopeColumns(source.connectionId);
    if (request !== completionRequest) return;
    if (completionCache.size > 12) {
      completionCache.clear();
    }
    completionCache.set(key, result);
    completionTables.value = result;
  } catch {
    if (request !== completionRequest) return;
    // Autocompletion is best-effort: keep keyword completion available.
    completionTables.value = [];
  }
}

watch(
  [completionKey, completionRevision],
  ([, revision], previous) => {
    if (revision !== previous?.[1]) {
      completionCache.clear();
    }
    void loadCompletion();
  },
  { immediate: true },
);

const tabOrder = ref<string[]>([firstTab.id]);
const activeWorkspaceTabId = ref<string>(firstTab.id);
const PINNED_TABS_KEY = "mydb.pinnedTabs.v1";

function loadPinnedTabs(): string[] {
  try {
    const raw = window.localStorage.getItem(PINNED_TABS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === "string")
      : [];
  } catch {
    return [];
  }
}

const pinnedTabIds = ref<string[]>(loadPinnedTabs());
const orderedTabIds = computed(() => {
  const present = new Set(tabOrder.value);
  const pinned = pinnedTabIds.value.filter((id) => present.has(id));
  const pinnedSet = new Set(pinned);
  return [
    ...pinned,
    ...tabOrder.value.filter((id) => !pinnedSet.has(id)),
  ];
});
const dragTabId = ref("");
const dragOverTabId = ref("");
const tabOverflowOpen = ref(false);
const tabOverflowRef = ref<HTMLElement | null>(null);
const tabOverflowPosition = ref({ left: 0, top: 0 });

function toggleTabOverflow(event: MouseEvent) {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  tabOverflowPosition.value = {
    left: Math.max(8, Math.min(rect.right - 280, window.innerWidth - 296)),
    top: rect.bottom + 6,
  };
  tabOverflowOpen.value = !tabOverflowOpen.value;
}

watch(
  pinnedTabIds,
  (value) => {
    try {
      window.localStorage.setItem(PINNED_TABS_KEY, JSON.stringify(value));
    } catch {
      // Pin persistence is best-effort.
    }
  },
);

watch(tabOrder, (order) => {
  const present = new Set(order);
  const next = pinnedTabIds.value.filter((id) => present.has(id));
  if (next.length !== pinnedTabIds.value.length) {
    pinnedTabIds.value = next;
  }
});

const activeTableTab = computed(
  () =>
    tableTabs.value.find((tab) => tab.id === activeWorkspaceTabId.value) ??
    null,
);
const activeErTab = computed(
  () =>
    erTabs.value.find((tab) => tab.id === activeWorkspaceTabId.value) ?? null,
);
const splitTabId = ref<string | null>(null);
const splitWidth = ref(520);
const splitView = computed(() => {
  const id = splitTabId.value;
  if (!id) return null;
  const query = tabs.value.find((tab) => tab.id === id);
  if (query) return { kind: "query" as const, query };
  const table = tableTabs.value.find((tab) => tab.id === id);
  if (table) return { kind: "table" as const, table };
  const er = erTabs.value.find((tab) => tab.id === id);
  if (er) return { kind: "er" as const, er };
  return null;
});

function setSplitExplainView(value: boolean) {
  if (splitView.value?.kind === "query") {
    splitView.value.query.showExplain = value;
  }
}

function runSplitQuery(selectedSql?: string) {
  const view = splitView.value;
  if (view?.kind === "query") {
    void runQueryOnTab(view.query, selectedSql);
  }
}

function stopSplitQuery() {
  const view = splitView.value;
  if (view?.kind === "query") void stopQueryOnTab(view.query);
}

function explainSplitQuery() {
  const view = splitView.value;
  if (view?.kind === "query") {
    void explainQueryOnTab(
      view.query,
      splitQueryWorkspaceRef.value?.currentStatement() ?? "",
    );
  }
}

function updateSplitSql(value: string) {
  const view = splitView.value;
  if (view?.kind === "query") view.query.sql = value;
}

function updateSplitSelection(value: string) {
  const view = splitView.value;
  if (view?.kind === "query") view.query.selectedSql = value;
}

function beginSplitTransaction() {
  const view = splitView.value;
  if (view?.kind === "query") void beginTransaction(view.query);
}

function commitSplitTransaction() {
  const view = splitView.value;
  if (view?.kind === "query") void commitTransaction(view.query);
}

function rollbackSplitTransaction() {
  const view = splitView.value;
  if (view?.kind === "query") void rollbackTransaction(view.query);
}

watch(
  () => splitTabId.value,
  (id) => {
    if (!id) return;
    if (
      !tabs.value.some((tab) => tab.id === id) &&
      !tableTabs.value.some((tab) => tab.id === id) &&
      !erTabs.value.some((tab) => tab.id === id)
    ) {
      splitTabId.value = null;
    }
  },
);

watch(
  () => [
    tabs.value.map((tab) => tab.id).join("|"),
    tableTabs.value.map((tab) => tab.id).join("|"),
    erTabs.value.map((tab) => tab.id).join("|"),
  ],
  () => {
    const id = splitTabId.value;
    if (
      id &&
      !tabs.value.some((tab) => tab.id === id) &&
      !tableTabs.value.some((tab) => tab.id === id) &&
      !erTabs.value.some((tab) => tab.id === id)
    ) {
      splitTabId.value = null;
    }
  },
);

// The left workspace is the active tab; showing that same tab on the right
// would mirror one editor. Close the split when the user activates it.
watch(activeWorkspaceTabId, (id) => {
  if (id && splitTabId.value === id) {
    splitTabId.value = null;
  }
});
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
const viewMode = computed<"sql" | "table" | "er" | "empty">(() => {
  if (activeTableTab.value) return "table";
  if (activeErTab.value) return "er";
  if (tabs.value.some((tab) => tab.id === activeWorkspaceTabId.value)) {
    return "sql";
  }
  return "empty";
});

const statusActiveKind = computed<"query" | "table" | "er" | null>(() => {
  if (activeTableTab.value) return "table";
  if (activeErTab.value) return "er";
  if (tabs.value.some((tab) => tab.id === activeWorkspaceTabId.value)) {
    return "query";
  }
  return null;
});

const statusActiveLabel = computed(() => {
  const table = activeTableTab.value;
  if (table) {
    return `${table.mode === "structure" ? "结构" : "表"} · ${
      table.table.name
    } · ${tabSourceLabel(table)}`;
  }
  const er = activeErTab.value;
  if (er) return `ER · ${tabSourceLabel(er)}`;
  const query = tabs.value.find(
    (tab) => tab.id === activeWorkspaceTabId.value,
  );
  if (query) return `查询 · ${query.title} · ${queryTabSource(query)}`;
  if (selected.value) {
    return connectionChipScope.value
      ? `${selected.value.name} · ${connectionChipScope.value}`
      : selected.value.name;
  }
  return "未连接";
});

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
const aboutOpen = ref(false);
const shortcutHelpOpen = ref(false);
const domModalOpen = ref(false);
const paletteOpen = ref(false);
const paletteRecent = ref<PaletteRecentEntry[]>(loadPaletteRecent());
let palettePreviousFocus: HTMLElement | null = null;
let modalObserver: MutationObserver | null = null;

function syncDomModalOpen() {
  domModalOpen.value = Boolean(document.querySelector(".modal-backdrop"));
}
const editing = ref<ConnectionConfig | null>(null);
const connectionSaveError = ref("");
const connectionsLoaded = ref(false);
const GROUP_COLLAPSE_STORAGE_KEY = "mydb.connectionGroupsCollapsed.v1";

function loadCollapsedGroups(): Record<string, boolean> {
  try {
    const raw = window.localStorage.getItem(GROUP_COLLAPSE_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== "object") return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(
        (entry): entry is [string, boolean] => typeof entry[1] === "boolean",
      ),
    );
  } catch {
    return {};
  }
}

const collapsedGroups = ref<Record<string, boolean>>(loadCollapsedGroups());
const SIDEBAR_FILTER_STORAGE_KEY = "mydb.sidebarOpenOnly.v1";
function loadSidebarOpenOnly(): boolean {
  try {
    return window.localStorage.getItem(SIDEBAR_FILTER_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}
const sidebarOpenOnly = ref(loadSidebarOpenOnly());
watch(sidebarOpenOnly, (value) => {
  try {
    window.localStorage.setItem(
      SIDEBAR_FILTER_STORAGE_KEY,
      value ? "1" : "0",
    );
  } catch {
    // Persisting the filter is best-effort.
  }
});
const locatedConnectionId = ref<string | null>(null);
let locatedConnectionTimer = 0;

watch(
  collapsedGroups,
  (value) => {
    try {
      window.localStorage.setItem(
        GROUP_COLLAPSE_STORAGE_KEY,
        JSON.stringify(value),
      );
    } catch {
      // Persisting collapse state is best-effort.
    }
  },
  { deep: true },
);
const createOpen = ref(false);
const importOpen = ref(false);
const importTable = ref("");
const historyOpen = computed<boolean>({
  get: () => layout.rightOpen,
  set: (value) => {
    layout.rightOpen = value;
  },
});
const historyLoading = ref(false);
const historyLoaded = ref(false);
const historyEntries = ref<HistoryEntry[]>([]);
const connectionStatus = ref<ConnectionStatus>("idle");
const connectionError = ref("");

interface ConfirmRequest {
  title: string;
  message: string;
  detail?: string | null;
  confirmText?: string;
  cancelText?: string;
  extraText?: string;
  danger?: boolean;
  action: () => void | Promise<void>;
  extraAction?: () => void | Promise<void>;
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

async function runConfirmExtra() {
  const request = confirmRequest.value;
  confirmRequest.value = null;
  if (request?.extraAction) await request.extraAction();
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
/// Whether the Redis key browser / console is currently open. Selecting a
/// Redis connection opens it; closing the Redis tab returns to the start page.
const redisWorkspaceOpen = ref(true);

watch(selectedId, (id) => {
  if (id && connectionFor(id)?.driver === "redis") {
    redisWorkspaceOpen.value = true;
  }
});

const redisDatabase = computed(() => {
  const connectionId = selectedId.value;
  if (!connectionId || !isRedisConnection.value) return null;
  return (
    redisDatabaseByConnection[connectionId] ??
    normalizeScopeValue(connectionFor(connectionId)?.database) ??
    "0"
  );
});

function handleRedisDatabaseChange(database: string) {
  const connectionId = selectedId.value;
  if (!connectionId) return;
  redisDatabaseByConnection[connectionId] = database;
  status.value = `Redis 数据库：db${database.padStart(2, "0")}`;
}

const connectionChipScope = computed(() => {
  const connection = selected.value;
  if (!connection) return "";
  if (connection.driver === "sqlite") return "main";
  if (connection.driver === "redis") {
    const database = redisDatabase.value ?? "0";
    return /^\d+$/.test(database)
      ? `db${database.padStart(2, "0")}`
      : `db${database}`;
  }
  const database = activeDatabase.value ?? connection.database;
  if (connection.driver === "postgres" && activeSchema.value) {
    return `${database} · ${activeSchema.value}`;
  }
  return database ?? "";
});

const connectionChipTitle = computed(() => {
  const connection = selected.value;
  if (!connection) return "未连接";
  const parts = [connection.name, connection.driver];
  if (connectionChipScope.value) parts.push(connectionChipScope.value);
  if (connection.readOnly) parts.push("只读");
  return parts.join(" · ");
});

function openConnectionChipMenu(event: MouseEvent) {
  if (!selected.value) return;
  openConnectionMenu(event, selected.value);
}

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

const objectLocateScope = computed(() => {
  const tab = activeErTab.value;
  if (!tab) return false;
  return sourceMatches(
    tab,
    selectedId.value,
    activeDatabase.value,
    activeSchema.value,
  );
});

const objectLocateReason = computed(() => {
  const tab = activeTableTab.value;
  if (!tab) {
    if (activeErTab.value) {
      return objectLocateScope.value
        ? ""
        : "当前对象树范围与活动标签不一致";
    }
    return "当前标签不是表或视图";
  }
  if (
    !sourceMatches(
      tab,
      selectedId.value,
      activeDatabase.value,
      activeSchema.value,
    )
  ) {
    return "当前对象树范围与活动标签不一致";
  }
  return "";
});

const isMacPlatform = navigator.platform.toUpperCase().includes("MAC");
const commandKeyLabel = isMacPlatform ? "⌘" : "Ctrl+";
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

const openConnectionIds = computed(() => {
  const ids = new Set<string>();
  for (const tab of tabs.value) {
    if (tab.connectionId) ids.add(tab.connectionId);
  }
  for (const tab of tableTabs.value) {
    if (tab.connectionId) ids.add(tab.connectionId);
  }
  for (const tab of erTabs.value) {
    if (tab.connectionId) ids.add(tab.connectionId);
  }
  if (
    redisWorkspaceOpen.value &&
    isRedisConnection.value &&
    selectedId.value
  ) {
    ids.add(selectedId.value);
  }
  return ids;
});

const activeConnectionId = computed(() => {
  const table = activeTableTab.value;
  if (table?.connectionId) return table.connectionId;
  const er = activeErTab.value;
  if (er?.connectionId) return er.connectionId;
  const query = tabs.value.find(
    (tab) => tab.id === activeWorkspaceTabId.value,
  );
  if (query?.connectionId) return query.connectionId;
  if (
    redisWorkspaceOpen.value &&
    isRedisConnection.value &&
    selectedId.value
  ) {
    return selectedId.value;
  }
  return null;
});

const filteredGroups = computed<ConnectionGroup[]>(() => {
  let groups = groupedConnections.value;
  if (sidebarOpenOnly.value) {
    groups = groups
      .map((group) => ({
        ...group,
        connections: group.connections.filter((connection) =>
          openConnectionIds.value.has(connection.id),
        ),
      }))
      .filter((group) => group.connections.length > 0);
  }
  const query = connectionSearch.value.trim().toLowerCase();
  if (!query) return groups;

  return groups
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

function fileNameFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function isQueryTabDirty(tab: QueryTab): boolean {
  return Boolean(tab.filePath) && tab.savedSql !== tab.sql;
}

function suggestedSqlFileName(tab: QueryTab): string {
  const base = (tab.filePath ? fileNameFromPath(tab.filePath) : tab.title).trim();
  const name = base || "query";
  return /\.sql$/i.test(name) ? name : `${name}.sql`;
}

function queryTabTitle(tab: QueryTab): string {
  const file = tab.filePath ? ` · ${tab.filePath}` : "";
  const dirty = isQueryTabDirty(tab) ? " · 未保存" : "";
  return `${tab.title}${dirty} · ${tabSourceLabel(tab)}${file}`;
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
  dirty: boolean;
  closable: boolean;
  pinned: boolean;
}

const workspaceTabViews = computed<WorkspaceTabView[]>(() => {
  const views: WorkspaceTabView[] = [];
  for (const id of orderedTabIds.value) {
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
        dirty: isQueryTabDirty(query),
        closable: true,
        pinned: pinnedTabIds.value.includes(id),
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
        dirty: false,
        closable: true,
        pinned: pinnedTabIds.value.includes(id),
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
        dirty: false,
        closable: true,
        pinned: pinnedTabIds.value.includes(id),
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
  if (standaloneQueryWindow) {
    tabsRestored = true;
    return;
  }
  tabsRestored = true;
  try {
    const raw = window.localStorage.getItem(QUERY_TABS_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as {
      tabs?: StoredQueryTab[];
      activeTabId?: string;
    };
    if (!Array.isArray(parsed.tabs)) return;
    if (!parsed.tabs.length) {
      tabs.value = [];
      tabOrder.value = [];
      activeTabId.value = "";
      activeWorkspaceTabId.value = "";
      return;
    }
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

const queryTabsSnapshot = computed(() => {
  const order = new Map(
    orderedTabIds.value.map((id, index) => [id, index]),
  );
  return JSON.stringify({
    tabs: [...tabs.value]
      .sort(
        (left, right) =>
          (order.get(left.id) ?? Number.MAX_SAFE_INTEGER) -
          (order.get(right.id) ?? Number.MAX_SAFE_INTEGER),
      )
      .map((tab) => ({
        id: tab.id,
        title: tab.title,
        connectionId: tab.connectionId,
        database: tab.database,
        schema: tab.schema,
        sql: tab.sql,
      })),
    activeTabId: activeTabId.value,
  });
});

watch(queryTabsSnapshot, (value) => {
  if (!tabsRestored || standaloneQueryWindow) return;
  window.clearTimeout(tabsSaveTimer);
  tabsSaveTimer = window.setTimeout(() => {
    try {
      window.localStorage.setItem(QUERY_TABS_STORAGE_KEY, value);
    } catch {
      // Local storage may be unavailable; tabs still work in memory.
    }
  }, 250);
});

watch(
  layout,
  () => {
    if (standaloneQueryWindow) return;
    window.clearTimeout(layoutSaveTimer);
    layoutSaveTimer = window.setTimeout(() => {
      saveWorkspaceLayout(layout);
    }, 200);
  },
  { deep: true },
);

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
  completionRevision.value += 1;
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
  for (const id of Object.keys(redisDatabaseByConnection)) {
    if (!ids.has(id)) {
      delete redisDatabaseByConnection[id];
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
    if (standaloneQueryWindow && tabs.value.length === 1) {
      const tab = tabs.value[0];
      if (tab.connectionId === selectedId.value) {
        tab.database = activeDatabase.value;
        tab.schema = activeSchema.value;
      }
    }
    const connection = connectionFor(selectedId.value);
    if (connection && connection.driver !== "redis" && tabs.value.length) {
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
  if (connection?.driver === "redis") {
    redisWorkspaceOpen.value = true;
  }
  if (connection && connection.driver !== "redis" && tabs.value.length) {
    ensureTabForSource(true);
  }
}

function openRedisWorkspace() {
  redisWorkspaceOpen.value = true;
  status.value = "已打开 Redis 控制台";
}

function closeRedisWorkspace() {
  if (!isRedisConnection.value) return;
  const connectionId = selectedId.value;
  if (connectionId) {
    closedTabs.value = pushClosedTab(closedTabs.value, {
      kind: "redis",
      tabId: `redis:${connectionId}`,
      connectionId,
    });
  }
  redisWorkspaceOpen.value = false;
  status.value = "已关闭 Redis 控制台";
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
  try {
    if (await api.connectionHasQueryWindow(connectionId)) {
      status.value =
        "该连接有独立查询窗口，请先关闭查询窗口再切换数据库 / 模式";
      return;
    }
  } catch {
    // The guard is best-effort; the backend still rejects unsafe switches.
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
      if (tabs.value.length) ensureTabForSource(true);
      return;
    }
    await refreshTables();
    if (tabs.value.length) ensureTabForSource(true);
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
  try {
    if (await api.connectionHasQueryWindow(connectionId)) {
      status.value =
        "该连接有独立查询窗口，请先关闭查询窗口再切换数据库 / 模式";
      return;
    }
  } catch {
    // The guard is best-effort; the backend still rejects unsafe switches.
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
    if (tabs.value.length) ensureTabForSource(true);
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

function toggleSidebarOpenOnly() {
  sidebarOpenOnly.value = !sidebarOpenOnly.value;
  status.value = sidebarOpenOnly.value
    ? "只显示有打开标签的连接"
    : "显示全部连接";
}

function collapseAllGroups() {
  const next: Record<string, boolean> = { ...collapsedGroups.value };
  for (const group of groupedConnections.value) {
    next[group.name] = true;
  }
  collapsedGroups.value = next;
  status.value = "已折叠全部连接分组";
}

function locateActiveConnection() {
  const id = activeConnectionId.value;
  if (!id) {
    status.value = "当前没有可定位的连接";
    return;
  }
  const connection = connectionFor(id);
  if (!connection) {
    status.value = "活动标签的来源连接已删除";
    return;
  }
  if (sidebarOpenOnly.value && !openConnectionIds.value.has(id)) {
    sidebarOpenOnly.value = false;
  }
  if (connectionSearch.value) connectionSearch.value = "";
  const group = connection.group.trim() || "未分组";
  collapsedGroups.value = { ...collapsedGroups.value, [group]: false };
  locatedConnectionId.value = id;
  window.clearTimeout(locatedConnectionTimer);
  locatedConnectionTimer = window.setTimeout(() => {
    if (locatedConnectionId.value === id) {
      locatedConnectionId.value = null;
    }
  }, 1600);
  void nextTick(() => {
    const element = document.querySelector<HTMLElement>(
      `.connection-item[data-connection-id="${id}"]`,
    );
    element?.scrollIntoView({ block: "nearest" });
  });
  status.value = `已定位连接：${connection.name}`;
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

async function openSqlFile() {
  if (isRedisConnection.value) {
    status.value = "Redis 连接请使用命令控制台";
    return;
  }
  try {
    const selectedPath = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "SQL 文件", extensions: ["sql"] }],
    });
    if (!selectedPath || Array.isArray(selectedPath)) return;
    const contents = await api.readTextFile(selectedPath);
    const source = activeTab.value;
    const connectionId = source?.connectionId ?? selectedId.value;
    const database =
      source?.database ?? connectionDatabase(connectionId);
    const schema =
      source?.schema ??
      (connectionId ? schemaByConnection[connectionId] ?? null : null);
    const tab = createQueryTab(connectionId, database, schema);
    tab.title = fileNameFromPath(selectedPath);
    tab.sql = contents;
    tab.filePath = selectedPath;
    tab.savedSql = contents;
    tabs.value.push(tab);
    tabOrder.value.push(tab.id);
    activeTabId.value = tab.id;
    activeWorkspaceTabId.value = tab.id;
    await showQueryTab(tab.id);
    status.value = `已打开：${tab.title}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

async function saveQueryTab(
  tab: QueryTab | undefined,
  saveAs = false,
): Promise<boolean> {
  if (!tab) {
    status.value = "当前没有可保存的查询标签";
    return false;
  }
  let path: string | null = saveAs ? null : tab.filePath;
  if (!path) {
    try {
      const chosen = await saveDialog({
        defaultPath: tab.filePath ?? suggestedSqlFileName(tab),
        filters: [{ name: "SQL 文件", extensions: ["sql"] }],
      });
      if (!chosen || Array.isArray(chosen)) return false;
      path = chosen;
    } catch (reason) {
      status.value = String(reason);
      return false;
    }
  }
  const contents = tab.sql;
  try {
    await api.writeTextFile(path, contents);
    tab.filePath = path;
    tab.savedSql = contents;
    tab.title = fileNameFromPath(path);
    status.value = `已保存：${tab.title}`;
    return true;
  } catch (reason) {
    status.value = String(reason);
    return false;
  }
}

function saveActiveSql() {
  if (viewMode.value !== "sql") {
    status.value = "请先切换到查询标签";
    return;
  }
  void saveQueryTab(activeTab.value);
}

function saveActiveSqlAs() {
  if (viewMode.value !== "sql") {
    status.value = "请先切换到查询标签";
    return;
  }
  void saveQueryTab(activeTab.value, true);
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

async function selectTableTab(tab: TableTab): Promise<boolean> {
  if (!tableTabs.value.some((item) => item.id === tab.id)) return false;
  if (!(await activateTabSource(tab))) return false;
  if (!tableTabs.value.some((item) => item.id === tab.id)) return false;
  activeWorkspaceTabId.value = tab.id;
  status.value =
    tab.mode === "structure"
      ? `查看表结构：${tab.table.name}`
      : `查看表数据：${tab.table.name}`;
  return true;
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
  const tab = tabs.value.find((item) => item.id === id);
  if (!tab) return;
  if (isQueryTabDirty(tab)) {
    askConfirm({
      title: "保存修改？",
      message: `「${tab.title}」有未保存的修改。关闭前要保存吗？`,
      detail: tab.filePath,
      cancelText: "取消",
      extraText: "不保存",
      extraAction: () => closeQueryTabNow(id),
      confirmText: "保存",
      danger: false,
      action: async () => {
        if (await saveQueryTab(tab)) closeQueryTabNow(id);
      },
    });
    return;
  }
  closeQueryTabNow(id);
}

function closeQueryTabNow(id: string) {
  const index = tabs.value.findIndex((tab) => tab.id === id);
  if (index < 0) return;

  const closing = tabs.value[index];
  if (closing.running && closing.operationId) {
    void api.cancelQuery(closing.operationId);
  }
  if (closing.inTransaction || closing.transactionBusy) {
    void api.closeTransactionSession(id);
  }
  closedTabs.value = pushClosedTab(closedTabs.value, {
    kind: "query",
    tabId: closing.id,
    title: closing.title,
    connectionId: closing.connectionId,
    database: closing.database,
    schema: closing.schema,
    sql: closing.sql,
    filePath: closing.filePath,
    savedSql: closing.savedSql,
    hadTransaction: closing.inTransaction || closing.transactionBusy,
  });

  const wasActive = activeWorkspaceTabId.value === id;
  const neighbor = neighborTabId(id);
  tabs.value.splice(index, 1);
  removeTabFromOrder(id);
  if (activeTabId.value === id) {
    activeTabId.value = tabs.value[0]?.id ?? "";
  }
  if (!wasActive) return;

  if (neighbor) {
    activeWorkspaceTabId.value = neighbor;
    if (tabs.value.some((tab) => tab.id === neighbor)) {
      activeTabId.value = neighbor;
    }
    void activateWorkspaceTabById(neighbor);
    return;
  }
  const replacement = tabs.value[0];
  if (replacement) {
    activeTabId.value = replacement.id;
    activeWorkspaceTabId.value = replacement.id;
    return;
  }
  activeWorkspaceTabId.value = "";
  activeTabId.value = "";
  status.value = "已关闭全部标签";
}

function closeTableTab(id: string) {
  const tab = tableTabs.value.find((item) => item.id === id);
  if (!tab) return;
  const instance = tableDataRefs.get(id);
  if (instance?.hasPendingChanges) {
    askConfirm({
      title: "存在待提交变更",
      message: `表「${tab.table.name}」有未提交的行变更，关闭标签前需要处理。`,
      detail: "提交失败会整体回滚并保留变更；放弃后本次修改不可恢复。",
      confirmText: "提交并关闭",
      extraText: "放弃并关闭",
      action: async () => {
        if (await instance.submitPendingChanges()) closeTableTab(id);
      },
      extraAction: () => {
        instance.discardPendingChanges();
        closeTableTab(id);
      },
    });
    return;
  }
  closedTabs.value = pushClosedTab(closedTabs.value, {
    kind: "table",
    tabId: tab.id,
    connectionId: tab.connectionId,
    database: tab.database,
    schema: tab.schema,
    table: { ...tab.table },
    mode: tab.mode,
    focusedIndex: tab.focusedIndex,
  });
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
  closedTabs.value = pushClosedTab(closedTabs.value, {
    kind: "er",
    tabId: tab.id,
    connectionId: tab.connectionId,
    database: tab.database,
    schema: tab.schema,
  });
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
  if (id && tabs.value.some((tab) => tab.id === id)) {
    activeWorkspaceTabId.value = id;
    void showQueryTab(id);
    return;
  }
  const next = tabOrder.value[0];
  if (next) {
    void activateWorkspaceTabById(next);
    return;
  }
  activeWorkspaceTabId.value = "";
  activeTabId.value = "";
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
      payload.sshKeyPassphrase,
    );
    resetConnectionTransactions(saved.id);
    dialogOpen.value = false;
    delete databaseByConnection[saved.id];
    delete redisDatabaseByConnection[saved.id];
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
        delete redisDatabaseByConnection[connection.id];
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

async function closeConnectionById(connection: ConnectionConfig) {
  const running = tabs.value.filter(
    (tab) =>
      tab.connectionId === connection.id && tab.running && tab.operationId,
  );
  if (running.length) {
    status.value = `正在取消 ${running.length} 个运行中的查询…`;
    await Promise.allSettled(
      running.map((tab) => api.cancelQuery(tab.operationId!)),
    );
  }
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

async function menuCloseConnection() {
  const connection = connectionMenu.value?.connection;
  closeConnectionMenu();
  if (!connection) return;
  await closeConnectionById(connection);
}

async function closeSelectedConnection() {
  if (!selected.value) {
    status.value = "请先选择连接";
    return;
  }
  await closeConnectionById(selected.value);
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

async function openDatabaseExportAt(scope?: {
  kind: "database" | "schema";
  name: string;
}) {
  const connection = selected.value;
  if (!connection || connection.driver === "redis") {
    status.value = "请选择 SQLite / MySQL / PostgreSQL 连接";
    return;
  }
  if (scope) {
    if (scope.kind === "database" && scope.name !== activeDatabase.value) {
      await selectDatabase(scope.name);
      if (activeDatabase.value !== scope.name) return;
    }
    if (scope.kind === "schema" && scope.name !== activeSchema.value) {
      await selectSchema(scope.name);
      if (activeSchema.value !== scope.name) return;
    }
  }
  if (connectionStatus.value !== "connected") {
    status.value = connectionError.value || "连接不可用，请先刷新对象";
    return;
  }
  const database =
    normalizeScopeValue(activeDatabase.value) ??
    normalizeScopeValue(connection.database);
  const schema = normalizeScopeValue(activeSchema.value);
  databaseExportScope.value = {
    connectionId: connection.id,
    driver: connection.driver,
    scopeLabel:
      connection.driver === "sqlite"
        ? "main"
        : [database, connection.driver === "postgres" ? schema : null]
            .filter(Boolean)
            .join(" · ") || connection.database || connection.name,
    database: database ?? undefined,
    schema,
  };
  databaseExportOpen.value = true;
}

function handleDatabaseExported(count: number) {
  status.value = `已导出数据库 SQL · ${count} 个对象`;
}

async function handleConnectionsImported(count: number) {
  await refreshConnections();
  status.value = `已导入 ${count} 个连接；密码需要在连接编辑框中重新填写`;
}

function handleSchemaSyncApplied() {
  void refreshTables();
  status.value = "结构同步已应用到目标，已刷新对象树";
}

function handleTransferred() {
  void refreshTables();
  status.value = "数据传输完成";
}

function openSqlScriptDialog() {
  const connection = selected.value;
  if (!connection || connection.driver === "redis") {
    status.value = "请选择 SQLite / MySQL / PostgreSQL 连接";
    return;
  }
  sqlScriptInitialPath.value = null;
  sqlScriptOpen.value = true;
}

function openSnippetLibrary() {
  if (!canEditSql.value) {
    status.value = "请先切换到查询标签";
    return;
  }
  snippetInitialBody.value = "";
  snippetOpen.value = true;
}

function openSnippetSave(text: string) {
  if (!canEditSql.value) return;
  if (!text.trim()) {
    status.value = "请先选中 SQL 或把光标放到一条语句中";
    return;
  }
  snippetInitialBody.value = text;
  snippetOpen.value = true;
}

function insertSnippetText(text: string) {
  queryWorkspaceRef.value?.insertSnippet(text);
  status.value = "已插入 SQL 片段（未执行）";
}

function openFormatSettings() {
  formatSettingsOpen.value = true;
}

function handleFormatSettingsSaved() {
  status.value = "格式化设置已保存";
}

async function restoreBackupFile(path: string, connectionId: string) {
  const connection = connections.value.find((item) => item.id === connectionId);
  if (!connection) {
    status.value = "备份对应的连接已删除，无法恢复";
    return;
  }
  backupSettingsOpen.value = false;
  if (selectedId.value !== connectionId) {
    await selectConnection(connectionId);
  }
  sqlScriptInitialPath.value = path;
  sqlScriptOpen.value = true;
}

const serverActivityReason = computed(() => {
  if (!selected.value) return "请先选择连接";
  if (selected.value.driver !== "mysql" && selected.value.driver !== "postgres") {
    return "仅 MySQL / PostgreSQL 支持服务器活动";
  }
  return "";
});

function openServerActivity() {
  if (serverActivityReason.value) return;
  serverActivityOpen.value = true;
}

function openGlobalSearch() {
  if (!selected.value || isRedisConnection.value) {
    status.value = "请选择 SQLite / MySQL / PostgreSQL 连接";
    return;
  }
  globalSearchOpen.value = true;
}

async function chooseGlobalSearchHit(hit: {
  database: string | null;
  schema: string | null;
  name: string;
  kind: string;
  column: string | null;
  owner: string | null;
}) {
  globalSearchOpen.value = false;
  if (hit.database && hit.database !== activeDatabase.value) {
    await selectDatabase(hit.database);
    if (activeDatabase.value !== hit.database) return;
  }
  if (hit.schema && hit.schema !== activeSchema.value) {
    await selectSchema(hit.schema);
    if (activeSchema.value !== hit.schema) return;
  }
  if (hit.kind === "column" && hit.owner) {
    const known = tables.value.find((table) => table.name === hit.owner);
    await openTableTabAt(
      { name: hit.owner, kind: known?.kind ?? "table" },
      "data",
    );
    status.value = `已打开字段 ${hit.name} 所属表 ${hit.owner}`;
    return;
  }
  if (["table", "view", "matview"].includes(hit.kind)) {
    await openTableTabAt(
      { name: hit.name, kind: hit.kind },
      hit.kind === "table" ? "data" : "structure",
    );
    status.value = `已打开 ${hit.kind}：${hit.name}`;
    return;
  }
  status.value = `已找到 ${hit.kind}：${hit.name}（请在对象树中查看）`;
}

function openObjectDefinition(table: { kind: string; name: string }) {
  objectSource.value = { kind: table.kind, name: table.name };
}

function handleObjectDefinitionSaved() {
  void refreshTables();
  status.value = "对象定义已保存";
}

function openGenerateData(payload: {
  connectionId: string;
  table: string;
  tableKind: string;
}) {
  if (!payload.table || payload.tableKind !== "table") {
    status.value = "只有表支持生成测试数据";
    return;
  }
  generateTarget.value = payload;
}

function handleGeneratedData(count: number) {
  void refreshTables();
  status.value = `已生成并写入 ${count} 行测试数据`;
}

function openCreateNamespace(kind: "database" | "schema") {
  if (!selected.value || selected.value.driver === "redis") return;
  if (selected.value.readOnly) {
    status.value = "当前连接为只读模式，无法创建数据库或模式";
    return;
  }
  if (kind === "schema" && selected.value.driver !== "postgres") {
    status.value = "仅 PostgreSQL 支持创建模式";
    return;
  }
  createNamespaceMode.value = kind;
  createNamespaceOpen.value = true;
}

async function handleCreateNamespace(payload: {
  name: string;
  charset: string | null;
  collation: string | null;
  switchAfter: boolean;
}) {
  const connection = selected.value;
  const connectionId = selectedId.value;
  if (!connection || !connectionId) return;
  try {
    if (createNamespaceMode.value === "schema") {
      await api.createSchema(connectionId, payload.name);
      createNamespaceOpen.value = false;
      await refreshTables();
      await selectSchema(payload.name);
      status.value = `已创建模式：${payload.name}`;
      return;
    }
    await api.createDatabase(
      connectionId,
      payload.name,
      payload.charset,
      payload.collation,
    );
    createNamespaceOpen.value = false;
    await refreshTables();
    if (connection.driver === "mysql") {
      if (payload.switchAfter) {
        await selectDatabase(payload.name);
      }
      status.value = `已创建数据库：${payload.name}${
        payload.switchAfter ? "（已切换）" : ""
      }`;
    } else {
      status.value = `已创建数据库：${payload.name}；需要切换或重新连接后才能浏览`;
    }
  } catch (reason) {
    createNamespaceRef.value?.setError(String(reason));
  }
}

function dropNamespace(kind: "database" | "schema", name: string) {
  const connection = selected.value;
  const connectionId = selectedId.value;
  if (!connection || !connectionId) return;
  if (connection.readOnly) {
    status.value = "当前连接为只读模式，无法删除数据库或模式";
    return;
  }
  const current =
    kind === "database" ? name === activeDatabase.value : name === activeSchema.value;
  askConfirm({
    title: kind === "database" ? "删除数据库" : "删除模式",
    message: `确定删除${kind === "database" ? "数据库" : "模式"}「${name}」？此操作不可恢复。`,
    detail: current
      ? "该对象正在使用；删除后会自动切换到安全默认对象。不会执行 CASCADE。"
      : "不会删除其中的连接、表或数据文件；非空对象可能被数据库拒绝。",
    confirmText: "删除",
    danger: true,
    action: async () => {
      try {
        if (kind === "database") {
          await api.dropDatabase(connectionId, name);
        } else {
          await api.dropSchema(connectionId, name);
        }
        await refreshTables();
        status.value = `已删除${kind === "database" ? "数据库" : "模式"}：${name}`;
      } catch (reason) {
        status.value = `删除失败: ${reason}`;
      }
    },
  });
}

function openCreateObject(
  kind: "view" | "procedure" | "function" | "trigger",
) {
  if (!selected.value || isRedisConnection.value) return;
  if (selected.value.readOnly) {
    status.value = "当前连接为只读模式，无法创建对象";
    return;
  }
  if (
    selected.value.driver === "sqlite" &&
    (kind === "procedure" || kind === "function")
  ) {
    status.value = "SQLite 不支持存储过程或函数";
    return;
  }
  createObjectKind.value = kind;
  createObjectOpen.value = true;
}

async function handleObjectCreated(payload: {
  kind: "view" | "procedure" | "function" | "trigger";
  name: string;
}) {
  createObjectOpen.value = false;
  await refreshTables();
  if (payload.kind === "view") {
    await openTableTabAt(
      { name: payload.name, kind: "view" },
      "structure",
    );
  }
  status.value = `已创建${payload.kind === "view" ? "视图" : payload.kind === "procedure" ? "存储过程" : payload.kind === "function" ? "函数" : "触发器"}：${payload.name}`;
}

async function handleSqlScriptExecuted(summary: {
  failed: number;
  cancelled: boolean;
  rolledBack: boolean;
}) {
  await refreshTables();
  await loadHistory();
  if (summary.cancelled) {
    status.value = "SQL 脚本执行已取消";
  } else if (summary.failed > 0) {
    status.value = summary.rolledBack
      ? `SQL 脚本执行失败，已回滚 · ${summary.failed} 条失败`
      : `SQL 脚本执行完成 · ${summary.failed} 条失败`;
  } else {
    status.value = "SQL 脚本执行完成";
  }
}

function openTableStructure(table: TableInfo) {
  void openTableTabAt(table, "structure");
}

function setTableMode(mode: "data" | "structure") {
  const tab = activeTableTab.value;
  if (!tab) return;
  const instance = tableDataRefs.get(tab.id);
  if (mode === "structure" && instance?.hasPendingChanges) {
    askConfirm({
      title: "存在待提交变更",
      message: `表「${tab.table.name}」有未提交的行变更，切换视图前需要处理。`,
      detail: "提交失败会整体回滚并保留变更；放弃后本次修改不可恢复。",
      confirmText: "提交并切换",
      extraText: "放弃并切换",
      action: async () => {
        if (await instance.submitPendingChanges()) setTableMode(mode);
      },
      extraAction: () => {
        instance.discardPendingChanges();
        setTableMode(mode);
      },
    });
    return;
  }
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

async function loadHistory(force = false) {
  if (historyLoading.value || (historyLoaded.value && !force)) return;
  historyLoading.value = true;
  try {
    historyEntries.value = await api.listHistory();
    historyLoaded.value = true;
  } catch (reason) {
    status.value = String(reason);
  } finally {
    historyLoading.value = false;
  }
}

watch(viewMode, (mode) => {
  if (mode === "empty" && connectionsLoaded.value && !isRedisConnection.value) {
    void loadHistory(true);
  }
});

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
  await loadHistory(true);
}

function useHistory(entry: HistoryEntry) {
  let tab = activeTab.value;
  if (!tab) {
    newQuery();
    tab = activeTab.value;
  }
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
        historyLoaded.value = true;
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
  if (tab) await runQueryOnTab(tab, selectedSql);
}

async function runQueryOnTab(tab: QueryTab, selectedSql?: string) {
  if (!tab || tab.running) return;
  if (!tab.connectionId) {
    status.value = "请先选择一个数据库连接";
    return;
  }
  if (tab.id === activeTabId.value && connectionStatus.value !== "connected") {
    status.value = connectionError.value || "连接不可用，请先刷新对象";
    return;
  }

  const candidate = (selectedSql !== undefined ? selectedSql : tab.selectedSql) ?? "";
  const usingSelection = candidate.trim().length > 0;
  const sqlText = usingSelection ? candidate : tab.sql;
  if (dangerousConfirmEnabled.value) {
    const finding = findDangerousSql(sqlText);
    if (finding) {
      dangerousSqlRequest.value = {
        tabId: tab.id,
        sql: sqlText,
        usingSelection,
        finding,
      };
      status.value = "危险 SQL：请确认后执行";
      return;
    }
  }
  await executeSqlOnTab(tab, sqlText, usingSelection);
}

async function executeSqlOnTab(
  tab: QueryTab,
  sqlText: string,
  usingSelection: boolean,
) {
  const tabId = tab.id;
  const connectionId = tab.connectionId;
  if (!connectionId) return;
  const operationId = newOperationId();
  tab.running = true;
  tab.operationId = operationId;
  tab.stopping = false;
  tab.stopNote = null;
  tab.error = null;
  tab.explainResult = null;
  tab.explainError = null;
  tab.explainLoading = false;
  tab.showExplain = false;
  status.value = usingSelection ? "执行选中 SQL…" : "执行中…";

  try {
    const queryResult = tab.inTransaction
      ? await api.executeTransactionQuery(
          connectionId,
          tabId,
          sqlText,
          1000,
          operationId,
        )
      : await api.executeQuery(connectionId, sqlText, 1000, operationId);
    if (!tabs.value.some((item) => item.id === tabId)) return;
    tab.result = queryResult;
    if (activeTabId.value === tabId || splitTabId.value === tabId) {
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
    if (activeTabId.value === tabId || splitTabId.value === tabId) {
      status.value = tab.error.startsWith("查询超时")
        ? "查询已超时"
        : tab.error === "已取消"
          ? "已取消"
          : "查询失败";
    }
  } finally {
    tab.running = false;
    tab.stopping = false;
    tab.operationId = null;
    tab.stopNote = null;
  }
}

function confirmDangerousSql() {
  const request = dangerousSqlRequest.value;
  dangerousSqlRequest.value = null;
  if (!request) return;
  const tab = tabs.value.find((item) => item.id === request.tabId);
  if (!tab) return;
  void executeSqlOnTab(tab, request.sql, request.usingSelection);
}

function cancelDangerousSql() {
  dangerousSqlRequest.value = null;
  status.value = "已取消执行危险 SQL";
}

function toggleDangerousConfirm() {
  dangerousConfirmEnabled.value = !dangerousConfirmEnabled.value;
  saveDangerConfirm(dangerousConfirmEnabled.value);
  status.value = dangerousConfirmEnabled.value
    ? "已开启危险 SQL 二次确认"
    : "已关闭危险 SQL 二次确认";
}

function newOperationId(): string {
  return (
    globalThis.crypto?.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(16).slice(2)}`
  );
}

async function stopActiveQuery() {
  const tab = activeTab.value;
  if (tab) await stopQueryOnTab(tab);
}

async function stopQueryOnTab(tab: QueryTab) {
  if (!tab.running || !tab.operationId || tab.stopping) return;
  const operationId = tab.operationId;
  tab.stopping = true;
  tab.stopNote = null;
  status.value = "停止中…";
  try {
    const result = await api.cancelQuery(operationId);
    if (
      activeTabId.value === tab.id ||
      splitTabId.value === tab.id
    ) {
      if (result.cancelled) {
        status.value = result.message;
      } else {
        tab.stopNote = result.message;
        status.value = result.message;
      }
    }
  } catch (reason) {
    if (
      activeTabId.value === tab.id ||
      splitTabId.value === tab.id
    ) {
      status.value = `停止失败: ${reason}`;
    }
  }
}

function setExplainView(value: boolean) {
  if (activeTab.value) activeTab.value.showExplain = value;
}

async function explainQuery() {
  const tab = activeTab.value;
  if (!tab) return;
  await explainQueryOnTab(
    tab,
    queryWorkspaceRef.value?.currentStatement() ?? "",
  );
}

async function explainQueryOnTab(tab: QueryTab, rawStatement: string) {
  if (!tab || tab.running) return;
  if (!tab.connectionId) {
    status.value = "请先选择一个数据库连接";
    return;
  }
  if (tab.id === activeTabId.value && connectionStatus.value !== "connected") {
    status.value = connectionError.value || "连接不可用，请先刷新对象";
    return;
  }
  const statement = rawStatement.trim();
  if (!statement) {
    status.value = "请把光标放到要解释的语句，或先选中一条 SQL";
    return;
  }
  if (splitSqlStatements(statement).length > 1) {
    status.value = "一次只能解释一条语句，请先选中单条 SQL";
    return;
  }
  const driver = driverForConnection(tab.connectionId);
  if (!driver) return;
  const tabId = tab.id;
  tab.explainLoading = true;
  tab.running = true;
  tab.explainError = null;
  tab.showExplain = true;
  status.value = "正在生成执行计划…";
  try {
    const plan = await api.explainQuery(tab.connectionId, statement);
    if (!tabs.value.some((item) => item.id === tabId)) return;
    tab.explainResult = plan;
    status.value = `执行计划 · ${plan.driver} · ${plan.format} · ${plan.result.rows.length} 行`;
  } catch (reason) {
    if (!tabs.value.some((item) => item.id === tabId)) return;
    tab.explainError = String(reason);
    status.value = "生成执行计划失败";
  } finally {
    if (tabs.value.some((item) => item.id === tabId)) {
      tab.explainLoading = false;
      tab.running = false;
    }
  }
}

async function beginTransaction(target?: QueryTab) {
  const tab = target ?? activeTab.value;
  if (!tab || tab.inTransaction || tab.transactionBusy) return;
  if (!tab.connectionId) {
    status.value = "请先选择一个数据库连接";
    return;
  }
  if (connectionFor(tab.connectionId)?.driver === "redis") {
    status.value = "Redis 连接不支持事务";
    return;
  }
  if (tab.id === activeTabId.value && connectionStatus.value !== "connected") {
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

async function commitTransaction(target?: QueryTab) {
  const tab = target ?? activeTab.value;
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

async function rollbackTransaction(target?: QueryTab) {
  const tab = target ?? activeTab.value;
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

const lastEditable = ref<HTMLElement | null>(null);

function trackEditableFocus(event: FocusEvent) {
  const target = event.target as HTMLElement | null;
  if (target && editableTarget(target)) {
    lastEditable.value = target;
  }
}

function currentEditableTarget(): HTMLElement | null {
  const remembered =
    lastEditable.value?.isConnected ? lastEditable.value : null;
  if (remembered) return remembered;
  const active = document.activeElement;
  return active instanceof HTMLElement ? editableTarget(active) : null;
}

const hasEditableTarget = computed(() =>
  Boolean(currentEditableTarget()),
);

async function pasteIntoEditable(target: HTMLElement) {
  try {
    const text = await readClipboard();
    if (!text) return;
    target.focus({ preventScroll: true });
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement
    ) {
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

function runMenuEdit(
  command: "undo" | "redo" | "cut" | "copy" | "paste" | "selectAll",
) {
  const target = currentEditableTarget();
  if (!target) return;
  target.focus({ preventScroll: true });

  if (command === "undo" || command === "redo") {
    const editor = target.closest(".cm-editor");
    const content = editor?.querySelector(".cm-content");
    if (content instanceof HTMLElement) {
      content.focus();
      content.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: command === "redo" ? "Z" : "z",
          code: "KeyZ",
          metaKey: isMacPlatform,
          ctrlKey: !isMacPlatform,
          shiftKey: command === "redo",
          bubbles: true,
          cancelable: true,
        }),
      );
      return;
    }
    document.execCommand(command);
    return;
  }

  if (command === "paste") {
    void pasteIntoEditable(target);
    return;
  }

  document.execCommand(command === "selectAll" ? "selectAll" : command);
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
  if (layoutMenuOpen.value && !layoutMenuRef.value?.contains(target)) {
    closeLayoutMenu();
  }
  if (connectionMenu.value && !connectionMenuRef.value?.contains(target)) {
    closeConnectionMenu();
  }
  if (groupMenu.value && !groupMenuRef.value?.contains(target)) {
    closeGroupMenu();
  }
  if (tabMenu.value && !tabMenuRef.value?.contains(target)) {
    closeTabMenu();
  }
  if (
    tabOverflowOpen.value &&
    !tabOverflowRef.value?.contains(target)
  ) {
    tabOverflowOpen.value = false;
  }
  if (!appContextMenu.value) return;
  if (appMenuRef.value?.contains(target)) return;
  closeAppContextMenu();
}

function cycleQueryTab(direction: number) {
  const order = orderedTabIds.value;
  if (order.length < 2) return;
  const index = order.indexOf(activeWorkspaceTabId.value);
  const next = order[(index + direction + order.length) % order.length];
  void activateWorkspaceTabById(next);
}

function onWindowKeydown(event: KeyboardEvent) {
  if (paletteOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closePalette(true);
    }
    return;
  }
  if (domModalOpen.value) {
    if (event.ctrlKey && event.key === "Tab") {
      event.preventDefault();
      return;
    }
    if (event.key === "Tab" && !event.defaultPrevented) {
      const backdrop = Array.from(
        document.querySelectorAll<HTMLElement>(".modal-backdrop"),
      ).pop();
      if (backdrop) {
        const items = Array.from(
          backdrop.querySelectorAll<HTMLElement>(
            'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
          ),
        ).filter((item) => item.offsetParent !== null);
        if (items.length) {
          const first = items[0];
          const last = items[items.length - 1];
          const active = document.activeElement as HTMLElement | null;
          if (!active || !backdrop.contains(active)) {
            event.preventDefault();
            first.focus();
            return;
          }
          if (event.shiftKey && active === first) {
            event.preventDefault();
            last.focus();
            return;
          }
          if (!event.shiftKey && active === last) {
            event.preventDefault();
            first.focus();
            return;
          }
        }
      }
    }
  }
  if (event.ctrlKey && !event.metaKey && event.key === "Tab") {
    event.preventDefault();
    cycleQueryTab(event.shiftKey ? -1 : 1);
    return;
  }
  const primaryModifier = isMacPlatform ? event.metaKey : event.ctrlKey;
  if (
    primaryModifier &&
    !event.altKey &&
    !event.shiftKey &&
    event.key === "," &&
    !event.repeat
  ) {
    if (!hasOpenModal.value) {
      event.preventDefault();
      settingsOpen.value = true;
    }
    return;
  }
  if (
    primaryModifier &&
    event.shiftKey &&
    !event.altKey &&
    event.key.toLowerCase() === "f" &&
    !event.repeat
  ) {
    if (!hasOpenModal.value) {
      event.preventDefault();
      runSqlEditorCommand("format-sql");
    }
    return;
  }
  if (
    primaryModifier &&
    event.shiftKey &&
    !event.altKey &&
    event.key.toLowerCase() === "k" &&
    !event.repeat
  ) {
    if (!hasOpenModal.value) {
      event.preventDefault();
      openSnippetLibrary();
    }
    return;
  }
  if (
    primaryModifier &&
    !event.altKey &&
    !event.shiftKey &&
    event.key === "." &&
    !event.repeat
  ) {
    if (!hasOpenModal.value) {
      event.preventDefault();
      void stopActiveQuery();
    }
    return;
  }
  const modifier = isMacPlatform ? event.metaKey : event.ctrlKey;
  // On macOS these are native menu accelerators; other platforms fall back
  // to the window-level shortcuts.
  if (!isMacPlatform && modifier && !event.altKey && !event.repeat) {
    const key = event.key.toLowerCase();
    if (hasOpenModal.value) {
      if (
        ["n", "t", "w", "r", "y", "m", "q", "o", "s"].includes(
          key,
        )
      ) {
        event.preventDefault();
      }
      return;
    }
    if (key === "n" && !event.shiftKey) {
      event.preventDefault();
      openNewConnection();
      return;
    }
    if (key === "t") {
      event.preventDefault();
      if (event.shiftKey) {
        void reopenClosedTab();
      } else {
        newQuery();
      }
      return;
    }
    if (key === "w") {
      event.preventDefault();
      if (event.shiftKey) {
        closeWindow();
      } else {
        closeActiveWorkspaceTab();
      }
      return;
    }
    if (key === "r" && !event.shiftKey) {
      event.preventDefault();
      if (selectedId.value) void refreshTables();
      return;
    }
    if (key === "y" && !event.shiftKey) {
      event.preventDefault();
      void toggleHistory();
      return;
    }
    if (key === "m" && !event.shiftKey) {
      event.preventDefault();
      void getCurrentWindow().minimize();
      return;
    }
    if (key === "q" && !event.shiftKey) {
      event.preventDefault();
      quitApp();
      return;
    }
    if (key === "o" && !event.shiftKey) {
      event.preventDefault();
      void openSqlFile();
      return;
    }
    if (key === "s") {
      event.preventDefault();
      if (event.shiftKey) {
        saveActiveSqlAs();
      } else {
        saveActiveSql();
      }
      return;
    }
    if (key === "b" && event.shiftKey) {
      event.preventDefault();
      toggleObjects();
      return;
    }
    if (key === "b") {
      event.preventDefault();
      toggleSidebar();
      return;
    }
    if (key === "j" && !event.shiftKey) {
      event.preventDefault();
      toggleResultPanel();
      return;
    }
    if (key === "h" && event.shiftKey) {
      event.preventDefault();
      toggleRightPanel();
      return;
    }
  }
  if (event.key === "Escape") {
    if (tabMenu.value) {
      closeTabMenu();
      return;
    }
    if (tabOverflowOpen.value) {
      tabOverflowOpen.value = false;
      return;
    }
    if (shortcutHelpOpen.value) {
      shortcutHelpOpen.value = false;
      return;
    }
    if (aboutOpen.value) {
      aboutOpen.value = false;
      return;
    }
    if (layoutMenuOpen.value) {
      closeLayoutMenu();
      return;
    }
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

async function reopenClosedTab() {
  const entry = popClosedTab(closedTabs.value);
  if (!entry) {
    status.value = "没有最近关闭的标签";
    return;
  }
  closedTabs.value = entry.rest;
  const snapshot = entry.snapshot;
  const connection = snapshot.connectionId
    ? connectionFor(snapshot.connectionId)
    : null;
  if (snapshot.connectionId && !connection) {
    status.value = "来源连接已删除，无法恢复该标签";
    return;
  }
  if (snapshot.kind === "redis") {
    if (!connection || connection.driver !== "redis") {
      status.value = "来源连接已删除，无法恢复 Redis 控制台";
      return;
    }
    selectedId.value = snapshot.connectionId;
    redisWorkspaceOpen.value = true;
    status.value = "已恢复 Redis 控制台";
    return;
  }
  if (snapshot.kind === "query") {
    if (snapshot.connectionId && connection?.driver === "redis") {
      status.value = "来源连接不是 SQL 连接，无法恢复该标签";
      return;
    }
    const tab = createQueryTab(
      snapshot.connectionId,
      snapshot.database,
      snapshot.schema,
    );
    tab.title = snapshot.title;
    tab.sql = snapshot.sql;
    tab.filePath = snapshot.filePath;
    tab.savedSql = snapshot.savedSql;
    tabs.value.push(tab);
    tabOrder.value.push(tab.id);
    if (snapshot.connectionId) {
      await showQueryTab(tab.id);
    } else {
      activeTabId.value = tab.id;
      activeWorkspaceTabId.value = tab.id;
    }
    if (activeWorkspaceTabId.value !== tab.id) {
      status.value = `已恢复标签：${snapshot.title} · 来源上下文切换被阻止`;
      return;
    }
    status.value = snapshot.hadTransaction
      ? `已恢复：${snapshot.title} · 事务未恢复`
      : `已恢复：${snapshot.title}`;
    return;
  }
  if (snapshot.kind === "table") {
    if (!connection || connection.driver === "redis") {
      status.value = "来源连接已删除，无法恢复表标签";
      return;
    }
    const tab = createTableTab(
      { ...snapshot.table },
      snapshot.connectionId,
      snapshot.database,
      snapshot.schema,
      snapshot.mode,
      snapshot.focusedIndex,
    );
    tableTabs.value.push(tab);
    tabOrder.value.push(tab.id);
    await reopenTableTab(tab, snapshot.table.name);
    return;
  }
  if (snapshot.kind === "er") {
    if (!connection || connection.driver === "redis") {
      status.value = "来源连接已删除，无法恢复 ER 图";
      return;
    }
    const tab = createErTab(
      snapshot.connectionId,
      snapshot.database,
      snapshot.schema,
    );
    erTabs.value.push(tab);
    tabOrder.value.push(tab.id);
    await showErTab(tab);
    status.value =
      activeWorkspaceTabId.value === tab.id
        ? "已恢复 ER 图"
        : "已恢复 ER 标签 · 来源上下文切换被阻止";
  }
}

async function reopenTableTab(tab: TableTab, tableName: string) {
  const activated = await selectTableTab(tab);
  if (!activated) {
    tableTabs.value = tableTabs.value.filter((item) => item.id !== tab.id);
    removeTabFromOrder(tab.id);
    fallbackToQueryTab();
    return;
  }
  if (
    !tables.value.some(
      (item) => item.name === tab.table.name && item.kind === tab.table.kind,
    )
  ) {
    tableTabs.value = tableTabs.value.filter((item) => item.id !== tab.id);
    removeTabFromOrder(tab.id);
    fallbackToQueryTab();
    status.value = `对象「${tableName}」已不存在，未恢复`;
    return;
  }
  status.value = tab.mode === "structure"
    ? `已恢复表结构：${tableName}`
    : `已恢复表数据：${tableName}`;
}

function openTabMenu(event: MouseEvent, tabId: string) {
  tabMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 230)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 260)),
    tabId,
  };
}

function closeTabMenu() {
  tabMenu.value = null;
}

function openSplitView(id: string) {
  closeTabMenu();
  let query = tabs.value.find((tab) => tab.id === id);
  let table = tableTabs.value.find((tab) => tab.id === id);
  let er = erTabs.value.find((tab) => tab.id === id);
  if (!query && !table && !er) {
    status.value = "没有可用的分屏标签";
    return;
  }
  const connectionId = query?.connectionId ?? table?.connectionId ?? er?.connectionId;
  const connection = connectionId ? connectionFor(connectionId) : null;
  if (connectionId && connectionId !== selectedId.value) {
    status.value = "分屏标签必须来自当前连接";
    return;
  }
  if (connection && (connection.driver === "mysql" || connection.driver === "postgres")) {
    const database = normalizeScopeValue(
      query?.database ?? table?.database ?? er?.database ?? activeDatabase.value,
    );
    const schema = normalizeScopeValue(
      query?.schema ?? table?.schema ?? er?.schema ?? activeSchema.value,
    );
    if (
      database !== normalizeScopeValue(activeDatabase.value) ||
      schema !== normalizeScopeValue(activeSchema.value)
    ) {
      status.value = "分屏标签必须与当前库 / 模式一致";
      return;
    }
  }
  let created = false;
  if (query && query.id === activeTabId.value && table === undefined && er === undefined) {
    // 分屏同一个查询标签只会镜像同一份 SQL 与结果；为右侧复制一个新查询
    // 标签，让左右两侧可以独立编辑和执行，形成真正的双查询分屏。
    const tab = createQueryTab(
      query.connectionId,
      query.database,
      query.schema,
    );
    tab.sql = query.sql;
    tabs.value.push(tab);
    tabOrder.value.push(tab.id);
    query = tab;
    id = tab.id;
    created = true;
  }
  splitTabId.value = id;
  historyOpen.value = false;
  status.value = created
    ? "已在右侧新建查询分屏（同一连接与库 / 模式）"
    : query !== undefined
      ? "已打开右侧查询分屏（同一连接与库 / 模式）"
      : "已打开右侧分屏（只读浏览）";
}

function closeSplitView() {
  splitTabId.value = null;
  status.value = "已关闭右侧分屏";
}

const tabMenuQueryTab = computed(() => {
  const menu = tabMenu.value;
  if (!menu) return null;
  return tabs.value.find((tab) => tab.id === menu.tabId) ?? null;
});

async function openQueryInNewWindow(tabId: string) {
  closeTabMenu();
  const tab = tabs.value.find((item) => item.id === tabId);
  if (!tab) {
    status.value = "没有可用的查询标签";
    return;
  }
  if (tab.connectionId !== selectedId.value) {
    status.value = "请先在主窗口选择该标签的连接，再打开新窗口";
    return;
  }
  const connection = tab.connectionId
    ? connectionFor(tab.connectionId)
    : null;
  if (
    connection &&
    (connection.driver === "mysql" || connection.driver === "postgres")
  ) {
    const database = normalizeScopeValue(
      tab.database ?? activeDatabase.value,
    );
    const schema = normalizeScopeValue(tab.schema ?? activeSchema.value);
    if (
      database !== normalizeScopeValue(activeDatabase.value) ||
      schema !== normalizeScopeValue(activeSchema.value)
    ) {
      status.value = "请先切换到该标签的库 / 模式，再打开独立查询窗口";
      return;
    }
  }
  try {
    await api.openQueryWindow({
      title: tab.title,
      connectionId: tab.connectionId,
      database: tab.database,
      schema: tab.schema,
      sql: tab.sql,
    });
    status.value = `已在新窗口打开 ${tab.title}`;
  } catch (reason) {
    status.value = String(reason);
  }
}

function startTabDrag(event: DragEvent, id: string) {
  dragTabId.value = id;
  dragOverTabId.value = "";
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", id);
  }
}

function markTabDragOver(event: DragEvent, id: string) {
  if (!dragTabId.value || dragTabId.value === id) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dragOverTabId.value = id;
}

function finishTabDrag() {
  dragTabId.value = "";
  dragOverTabId.value = "";
}

function dropTab(event: DragEvent, targetId: string) {
  event.preventDefault();
  const sourceId = dragTabId.value;
  finishTabDrag();
  if (!sourceId || sourceId === targetId) return;
  if (isTabPinned(sourceId) !== isTabPinned(targetId)) {
    status.value = "固定标签与普通标签不能跨组拖动";
    return;
  }
  const order = [...orderedTabIds.value];
  const from = order.indexOf(sourceId);
  const to = order.indexOf(targetId);
  if (from < 0 || to < 0) return;
  order.splice(from, 1);
  const insertAt = order.indexOf(targetId);
  order.splice(insertAt < 0 ? order.length : insertAt, 0, sourceId);
  const pinned = order.filter((id) => isTabPinned(id));
  const rest = order.filter((id) => !isTabPinned(id));
  pinnedTabIds.value = pinned;
  tabOrder.value = [...pinned, ...rest];
  status.value = "已调整标签顺序";
}

const tabMenuCounts = computed(() => {
  if (!tabMenu.value) return { other: 0, right: 0, all: 0 };
  const index = tabOrder.value.indexOf(tabMenu.value.tabId);
  const pinned = new Set(pinnedTabIds.value);
  return {
    other: tabOrder.value.filter(
      (id) => id !== tabMenu.value?.tabId && !pinned.has(id),
    ).length,
    right:
      index >= 0
        ? tabOrder.value
            .slice(index + 1)
            .filter((id) => !pinned.has(id)).length
        : 0,
    all: tabOrder.value.filter((id) => !pinned.has(id)).length,
  };
});

function tabIdsForMenu(kind: "other" | "right" | "all"): string[] {
  const menu = tabMenu.value;
  if (!menu) return [];
  const index = tabOrder.value.indexOf(menu.tabId);
  const pinned = new Set(pinnedTabIds.value);
  if (kind === "all") {
    return tabOrder.value.filter((id) => !pinned.has(id));
  }
  if (kind === "other") {
    return tabOrder.value.filter(
      (id) => id !== menu.tabId && !pinned.has(id),
    );
  }
  return index >= 0
    ? tabOrder.value.slice(index + 1).filter((id) => !pinned.has(id))
    : [];
}

function isTabPinned(id: string): boolean {
  return pinnedTabIds.value.includes(id);
}

function togglePinnedTab() {
  const id = tabMenu.value?.tabId;
  closeTabMenu();
  if (!id) return;
  if (isTabPinned(id)) {
    pinnedTabIds.value = pinnedTabIds.value.filter((item) => item !== id);
    const rest = tabOrder.value.filter((item) => item !== id);
    const pinnedCount = pinnedTabIds.value.filter((item) =>
      rest.includes(item),
    ).length;
    rest.splice(pinnedCount, 0, id);
    tabOrder.value = rest;
    status.value = "已取消固定标签";
  } else {
    pinnedTabIds.value = [...pinnedTabIds.value, id];
    status.value = "已固定标签到左侧";
  }
}

function closeTabsImmediately(ids: string[]) {
  for (const id of ids) {
    if (tabs.value.some((tab) => tab.id === id)) {
      closeQueryTabNow(id);
    } else if (tableTabs.value.some((tab) => tab.id === id)) {
      closeTableTab(id);
    } else if (erTabs.value.some((tab) => tab.id === id)) {
      closeErTab(id);
    }
  }
  void nextTick(() => {
    if (tabOrder.value.includes(activeWorkspaceTabId.value)) return;
    const remaining = tabOrder.value[0];
    if (remaining) {
      void activateWorkspaceTabById(remaining);
    } else {
      activeWorkspaceTabId.value = "";
      activeTabId.value = "";
    }
  });
}

function requestCloseTabs(kind: "other" | "right" | "all") {
  const ids = tabIdsForMenu(kind);
  closeTabMenu();
  if (!ids.length) {
    status.value = "没有可关闭的标签";
    return;
  }
  const closing = ids
    .map((id) => tabs.value.find((tab) => tab.id === id))
    .filter((tab): tab is QueryTab => Boolean(tab));
  const dirty = closing.filter(isQueryTabDirty).length;
  const transaction = closing.filter(
    (tab) => tab.inTransaction || tab.transactionBusy,
  ).length;
  const label =
    kind === "other"
      ? "关闭其他标签"
      : kind === "right"
        ? "关闭右侧标签"
        : "关闭全部标签";
  if (!dirty && !transaction) {
    closeTabsImmediately(ids);
    status.value = `已关闭 ${ids.length} 个标签`;
    return;
  }
  askConfirm({
    title: label,
    message:
      dirty > 0
        ? `其中 ${dirty} 个标签有未保存的 SQL 文件修改，关闭后会丢失。`
        : `其中 ${transaction} 个标签有未提交事务，关闭后会回滚。`,
    detail: `将关闭 ${ids.length} 个标签。`,
    confirmText: "关闭",
    danger: true,
    action: () => {
      closeTabsImmediately(ids);
      status.value = `已关闭 ${ids.length} 个标签`;
    },
  });
}

function closeTabFromMenu() {
  const id = tabMenu.value?.tabId;
  closeTabMenu();
  if (id) closeWorkspaceTab(id);
}

function reopenFromTabMenu() {
  closeTabMenu();
  void reopenClosedTab();
}

function copyTabMenuSource() {
  const menu = tabMenu.value;
  if (!menu) return;
  const view = workspaceTabViews.value.find(
    (item) => item.id === menu.tabId,
  );
  const text = view ? `${view.label} · ${view.source}` : menu.tabId;
  closeTabMenu();
  void writeClipboard(text)
    .then(() => {
      status.value = "已复制标签来源";
    })
    .catch(() => {
      status.value = "复制标签来源失败";
    });
}

function closeActiveWorkspaceTab() {
  if (isRedisConnection.value) {
    if (redisWorkspaceOpen.value) closeRedisWorkspace();
    return;
  }
  if (activeWorkspaceTabId.value) {
    closeWorkspaceTab(activeWorkspaceTabId.value);
  }
}

function closeWindow() {
  const dirty = tabs.value.some(isQueryTabDirty);
  const transaction = tabs.value.some(
    (tab) => tab.inTransaction || tab.transactionBusy,
  );
  if (!dirty && !transaction) {
    void getCurrentWindow().close();
    return;
  }
  askConfirm({
    title: "关闭窗口？",
    message: dirty
      ? "还有未保存的 SQL 文件修改，关闭后会丢失。"
      : "还有未提交的事务，关闭后会被回滚。",
    detail: "确定要关闭窗口吗？",
    confirmText: "关闭",
    danger: true,
    action: () => {
      void getCurrentWindow().close();
    },
  });
}

function quitApp() {
  const dirty = tabs.value.some(isQueryTabDirty);
  const transaction = tabs.value.some(
    (tab) => tab.inTransaction || tab.transactionBusy,
  );
  if (!dirty && !transaction) {
    void api.quitApp();
    return;
  }
  askConfirm({
    title: "退出 mydb？",
    message: dirty
      ? "还有未保存的 SQL 文件修改，退出后会丢失。"
      : "还有未提交的事务，退出后会被回滚。",
    detail: "确定要退出 mydb 吗？",
    confirmText: "退出",
    danger: true,
    action: () => {
      void api.quitApp();
    },
  });
}

async function toggleFullscreen() {
  const window = getCurrentWindow();
  try {
    const fullscreen = await window.isFullscreen();
    await window.setFullscreen(!fullscreen);
  } catch (reason) {
    status.value = String(reason);
  }
}

function openAuthorGithub() {
  void openUrl("https://github.com/mengqi19990203-creator");
}

function openProjectRepo() {
  void openUrl("https://github.com/mengqi19990203-creator/mydb");
}

async function copyContactEmail() {
  try {
    await writeClipboard("jianglichun996@163.com");
    status.value = "已复制联系邮箱";
  } catch {
    status.value = "复制联系邮箱失败";
  }
}

const canCloseActiveTab = computed(() =>
  isRedisConnection.value
    ? redisWorkspaceOpen.value
    : Boolean(activeWorkspaceTabId.value) &&
      tabOrder.value.includes(activeWorkspaceTabId.value),
);

const appActions = computed<AppAction[]>(() => {
  const modifier = isMacPlatform ? "⌘" : "Ctrl+";
  const shift = isMacPlatform ? "⇧" : "Shift+";
  const readOnly = Boolean(selected.value?.readOnly);
  const noConnection = !selectedId.value;
  const redis = isRedisConnection.value;
  const connectionReason = noConnection
    ? "请先选择连接"
    : redis
      ? "Redis 连接请使用命令控制台"
      : "";
  const canExport =
    viewMode.value === "sql" &&
    !redis &&
    !running.value &&
    !queryError.value &&
    Boolean(result.value?.isQuery) &&
    Boolean(result.value?.rows.length);
  const exportReason = redis
    ? "Redis 连接不支持 SQL 结果"
    : viewMode.value !== "sql"
      ? "请切换到查询标签"
      : running.value
        ? "查询执行中"
        : queryError.value
          ? "查询失败"
          : !result.value?.isQuery
            ? "当前没有结果集"
            : !result.value?.rows.length
              ? "结果为空"
              : "";
  const importReason = connectionReason
    ? connectionReason
    : readOnly
      ? "当前连接为只读"
      : activeTable.value?.kind === "table"
        ? ""
        : "请先打开一个表标签";

  return [
    {
      id: "new-connection",
      group: "connection",
      label: "新建连接",
      icon: "plus",
      shortcut: `${modifier}N`,
      priority: 1,
      enabled: true,
      run: openNewConnection,
    },
    {
      id: "new-query",
      group: "query",
      label: "新建查询",
      icon: "code",
      shortcut: `${modifier}T`,
      priority: 2,
      enabled: !redis,
      disabledReason: redis ? "Redis 连接请使用命令控制台" : undefined,
      run: newQuery,
    },
    {
      id: "open-sql-file",
      group: "query",
      label: "打开 SQL 文件…",
      icon: "code",
      shortcut: `${modifier}O`,
      priority: 3,
      enabled: !redis,
      disabledReason: redis ? "Redis 连接请使用命令控制台" : undefined,
      run: () => void openSqlFile(),
    },
    {
      id: "save-sql",
      group: "query",
      label: "保存 SQL",
      icon: "download",
      shortcut: `${modifier}S`,
      priority: 4,
      enabled: !redis && viewMode.value === "sql" && Boolean(activeTab.value),
      disabledReason: redis
        ? "Redis 连接请使用命令控制台"
        : viewMode.value !== "sql"
          ? "请先切换到查询标签"
          : activeTab.value
            ? undefined
            : "当前没有查询标签",
      run: saveActiveSql,
    },
    {
      id: "save-sql-as",
      group: "query",
      label: "另存为…",
      icon: "download",
      shortcut: `${modifier}${shift}S`,
      priority: 5,
      enabled: !redis && viewMode.value === "sql" && Boolean(activeTab.value),
      disabledReason: redis
        ? "Redis 连接请使用命令控制台"
        : viewMode.value !== "sql"
          ? "请先切换到查询标签"
          : activeTab.value
            ? undefined
            : "当前没有查询标签",
      run: saveActiveSqlAs,
    },
    {
      id: "reopen-closed-tab",
      group: "query",
      label: "重新打开关闭的标签",
      icon: "history",
      shortcut: `${modifier}${shift}T`,
      priority: 6,
      enabled: closedTabs.value.length > 0,
      disabledReason:
        closedTabs.value.length > 0
          ? undefined
          : "没有最近关闭的标签",
      run: () => void reopenClosedTab(),
    },
    {
      id: "new-table",
      group: "object",
      label: "新建表",
      icon: "structure",
      priority: 3,
      enabled: !connectionReason && !readOnly,
      disabledReason:
        connectionReason || (readOnly ? "当前连接为只读" : undefined),
      run: openCreateTable,
    },
    {
      id: "er-diagram",
      group: "tools",
      label: "ER 图",
      icon: "diagram",
      priority: 4,
      enabled: !connectionReason,
      disabledReason: connectionReason || undefined,
      active: viewMode.value === "er",
      run: () => void openErDiagram(),
    },
    {
      id: "refresh",
      group: "object",
      label: "刷新",
      icon: "refresh",
      shortcut: `${modifier}R`,
      priority: 5,
      enabled: Boolean(selectedId.value) && !tablesLoading.value,
      disabledReason: noConnection
        ? "请先选择连接"
        : tablesLoading.value
          ? "对象列表加载中"
          : undefined,
      run: () => void refreshTables(),
    },
    {
      id: "history",
      group: "view",
      label: "历史",
      icon: "history",
      shortcut: `${modifier}Y`,
      priority: 6,
      enabled: !redis,
      disabledReason: redis ? "Redis 连接暂不支持 SQL 历史" : undefined,
      active: historyOpen.value,
      run: () => void toggleHistory(),
    },
    {
      id: "structure-sync",
      group: "tools",
      label: "结构同步",
      icon: "structure",
      priority: 7,
      enabled: !connectionReason,
      disabledReason: connectionReason || undefined,
      run: () => {
        syncOpen.value = true;
      },
    },
    {
      id: "data-sync",
      group: "tools",
      label: "数据同步",
      icon: "sync",
      priority: 8,
      enabled: !connectionReason,
      disabledReason: connectionReason || undefined,
      run: () => {
        dataSyncOpen.value = true;
      },
    },
    {
      id: "query-builder",
      group: "tools",
      label: "查询构建器",
      icon: "structure",
      priority: 9,
      enabled: !connectionReason,
      disabledReason: connectionReason || undefined,
      run: () => {
        queryBuilderOpen.value = true;
      },
    },
    {
      id: "database-docs",
      group: "tools",
      label: "数据库文档",
      icon: "history",
      priority: 10,
      enabled: !connectionReason,
      disabledReason: connectionReason || undefined,
      run: () => {
        databaseDocsOpen.value = true;
      },
    },
    {
      id: "sql-snippets",
      group: "query",
      label: "SQL 片段库",
      icon: "code",
      shortcut: `${shift}${modifier}K`,
      priority: 23,
      placement: "more",
      enabled:
        viewMode.value === "sql" &&
        Boolean(activeTab.value) &&
        !isRedisConnection.value,
      disabledReason: "请先切换到查询标签",
      run: openSnippetLibrary,
    },
    {
      id: "save-snippet",
      group: "query",
      label: "保存选区为片段",
      icon: "download",
      priority: 24,
      placement: "more",
      enabled:
        viewMode.value === "sql" &&
        Boolean(activeTab.value) &&
        !isRedisConnection.value,
      disabledReason: "请先切换到查询标签",
      run: () =>
        openSnippetSave(
          queryWorkspaceRef.value?.currentStatement() ?? "",
        ),
    },
    {
      id: "compress-sql",
      group: "query",
      label: "压缩当前 SQL",
      icon: "code",
      priority: 25,
      placement: "more",
      enabled:
        viewMode.value === "sql" &&
        Boolean(activeTab.value) &&
        !isRedisConnection.value,
      disabledReason: "请先切换到查询标签",
      run: () => runSqlEditorCommand("compress-sql"),
    },
    {
      id: "format-settings",
      group: "query",
      label: "格式化设置…",
      icon: "code",
      priority: 26,
      placement: "more",
      enabled: true,
      run: openFormatSettings,
    },
    {
      id: "import-data",
      group: "object",
      label: "导入数据",
      icon: "upload",
      priority: 20,
      placement: "more",
      enabled: !importReason,
      disabledReason: importReason || undefined,
      run: () => {
        const table = activeTable.value;
        if (table) void openImportData(table);
      },
    },
    {
      id: "export-result",
      group: "query",
      label: "导出结果",
      icon: "download",
      priority: 21,
      placement: "more",
      enabled: canExport,
      disabledReason: exportReason || undefined,
      run: () => queryWorkspaceRef.value?.requestExport(),
    },
    {
      id: "close-tab",
      group: "query",
      label: "关闭标签",
      icon: "close",
      shortcut: `${modifier}W`,
      priority: 22,
      placement: "more",
      enabled: canCloseActiveTab.value,
      disabledReason: canCloseActiveTab.value
        ? undefined
        : "当前没有可关闭的标签",
      run: closeActiveWorkspaceTab,
    },
  ];
});

const canEditSql = computed(
  () =>
    viewMode.value === "sql" &&
    Boolean(activeTab.value) &&
    !isRedisConnection.value,
);

const exportDatabaseReason = computed(() => {
  if (!selected.value) return "请先选择连接";
  if (selected.value.driver === "redis") return "Redis 不支持 SQL 导出";
  if (connectionStatus.value !== "connected") return "连接尚未就绪";
  return "";
});

const hasOpenModal = computed(
  () =>
    paletteOpen.value ||
    domModalOpen.value ||
    dialogOpen.value ||
    createOpen.value ||
    importOpen.value ||
    syncOpen.value ||
    dataSyncOpen.value ||
    databaseExportOpen.value ||
    sqlScriptOpen.value ||
    snippetOpen.value ||
    formatSettingsOpen.value ||
    settingsOpen.value ||
    queryBuilderOpen.value ||
    databaseDocsOpen.value ||
    Boolean(dangerousSqlRequest.value) ||
    Boolean(objectSource.value) ||
    createNamespaceOpen.value ||
    Boolean(generateTarget.value) ||
    backupSettingsOpen.value ||
    serverActivityOpen.value ||
    dataTransferOpen.value ||
    connectionTransferOpen.value ||
    globalSearchOpen.value ||
    aboutOpen.value ||
    shortcutHelpOpen.value ||
    duplicateOpen.value ||
    groupPromptOpen.value ||
    Boolean(confirmRequest.value),
);

const modalBlockedActionIds = new Set([
  "new-connection",
  "new-query",
  "open-sql-file",
  "save-sql",
  "save-sql-as",
  "reopen-closed-tab",
  "import-data",
  "export-result",
  "export-database",
  "execute-sql-file",
  "server-activity",
  "data-transfer",
  "connection-transfer",
  "backup-settings",
  "sql-snippets",
  "save-snippet",
  "global-search",
  "close-tab",
  "refresh",
  "er-diagram",
  "structure-sync",
  "data-sync",
  "query-builder",
  "database-docs",
  "edit-connection",
  "close-connection",
  "editor-search",
  "editor-find-next",
  "editor-find-previous",
  "editor-goto-line",
  "editor-toggle-comment",
  "editor-format-sql",
  "editor-format-settings",
  "settings",
  "close-window",
  "quit-app",
  "command-palette",
  "toggle-sidebar",
  "toggle-objects",
  "toggle-result",
  "toggle-right",
  "reset-layout",
  "toggle-fullscreen",
  "about",
  "shortcut-help",
]);

function blockedByModal(id: string | undefined): boolean {
  return Boolean(
    id && hasOpenModal.value && modalBlockedActionIds.has(id),
  );
}

function modalDisabledReason(id: string | undefined): string | undefined {
  return blockedByModal(id) ? "请先关闭当前对话框" : undefined;
}

function runSqlEditorCommand(
  command:
    | "search"
    | "find-next"
    | "find-previous"
    | "goto-line"
    | "toggle-comment"
    | "format-sql"
    | "compress-sql",
) {
  if (!canEditSql.value) return;
  if (hasOpenModal.value) {
    status.value = "请先关闭当前对话框";
    return;
  }
  queryWorkspaceRef.value?.runEditorCommand(command);
}

const appMenus = computed<MenuBarMenu[]>(() => {
  const modifier = isMacPlatform ? "⌘" : "Ctrl+";
  const shift = isMacPlatform ? "⇧" : "Shift+";
  const alt = isMacPlatform ? "⌥" : "Alt+";
  const fromAction = (id: string): MenuBarItem => {
    const action = appActions.value.find((item) => item.id === id);
    if (!action) return { label: id, disabled: true };
    const blocked = blockedByModal(action.id);
    return {
      id: action.id,
      label: action.label,
      shortcut: action.shortcut,
      disabled: !action.enabled || blocked,
      disabledReason: blocked
        ? "请先关闭当前对话框"
        : action.disabledReason,
      action: action.run,
    };
  };
  const editItem = (
    id: string,
    label: string,
    shortcut: string,
    command: "undo" | "redo" | "cut" | "copy" | "paste" | "selectAll",
  ): MenuBarItem => ({
    id,
    label,
    shortcut,
    disabled: !hasEditableTarget.value,
    disabledReason: "请先聚焦输入框或代码编辑器",
    action: () => runMenuEdit(command),
  });

  return [
    {
      label: "文件",
      items: [
        fromAction("new-connection"),
        fromAction("new-query"),
        { separator: true },
        fromAction("open-sql-file"),
        {
          id: "execute-sql-file",
          label: "执行 SQL 文件…",
          disabled:
            !selected.value ||
            selected.value.driver === "redis" ||
            blockedByModal("execute-sql-file"),
          disabledReason: blockedByModal("execute-sql-file")
            ? "请先关闭当前对话框"
            : "请选择 SQLite / MySQL / PostgreSQL 连接",
          action: openSqlScriptDialog,
        },
        fromAction("save-sql"),
        fromAction("save-sql-as"),
        fromAction("reopen-closed-tab"),
        { separator: true },
        fromAction("import-data"),
        fromAction("export-result"),
        {
          id: "export-database",
          label: "导出数据库 SQL…",
          disabled: Boolean(exportDatabaseReason.value) || blockedByModal("export-database"),
          disabledReason: blockedByModal("export-database")
            ? "请先关闭当前对话框"
            : exportDatabaseReason.value || undefined,
          action: () => void openDatabaseExportAt(),
        },
        {
          id: "connection-transfer",
          label: "导入 / 导出连接…",
          disabled: blockedByModal("connection-transfer"),
          disabledReason: blockedByModal("connection-transfer")
            ? "请先关闭当前对话框"
            : undefined,
          action: () => {
            connectionTransferOpen.value = true;
          },
        },
        {
          id: "backup-settings",
          label: "备份计划…",
          disabled: blockedByModal("backup-settings"),
          disabledReason: blockedByModal("backup-settings")
            ? "请先关闭当前对话框"
            : undefined,
          action: () => {
            backupSettingsOpen.value = true;
          },
        },
        { separator: true },
        fromAction("close-tab"),
        { separator: true },
        {
          id: "close-window",
          label: "关闭窗口",
          shortcut: `${modifier}${shift}W`,
          disabled: blockedByModal("close-window"),
          disabledReason: modalDisabledReason("close-window"),
          action: closeWindow,
        },
        ...(isMacPlatform
          ? []
          : [
              {
                id: "quit-app",
                label: "退出 mydb",
                shortcut: `${modifier}Q`,
                disabled: blockedByModal("quit-app"),
                disabledReason: modalDisabledReason("quit-app"),
                action: quitApp,
              },
            ]),
      ],
    },
    {
      label: "编辑",
      items: [
        editItem("edit-undo", "撤销", `${modifier}Z`, "undo"),
        editItem(
          "edit-redo",
          "重做",
          `${modifier}${shift}Z`,
          "redo",
        ),
        { separator: true },
        editItem("edit-cut", "剪切", `${modifier}X`, "cut"),
        editItem("edit-copy", "复制", `${modifier}C`, "copy"),
        editItem("edit-paste", "粘贴", `${modifier}V`, "paste"),
        editItem(
          "edit-select-all",
          "全选",
          `${modifier}A`,
          "selectAll",
        ),
        { separator: true },
        {
          id: "editor-search",
          label: "查找和替换…",
          shortcut: `${modifier}F`,
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("search"),
        },
        {
          id: "editor-find-next",
          label: "查找下一个",
          shortcut: "F3",
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("find-next"),
        },
        {
          id: "editor-find-previous",
          label: "查找上一个",
          shortcut: `${shift}F3`,
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("find-previous"),
        },
        {
          id: "editor-goto-line",
          label: "转到行…",
          shortcut: `${modifier}${alt}G`,
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("goto-line"),
        },
        {
          id: "editor-toggle-comment",
          label: "注释 / 取消注释",
          shortcut: `${modifier}/`,
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("toggle-comment"),
        },
        {
          id: "editor-format-sql",
          label: "格式化 SQL",
          shortcut: `${shift}${alt}F`,
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("format-sql"),
        },
        {
          id: "editor-compress-sql",
          label: "压缩 SQL",
          disabled: !canEditSql.value,
          disabledReason: "请先打开查询标签",
          action: () => runSqlEditorCommand("compress-sql"),
        },
        {
          id: "editor-format-settings",
          label: "格式化设置…",
          disabled: blockedByModal("editor-format-settings"),
          disabledReason: blockedByModal("editor-format-settings")
            ? "请先关闭当前对话框"
            : undefined,
          action: openFormatSettings,
        },
        { separator: true },
        {
          id: "dangerous-confirm",
          label: "危险 SQL 二次确认",
          checked: dangerousConfirmEnabled.value,
          action: toggleDangerousConfirm,
        },
        { separator: true },
        {
          id: "sql-snippets",
          label: "SQL 片段库…",
          shortcut: `${shift}${modifier}K`,
          disabled:
            !canEditSql.value || blockedByModal("sql-snippets"),
          disabledReason: blockedByModal("sql-snippets")
            ? "请先关闭当前对话框"
            : "请先打开查询标签",
          action: openSnippetLibrary,
        },
        {
          id: "save-snippet",
          label: "保存选区为片段…",
          disabled:
            !canEditSql.value || blockedByModal("save-snippet"),
          disabledReason: blockedByModal("save-snippet")
            ? "请先关闭当前对话框"
            : "请先打开查询标签",
          action: () =>
            openSnippetSave(
              queryWorkspaceRef.value?.currentStatement() ?? "",
            ),
        },
      ],
    },
    {
      label: "视图",
      items: [
        fromAction("refresh"),
        fromAction("history"),
        {
          id: "command-palette",
          label: "命令面板…",
          shortcut: `${modifier}K`,
          disabled: blockedByModal("command-palette"),
          disabledReason: modalDisabledReason("command-palette"),
          action: openPalette,
        },
        { separator: true },
        {
          id: "toggle-sidebar",
          label: "连接侧栏",
          shortcut: layoutShortcuts.value.sidebar,
          checked: !layout.sidebarCollapsed,
          disabled: blockedByModal("toggle-sidebar"),
          disabledReason: modalDisabledReason("toggle-sidebar"),
          action: toggleSidebar,
        },
        {
          id: "toggle-objects",
          label: "对象树",
          shortcut: layoutShortcuts.value.objects,
          checked: !layout.objectsCollapsed,
          disabled: blockedByModal("toggle-objects"),
          disabledReason: modalDisabledReason("toggle-objects"),
          action: toggleObjects,
        },
        {
          id: "toggle-result",
          label: "结果面板",
          shortcut: layoutShortcuts.value.result,
          checked: !layout.resultCollapsed,
          disabled: blockedByModal("toggle-result"),
          disabledReason: modalDisabledReason("toggle-result"),
          action: toggleResultPanel,
        },
        {
          id: "toggle-right",
          label: "右侧面板",
          shortcut: layoutShortcuts.value.right,
          checked: layout.rightOpen,
          disabled: blockedByModal("toggle-right"),
          disabledReason: modalDisabledReason("toggle-right"),
          action: toggleRightPanel,
        },
        { separator: true },
        {
          id: "reset-layout",
          label: "重置布局",
          disabled: blockedByModal("reset-layout"),
          disabledReason: modalDisabledReason("reset-layout"),
          action: resetLayout,
        },
        { separator: true },
        {
          id: "toggle-fullscreen",
          label: "切换全屏",
          disabled: blockedByModal("toggle-fullscreen"),
          disabledReason: modalDisabledReason("toggle-fullscreen"),
          action: () => void toggleFullscreen(),
        },
      ],
    },
    {
      label: "工具",
      items: [
        fromAction("er-diagram"),
        fromAction("structure-sync"),
        fromAction("data-sync"),
        fromAction("query-builder"),
        fromAction("database-docs"),
        {
          id: "data-transfer",
          label: "数据传输…",
          disabled:
            !selected.value ||
            selected.value.driver === "redis" ||
            blockedByModal("data-transfer"),
          disabledReason: blockedByModal("data-transfer")
            ? "请先关闭当前对话框"
            : "请选择 SQLite / MySQL / PostgreSQL 连接",
          action: () => {
            dataTransferOpen.value = true;
          },
        },
        {
          id: "server-activity",
          label: "服务器活动…",
          disabled:
            Boolean(serverActivityReason.value) ||
            blockedByModal("server-activity"),
          disabledReason: blockedByModal("server-activity")
            ? "请先关闭当前对话框"
            : serverActivityReason.value || undefined,
          action: openServerActivity,
        },
        {
          id: "global-search",
          label: "全局对象搜索…",
          disabled:
            !selected.value ||
            selected.value.driver === "redis" ||
            blockedByModal("global-search"),
          disabledReason: blockedByModal("global-search")
            ? "请先关闭当前对话框"
            : "请选择 SQLite / MySQL / PostgreSQL 连接",
          action: openGlobalSearch,
        },
        {
          id: "settings",
          label: "设置…",
          shortcut: `${modifier},`,
          disabled: blockedByModal("settings"),
          disabledReason: "请先关闭当前对话框",
          action: () => {
            settingsOpen.value = true;
          },
        },
        { separator: true },
        {
          id: "edit-connection",
          label: "编辑当前连接…",
          disabled:
            !selected.value || blockedByModal("edit-connection"),
          disabledReason: blockedByModal("edit-connection")
            ? "请先关闭当前对话框"
            : "请先选择连接",
          action: editSelectedConnection,
        },
        {
          id: "close-connection",
          label: "关闭当前连接",
          disabled:
            !selected.value || blockedByModal("close-connection"),
          disabledReason: blockedByModal("close-connection")
            ? "请先关闭当前对话框"
            : "请先选择连接",
          action: () => void closeSelectedConnection(),
        },
      ],
    },
    {
      label: "窗口",
      items: [
        {
          id: "minimize-window",
          label: "最小化",
          shortcut: `${modifier}M`,
          action: () => {
            void getCurrentWindow().minimize();
          },
        },
      ],
    },
    {
      label: "帮助",
      items: [
        {
          id: "about",
          label: "关于 mydb",
          disabled: blockedByModal("about"),
          disabledReason: modalDisabledReason("about"),
          action: () => {
            aboutOpen.value = true;
          },
        },
        {
          id: "shortcut-help",
          label: "键盘快捷键",
          disabled: blockedByModal("shortcut-help"),
          disabledReason: modalDisabledReason("shortcut-help"),
          action: () => {
            shortcutHelpOpen.value = true;
          },
        },
        { separator: true },
        {
          id: "github",
          label: "GitHub 项目主页",
          action: () => {
            void openUrl(
              "https://github.com/mengqi19990203-creator/mydb",
            );
          },
        },
      ],
    },
  ];
});

const nativeMenuState = computed<api.NativeMenuState>(() => {
  const enabled: Record<string, boolean> = {};
  const checked: Record<string, boolean> = {};
  for (const menu of appMenus.value) {
    for (const item of menu.items) {
      if (!item.id) continue;
      enabled[`menu-${item.id}`] =
        !item.disabled &&
        !(hasOpenModal.value && modalBlockedActionIds.has(item.id));
      if (item.checked !== undefined) {
        checked[`menu-${item.id}`] = item.checked;
      }
    }
  }
  return { enabled, checked };
});

let menuSyncTimer = 0;

watch(
  nativeMenuState,
  (state) => {
    if (!isMacPlatform) return;
    window.clearTimeout(menuSyncTimer);
    menuSyncTimer = window.setTimeout(() => {
      void api.syncMenuState(state).catch(() => {
        // The native menu is optional; keep the in-window menu working.
      });
    }, 80);
  },
  { deep: true },
);

const paletteConnections = computed<PaletteItem[]>(() =>
  connections.value.map((connection) => ({
    id: connection.id,
    kind: "connection",
    label: connection.name,
    detail: [
      connection.driver,
      connectionTarget(connection),
      connection.id === selectedId.value
        ? connectionStatusLabel.value
        : "未打开",
      connection.group || "",
    ]
      .filter(Boolean)
      .join(" · "),
    badge: connection.driver,
    icon: "database",
    keywords: [
      connection.driver,
      connection.group,
      connection.name,
      connection.host,
    ].filter(Boolean),
    run: () => openConnectionFromPalette(connection.id),
  })),
);

async function openConnectionFromPalette(id: string) {
  await selectConnection(id);
  if (isRedisConnection.value) {
    redisWorkspaceOpen.value = true;
    return;
  }
  ensureTabForSource(true);
}

const paletteObjects = computed<PaletteItem[]>(() => {
  if (!selectedId.value || isRedisConnection.value) return [];
  const items: PaletteItem[] = [];
  const source = {
    connectionId: selectedId.value,
    database: activeDatabase.value,
    schema: activeSchema.value,
  };
  const sourceLabel = tabSourceLabel(source);
  for (const database of databases.value) {
    items.push({
      id: `database:${database}`,
      kind: "object",
      label: database,
      detail: `数据库 · ${selected.value?.name ?? ""}`,
      badge: "库",
      icon: "database",
      keywords: ["database", "db"],
      run: () => void selectDatabase(database),
    });
  }
  for (const schema of schemas.value) {
    items.push({
      id: `schema:${schema}`,
      kind: "object",
      label: schema,
      detail: `模式 · ${selected.value?.name ?? ""}`,
      badge: "模式",
      icon: "structure",
      keywords: ["schema"],
      run: () => void selectSchema(schema),
    });
  }
  for (const table of tables.value.filter(isViewableTable)) {
    const isView = table.kind !== "table";
    items.push({
      id: `table:${table.kind}:${table.name}`,
      kind: "object",
      label: table.name,
      detail: sourceLabel,
      badge: isView ? "视图" : "表",
      icon: isView ? "view" : "table",
      keywords: [table.kind, table.name],
      run: () => openTable(table),
      runAlt: () => openTableInNewTab(table),
    });
  }
  return items;
});

const paletteTabs = computed<PaletteItem[]>(() =>
  workspaceTabViews.value.map((view) => ({
    id: view.id,
    kind: "tab",
    label: view.label,
    detail: view.running ? `${view.source} · 执行中` : view.source,
    badge:
      view.kind === "query"
        ? "查询"
        : view.kind === "er"
          ? "ER"
          : "表",
    icon:
      view.kind === "query"
        ? "code"
        : view.kind === "er"
          ? "diagram"
          : "table",
    keywords: [view.title, view.source],
    run: () => void activateWorkspaceTabById(view.id),
  })),
);

const paletteActionIds = [
  "new-connection",
  "new-query",
  "new-table",
  "refresh",
  "er-diagram",
  "structure-sync",
  "data-sync",
  "history",
  "import-data",
  "export-result",
  "open-sql-file",
  "save-sql",
  "reopen-closed-tab",
  "close-tab",
];

const paletteCommands = computed<PaletteItem[]>(() => {
  const commands: PaletteItem[] = [];
  for (const id of paletteActionIds) {
    const action = appActions.value.find((item) => item.id === id);
    if (!action) continue;
    commands.push({
      id: `command:${action.id}`,
      kind: "command",
      label: action.label,
      detail: action.disabledReason || "命令",
      badge: "命令",
      icon: action.icon,
      keywords: [action.id, action.label],
      shortcut: action.shortcut,
      disabledReason: action.enabled
        ? undefined
        : action.disabledReason || "当前不可用",
      run: action.run,
    });
  }
  commands.push({
    id: "command:run-query",
    kind: "command",
    label: "运行当前查询",
    detail: runDisabledReason.value || "执行当前 SQL",
    badge: "命令",
    icon: "play",
    keywords: ["run", "execute", "sql"],
    shortcut: runShortcutLabel.value,
    disabledReason: runDisabledReason.value || undefined,
    run: () => void runQuery(),
  });
  commands.push({
    id: "command:edit-connection",
    kind: "command",
    label: "编辑当前连接",
    detail: selected.value ? selected.value.name : "请先选择连接",
    badge: "命令",
    icon: "database",
    keywords: ["edit", "connection"],
    disabledReason: selected.value ? undefined : "请先选择连接",
    run: editSelectedConnection,
  });
  for (const item of [
    {
      id: "command:layout.sidebar",
      label: "切换连接侧栏",
      shortcut: layoutShortcuts.value.sidebar,
      run: toggleSidebar,
    },
    {
      id: "command:layout.objects",
      label: "切换对象树",
      shortcut: layoutShortcuts.value.objects,
      run: toggleObjects,
    },
    {
      id: "command:layout.result",
      label: "切换结果面板",
      shortcut: layoutShortcuts.value.result,
      run: toggleResultPanel,
    },
    {
      id: "command:layout.right",
      label: "切换右侧面板",
      shortcut: layoutShortcuts.value.right,
      run: toggleRightPanel,
    },
    {
      id: "command:layout.reset",
      label: "重置布局",
      shortcut: "",
      run: resetLayout,
    },
  ]) {
    commands.push({
      id: item.id,
      kind: "command",
      label: item.label,
      detail: "布局",
      badge: "命令",
      icon: "structure",
      keywords: ["layout", item.label],
      shortcut: item.shortcut,
      run: item.run,
    });
  }
  return commands;
});

function openPalette() {
  if (paletteOpen.value || hasOpenModal.value) return;
  closeLayoutMenu();
  closeConnectionMenu();
  closeGroupMenu();
  closeAppContextMenu();
  palettePreviousFocus =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  paletteOpen.value = true;
}

function closePalette(restoreFocus = true) {
  const previous = palettePreviousFocus;
  palettePreviousFocus = null;
  paletteOpen.value = false;
  if (restoreFocus && previous?.isConnected) {
    void nextTick(() => {
      if (previous.isConnected) {
        previous.focus({ preventScroll: true });
      }
    });
  }
}

function executePaletteItem(item: PaletteItem, alt: boolean) {
  if (item.disabledReason) return;
  if (item.kind === "connection" || item.kind === "command") {
    paletteRecent.value = recordPaletteRecent(
      paletteRecent.value,
      item.kind,
      item.id,
    );
    savePaletteRecent(paletteRecent.value);
  }
  const previous = palettePreviousFocus;
  closePalette(false);
  if (previous?.isConnected) {
    previous.focus({ preventScroll: true });
  }
  const action = alt && item.runAlt ? item.runAlt : item.run;
  try {
    void action();
  } catch (reason) {
    status.value = String(reason);
  }
}

function clearPaletteRecentItems() {
  clearPaletteRecent();
  paletteRecent.value = [];
}

function onPaletteShortcut(event: KeyboardEvent) {
  const modifier = isMacPlatform ? event.metaKey : event.ctrlKey;
  if (
    !modifier ||
    event.altKey ||
    event.shiftKey ||
    event.repeat ||
    event.key.toLowerCase() !== "k"
  ) {
    return;
  }
  if (!paletteOpen.value && hasOpenModal.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (paletteOpen.value) {
    closePalette(true);
  } else {
    openPalette();
  }
}

async function bindMenuEvents() {
  const handlers: Array<[string, () => void]> = [
    ["menu:new-connection", openNewConnection],
    ["menu:settings", () => {
      settingsOpen.value = true;
    }],
    ["menu:new-query", newQuery],
    ["menu:open-sql-file", () => void openSqlFile()],
    ["menu:save-sql", saveActiveSql],
    ["menu:save-sql-as", saveActiveSqlAs],
    ["menu:reopen-closed-tab", () => void reopenClosedTab()],
    ["menu:import-data", () => {
      const table = activeTable.value;
      if (table) void openImportData(table);
    }],
    ["menu:export-result", () => queryWorkspaceRef.value?.requestExport()],
    ["menu:export-database", () => void openDatabaseExportAt()],
    ["menu:connection-transfer", () => {
      connectionTransferOpen.value = true;
    }],
    ["menu:backup-settings", () => {
      backupSettingsOpen.value = true;
    }],
    ["menu:execute-sql-file", openSqlScriptDialog],
    ["menu:close-tab", closeActiveWorkspaceTab],
    ["menu:close-window", closeWindow],
    [
      "menu:refresh",
      () => {
        if (selectedId.value) void refreshTables();
      },
    ],
    ["menu:history", () => void toggleHistory()],
    ["menu:command-palette", openPalette],
    ["menu:toggle-sidebar", toggleSidebar],
    ["menu:toggle-objects", toggleObjects],
    ["menu:toggle-result", toggleResultPanel],
    ["menu:toggle-right", toggleRightPanel],
    ["menu:reset-layout", resetLayout],
    [
      "menu:er-diagram",
      () => {
        if (selectedId.value && !isRedisConnection.value) {
          void openErDiagram();
        }
      },
    ],
    [
      "menu:structure-sync",
      () => {
        if (selectedId.value && !isRedisConnection.value) {
          syncOpen.value = true;
        }
      },
    ],
    [
      "menu:data-sync",
      () => {
        if (selectedId.value && !isRedisConnection.value) {
          dataSyncOpen.value = true;
        }
      },
    ],
    ["menu:server-activity", openServerActivity],
    ["menu:query-builder", () => {
      queryBuilderOpen.value = true;
    }],
    ["menu:database-docs", () => {
      databaseDocsOpen.value = true;
    }],
    ["menu:global-search", openGlobalSearch],
    [
      "menu:data-transfer",
      () => {
        if (
          selectedId.value &&
          !isRedisConnection.value &&
          selected.value?.driver !== "redis"
        ) {
          dataTransferOpen.value = true;
        }
      },
    ],
    [
      "menu:edit-connection",
      () => {
        if (selected.value) editSelectedConnection();
      },
    ],
    [
      "menu:close-connection",
      () => {
        if (selected.value) void closeSelectedConnection();
      },
    ],
    [
      "menu:github",
      () => {
        void openUrl(
          "https://github.com/mengqi19990203-creator/mydb",
        );
      },
    ],
    [
      "menu:shortcut-help",
      () => {
        shortcutHelpOpen.value = true;
      },
    ],
    [
      "menu:editor-search",
      () => runSqlEditorCommand("search"),
    ],
    [
      "menu:editor-find-next",
      () => runSqlEditorCommand("find-next"),
    ],
    [
      "menu:editor-find-previous",
      () => runSqlEditorCommand("find-previous"),
    ],
    [
      "menu:editor-goto-line",
      () => runSqlEditorCommand("goto-line"),
    ],
    [
      "menu:editor-toggle-comment",
      () => runSqlEditorCommand("toggle-comment"),
    ],
    [
      "menu:editor-format-sql",
      () => runSqlEditorCommand("format-sql"),
    ],
    [
      "menu:editor-compress-sql",
      () => runSqlEditorCommand("compress-sql"),
    ],
    ["menu:editor-format-settings", openFormatSettings],
    ["menu:editor-snippets", openSnippetLibrary],
    [
      "menu:editor-save-snippet",
      () =>
        openSnippetSave(
          queryWorkspaceRef.value?.currentStatement() ?? "",
        ),
    ],
  ];

  try {
    for (const [name, handler] of handlers) {
      unlisteners.push(
        await listen(name, () => {
          const actionId = name.startsWith("menu:")
            ? name.slice("menu:".length)
            : "";
          if (
            hasOpenModal.value &&
            modalBlockedActionIds.has(actionId)
          ) {
            return;
          }
          handler();
        }),
      );
    }
  } catch (reason) {
    console.error("无法绑定应用菜单事件", reason);
  }
}

onMounted(async () => {
  if (import.meta.env.DEV) {
    const duplicates = findDuplicateShortcuts();
    if (duplicates.length) {
      console.warn("快捷键冲突：", duplicates);
    }
  }
  window.addEventListener("contextmenu", openAppContextMenu);
  window.addEventListener("mousedown", onWindowMouseDown);
  window.addEventListener("keydown", onWindowKeydown);
  window.addEventListener("resize", closeAppContextMenu);
  window.addEventListener("resize", updateTabScroll);
  window.addEventListener("resize", updateViewportWidth);
  window.addEventListener("blur", closeAppContextMenu);
  window.addEventListener("blur", closeTabMenu);
  window.addEventListener("blur", () => {
    tabOverflowOpen.value = false;
  });
  window.addEventListener("resize", closeTabMenu);
  window.addEventListener("resize", () => {
    tabOverflowOpen.value = false;
  });
  window.addEventListener("keydown", onPaletteShortcut, true);
  document.addEventListener("focusin", trackEditableFocus);
  modalObserver = new MutationObserver(syncDomModalOpen);
  modalObserver.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class", "style"],
  });
  syncDomModalOpen();

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

  if (
    connections.value.length &&
    viewMode.value === "empty" &&
    !isRedisConnection.value
  ) {
    void loadHistory();
  }
  if (isMacPlatform) {
    void api.syncMenuState(nativeMenuState.value).catch(() => {
      // Native menu state is best-effort.
    });
  }
});

onBeforeUnmount(() => {
  if (tabsRestored && !standaloneQueryWindow) {
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
  window.clearTimeout(layoutSaveTimer);
  window.clearTimeout(menuSyncTimer);
  window.clearTimeout(locatedConnectionTimer);
  modalObserver?.disconnect();
  modalObserver = null;
  if (!standaloneQueryWindow) saveWorkspaceLayout(layout);
  for (const unlisten of unlisteners) unlisten();
  window.removeEventListener("contextmenu", openAppContextMenu);
  window.removeEventListener("mousedown", onWindowMouseDown);
  window.removeEventListener("keydown", onWindowKeydown);
  window.removeEventListener("resize", closeAppContextMenu);
  window.removeEventListener("resize", updateTabScroll);
  window.removeEventListener("resize", updateViewportWidth);
  window.removeEventListener("blur", closeAppContextMenu);
  window.removeEventListener("blur", closeTabMenu);
  window.removeEventListener("resize", closeTabMenu);
  window.removeEventListener("keydown", onPaletteShortcut, true);
  document.removeEventListener("focusin", trackEditableFocus);
});
</script>

<template>
  <div
    class="app-shell"
    :class="{ 'standalone-window': standaloneQueryWindow }"
    :style="layoutStyle"
  >
    <header class="titlebar">
      <AppMenuBar :menus="appMenus" />
      <span class="titlebar-spacer" data-tauri-drag-region></span>

      <button
        v-if="selected"
        class="connection-chip"
        :class="connectionStatus"
        :title="connectionChipTitle"
        @click="openConnectionChipMenu"
      >
        <span class="connection-chip-head">
          <span class="titlebar-connection">{{ selected.name }}</span>
          <span class="driver-badge">{{ selected.driver }}</span>
        </span>
        <span class="connection-chip-meta">
          <span v-if="connectionChipScope" class="connection-chip-scope">
            {{ connectionChipScope }}
          </span>
          <span class="status-dot" :class="connectionStatus"></span>
          <span class="connection-state" :class="connectionStatus">
            {{ connectionStatusLabel }}
          </span>
          <span v-if="selected.readOnly" class="readonly-badge">只读</span>
        </span>
      </button>
      <span v-else class="titlebar-muted">未连接</span>

      <div ref="layoutMenuRef" class="layout-menu-host">
        <button
          ref="layoutButtonRef"
          class="toolbar-action layout-button"
          :class="{ active: layoutMenuOpen }"
          title="布局"
          aria-label="布局"
          :aria-expanded="layoutMenuOpen"
          @click.stop="toggleLayoutMenu"
        >
          <Icon name="structure" :size="14" />
        </button>
        <div
          v-if="layoutMenuOpen"
          class="context-menu layout-menu"
          role="menu"
          :style="{
            position: 'fixed',
            top: `${layoutMenuTop}px`,
            right: `${layoutMenuRight}px`,
            left: 'auto',
          }"
          @click.stop
        >
          <button class="context-menu-item" @click="toggleSidebar">
            <span class="layout-check-slot">
              <Icon v-if="!layout.sidebarCollapsed" name="check" :size="14" />
            </span>
            <span>连接侧栏</span>
            <span class="context-menu-shortcut">
              {{ layoutShortcuts.sidebar }}
            </span>
          </button>
          <button class="context-menu-item" @click="toggleObjects">
            <span class="layout-check-slot">
              <Icon v-if="!layout.objectsCollapsed" name="check" :size="14" />
            </span>
            <span>对象树</span>
            <span class="context-menu-shortcut">
              {{ layoutShortcuts.objects }}
            </span>
          </button>
          <button class="context-menu-item" @click="toggleResultPanel">
            <span class="layout-check-slot">
              <Icon v-if="!layout.resultCollapsed" name="check" :size="14" />
            </span>
            <span>结果面板</span>
            <span class="context-menu-shortcut">
              {{ layoutShortcuts.result }}
            </span>
          </button>
          <button class="context-menu-item" @click="toggleRightPanel">
            <span class="layout-check-slot">
              <Icon v-if="layout.rightOpen" name="check" :size="14" />
            </span>
            <span>右侧面板</span>
            <span class="context-menu-shortcut">
              {{ layoutShortcuts.right }}
            </span>
          </button>
          <div class="context-separator"></div>
          <button class="context-menu-item" @click="resetLayout">
            <Icon name="refresh" :size="14" />
            <span>重置布局</span>
          </button>
        </div>
      </div>
    </header>

    <div class="app-body">
      <aside
        class="sidebar"
        :class="{ collapsed: layout.sidebarCollapsed }"
      >
        <div v-if="layout.sidebarCollapsed" class="sidebar-rail">
          <button
            class="icon-button"
            title="展开连接侧栏"
            aria-label="展开连接侧栏"
            @click="toggleSidebar"
          >
            <Icon name="chevron-right" :size="14" />
          </button>
          <button
            class="icon-button"
            title="新建连接"
            aria-label="新建连接"
            @click="openNewConnection"
          >
            <Icon name="plus" :size="14" />
          </button>
          <span class="sidebar-rail-label">连接</span>
        </div>
        <div class="sidebar-toolbar">
          <span class="sidebar-toolbar-title">连接</span>
          <button
            class="icon-button"
            :class="{ active: sidebarOpenOnly }"
            :aria-pressed="sidebarOpenOnly"
            :title="
              sidebarOpenOnly
                ? '显示全部连接'
                : '只看有打开标签的连接'
            "
            aria-label="筛选有打开标签的连接"
            @click="toggleSidebarOpenOnly"
          >
            <Icon name="filter" :size="14" />
          </button>
          <button
            class="icon-button"
            title="定位当前标签的连接"
            aria-label="定位当前标签的连接"
            :disabled="!activeConnectionId"
            @click="locateActiveConnection"
          >
            <Icon name="target" :size="14" />
          </button>
          <button
            class="icon-button"
            title="全部折叠"
            aria-label="折叠全部连接分组"
            @click="collapseAllGroups"
          >
            <Icon name="collapse" :size="14" />
          </button>
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
                :class="{
                  active: connection.id === selectedId,
                  located: locatedConnectionId === connection.id,
                }"
                :data-connection-id="connection.id"
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
                <span class="db-icon"><DriverIcon :driver="connection.driver" :size="18" /></span>
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
            {{
              sidebarOpenOnly
                ? "没有带打开标签的连接"
                : "没有匹配的连接"
            }}
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

      <ResizeHandle
        v-if="!layout.sidebarCollapsed"
        orientation="vertical"
        :value="effectiveWidths.sidebar"
        :min="180"
        :max="360"
        label="调整连接侧栏宽度"
        @update:value="layout.sidebarWidth = $event"
        @reset="layout.sidebarWidth = 240"
      />

      <section class="main">
        <div v-if="isRedisConnection" class="workspace-tabs">
          <div v-if="redisWorkspaceOpen" class="workspace-tab active redis-tab">
            <Icon name="database" :size="14" />
            <span class="tab-label">Redis</span>
            <span v-if="selected" class="hint">{{ selected.name }}</span>
            <button
              class="tab-close"
              title="关闭 Redis 控制台"
              @click.stop="closeRedisWorkspace"
            >
              <Icon name="close" :size="12" />
            </button>
          </div>
          <span v-else class="workspace-start-label">主工作区</span>
          <button
            v-if="!redisWorkspaceOpen"
            class="workspace-tab-add"
            title="打开 Redis 控制台"
            @click="openRedisWorkspace"
          >
            <Icon name="plus" :size="14" />
          </button>
        </div>
        <div v-else-if="connections.length" class="workspace-tabs">
          <span
            v-if="!workspaceTabViews.length"
            class="workspace-start-label"
          >
            主工作区
          </span>
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
              pinned: view.pinned,
              dragging: dragTabId === view.id,
              'drop-target': dragOverTabId === view.id,
            }"
            role="tab"
            tabindex="0"
            draggable="true"
            :title="view.title"
            @dragstart="startTabDrag($event, view.id)"
            @dragover="markTabDragOver($event, view.id)"
            @dragend="finishTabDrag"
            @drop="dropTab($event, view.id)"
            @click="activateWorkspaceTabById(view.id)"
            @contextmenu.prevent.stop="openTabMenu($event, view.id)"
            @keydown.enter="activateWorkspaceTabById(view.id)"
          >
            <Icon v-if="view.pinned" name="pin" :size="11" class="tab-pin" />
            <Icon :name="view.icon" :size="14" />
            <span class="tab-label">{{ view.label }}</span>
            <span
              v-if="view.dirty"
              class="tab-dirty"
              title="有未保存的修改"
              aria-label="有未保存的修改"
            ></span>
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
          <div
            v-if="workspaceTabViews.length"
            ref="tabOverflowRef"
            class="tab-overflow-host"
            @click.stop
          >
            <button
              class="workspace-tab-add tab-overflow-button"
              title="全部标签"
              aria-label="全部标签"
              @click="toggleTabOverflow"
            >
              <Icon name="list" :size="14" />
            </button>
            <div
              v-if="tabOverflowOpen"
              class="context-menu tab-overflow-menu"
              :style="{
                position: 'fixed',
                left: `${tabOverflowPosition.left}px`,
                top: `${tabOverflowPosition.top}px`,
              }"
            >
              <button
                v-for="view in workspaceTabViews"
                :key="view.id"
                class="context-menu-item"
                :class="{ 'tab-overflow-active': view.id === activeWorkspaceTabId }"
                @click="
                  activateWorkspaceTabById(view.id);
                  tabOverflowOpen = false;
                "
              >
                <Icon v-if="view.pinned" name="pin" :size="11" />
                <Icon v-else :name="view.icon" :size="13" />
                <span class="tab-overflow-label">{{ view.label }}</span>
                <span v-if="view.running" class="tab-overflow-running">执行中</span>
                <span class="tab-overflow-source">{{ view.source }}</span>
              </button>
            </div>
          </div>
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
          <WorkspaceStart
            v-if="connectionsLoaded && !connections.length"
            :connections-count="0"
            :selected="null"
            :target="''"
            :scope="''"
            :connection-status="connectionStatus"
            :connection-status-label="connectionStatusLabel"
            :tables="[]"
            :tables-loading="false"
            :history="[]"
            :history-loading="false"
            :command-key="commandKeyLabel"
            @new-connection="openNewConnection"
          />
          <RedisWorkspace
            v-else-if="isRedisConnection && selectedId && redisWorkspaceOpen"
            :key="selectedId"
            :connection-id="selectedId"
            :connection-name="selected?.name"
            :database="redisDatabase ?? selected?.database"
            :read-only="Boolean(selected?.readOnly)"
            @database-change="handleRedisDatabaseChange"
          />
          <WorkspaceStart
            v-else-if="isRedisConnection && selectedId"
            :connections-count="connections.length"
            :selected="selected"
            :target="selected ? connectionTarget(selected) : ''"
            :scope="connectionChipScope"
            :connection-status="connectionStatus"
            :connection-status-label="connectionStatusLabel"
            :tables="[]"
            :tables-loading="false"
            :history="[]"
            :history-loading="false"
            :command-key="commandKeyLabel"
            :redis="true"
            @new-connection="openNewConnection"
            @open-redis="openRedisWorkspace"
          />
          <template v-else>
            <ObjectBrowser
              v-if="selected"
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
              :collapsed="layout.objectsCollapsed"
              :locate-reason="objectLocateReason"
              :locate-scope="objectLocateScope"
              :read-only="Boolean(selected?.readOnly)"
              @toggle="toggleObjects"
              @select="openTable"
              @select-structure="openTableStructure"
              @select-index="openIndex"
              @open-new-tab="openTableInNewTab"
              @import-data="openImportData"
              @generate-data="
                openGenerateData({
                  connectionId: selectedId ?? '',
                  table: $event.name,
                  tableKind: $event.kind,
                })
              "
              @view-definition="openObjectDefinition"
              @create-namespace="openCreateNamespace"
              @drop-namespace="dropNamespace"
              @select-database="selectDatabase"
              @select-schema="selectSchema"
              @refresh="refreshTables"
              @create="openCreateTable"
              @generate-sql="openGeneratedSql"
              @export-database="openDatabaseExportAt"
              @create-object="openCreateObject"
            />
            <ResizeHandle
            v-if="selected && !layout.objectsCollapsed"
            orientation="vertical"
            :value="effectiveWidths.objects"
            :min="160"
            :max="420"
            label="调整对象树宽度"
            @update:value="layout.objectsWidth = $event"
            @reset="layout.objectsWidth = 216"
          />
          <div class="workspace-primary">
          <WorkspaceStart
            v-if="viewMode === 'empty'"
            :connections-count="connections.length"
            :selected="selected"
            :target="selected ? connectionTarget(selected) : ''"
            :scope="connectionChipScope"
            :connection-status="connectionStatus"
            :connection-status-label="connectionStatusLabel"
            :tables="tables"
            :tables-loading="tablesLoading"
            :history="historyEntries"
            :history-loading="historyLoading"
            :command-key="commandKeyLabel"
            @new-connection="openNewConnection"
            @new-query="newQuery"
            @select-history="useHistory"
            @open-table="openTable"
            @refresh-tables="refreshTables"
            @refresh-history="loadHistory(true)"
          />
          <QueryWorkspace
            ref="queryWorkspaceRef"
            v-show="viewMode === 'sql'"
            :sql="sql"
            :tab-id="activeTabId"
            :driver="driverForConnection(activeTab?.connectionId ?? null)"
            :completion-tables="completionTables"
            :result="result"
            :running="running"
            :stopping="activeTab?.stopping ?? false"
            :stop-note="activeTab?.stopNote ?? null"
            :error="queryError"
            :in-transaction="activeTab?.inTransaction ?? false"
            :transaction-busy="activeTab?.transactionBusy ?? false"
            :editor-ratio="layout.editorRatio"
            :result-collapsed="layout.resultCollapsed"
            :can-run="!runDisabledReason"
            :run-disabled-reason="runDisabledReason"
            :run-shortcut-label="runShortcutLabel"
            :explain-result="activeTab?.explainResult ?? null"
            :explain-error="activeTab?.explainError ?? null"
            :explain-loading="activeTab?.explainLoading ?? false"
            :show-explain="activeTab?.showExplain ?? false"
            @update:sql="sql = $event"
            @selection="handleSqlSelection"
            @run="runQuery"
            @stop="stopActiveQuery"
            @open-snippets="openSnippetLibrary"
            @open-format-settings="openFormatSettings"
            @explain="explainQuery"
            @show-explain="setExplainView"
            @begin-transaction="beginTransaction"
            @commit-transaction="commitTransaction"
            @rollback-transaction="rollbackTransaction"
            @update:editor-ratio="layout.editorRatio = $event"
            @toggle-result="toggleResultPanel"
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
                :ref="(element) => setTableDataRef(tab.id, element)"
                :connection-id="tab.connectionId ?? ''"
                :table="tab.table.name"
                :table-kind="tab.table.kind"
                :read-only="Boolean(connectionFor(tab.connectionId)?.readOnly)"
                :driver="driverForConnection(tab.connectionId)"
                :database-label="scopeLabelFor(tab)"
                :database="tab.database ?? undefined"
                :schema="tab.schema"
                @generate-data="
                  openGenerateData({
                    connectionId: tab.connectionId ?? '',
                    table: tab.table.name,
                    tableKind: tab.table.kind,
                  })
                "
                @import-data="
                  importTable = tab.table.name;
                  importOpen = true;
                "
              />
              <TableStructureView
                v-else
                :connection-id="tab.connectionId ?? ''"
                :driver="driverForConnection(tab.connectionId)"
                :read-only="Boolean(connectionFor(tab.connectionId)?.readOnly)"
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
          </div>
          <template v-if="splitView">
            <ResizeHandle
              orientation="vertical"
              invert
              :value="splitWidth"
              :min="320"
              :max="900"
              label="调整分屏宽度"
              @update:value="splitWidth = $event"
              @reset="splitWidth = 520"
            />
            <aside
              class="split-pane"
              :style="{ width: `${splitWidth}px` }"
            >
              <header class="split-pane-header">
                <Icon
                  :name="
                    splitView.kind === 'er'
                      ? 'diagram'
                      : splitView.kind === 'query'
                        ? 'code'
                        : splitView.table.mode === 'structure'
                          ? 'structure'
                          : 'table'
                  "
                  :size="14"
                />
                <strong>
                  {{
                    splitView.kind === "er"
                      ? "ER 图"
                      : splitView.kind === "query"
                        ? splitView.query.title
                        : splitView.table.table.name
                  }}
                </strong>
                <span class="hint">
                  {{
                    splitView.kind === "er"
                      ? scopeLabelFor(splitView.er)
                      : splitView.kind === "query"
                        ? [splitView.query.database, splitView.query.schema]
                            .filter(Boolean)
                            .join(" / ") || "当前库 / 模式"
                        : `${scopeLabelFor(splitView.table)} · ${
                            splitView.table.mode === "structure"
                              ? "结构"
                              : "数据"
                          }`
                  }}
                </span>
                <span class="split-badge">
                  {{ splitView.kind === "query" ? "查询分屏" : "只读分屏" }}
                </span>
                <button
                  class="icon-button"
                  title="关闭分屏"
                  @click="closeSplitView"
                >
                  <Icon name="close" :size="13" />
                </button>
              </header>
              <div class="split-pane-body">
                <template v-if="splitView.kind === 'query'">
                  <QueryWorkspace
                    ref="splitQueryWorkspaceRef"
                    :sql="splitView.query.sql"
                    :tab-id="splitView.query.id"
                    :driver="
                      driverForConnection(splitView.query.connectionId)
                    "
                    :completion-tables="completionTables"
                    :result="splitView.query.result"
                    :running="splitView.query.running"
                    :stopping="splitView.query.stopping"
                    :stop-note="splitView.query.stopNote"
                    :error="splitView.query.error"
                    :in-transaction="splitView.query.inTransaction"
                    :transaction-busy="splitView.query.transactionBusy"
                    :editor-ratio="layout.editorRatio"
                    :result-collapsed="layout.resultCollapsed"
                    :can-run="Boolean(splitView.query.connectionId)"
                    :run-disabled-reason="
                      splitView.query.connectionId ? '' : '请先选择连接'
                    "
                    run-shortcut-label="⌘/Ctrl+Enter"
                    :explain-result="splitView.query.explainResult"
                    :explain-error="splitView.query.explainError"
                    :explain-loading="splitView.query.explainLoading"
                    :show-explain="splitView.query.showExplain"
                    @update:sql="updateSplitSql"
                    @selection="updateSplitSelection"
                    @run="runSplitQuery"
                    @stop="stopSplitQuery"
                    @open-snippets="openSnippetLibrary"
                    @open-format-settings="openFormatSettings"
                    @explain="explainSplitQuery"
                    @show-explain="setSplitExplainView"
                    @begin-transaction="beginSplitTransaction"
                    @commit-transaction="commitSplitTransaction"
                    @rollback-transaction="rollbackSplitTransaction"
                    @update:editor-ratio="layout.editorRatio = $event"
                    @toggle-result="toggleResultPanel"
                  />
                </template>
                <template v-else-if="splitView.kind === 'er'">
                  <ErDiagram
                    :connection-id="splitView.er.connectionId ?? ''"
                    :scope-label="scopeLabelFor(splitView.er)"
                    @select="openTable"
                  />
                </template>
                <template v-else>
                  <TableDataView
                    v-if="splitView.table.mode === 'data'"
                    :key="`split-data-${splitView.table.id}`"
                    :connection-id="splitView.table.connectionId ?? ''"
                    :table="splitView.table.table.name"
                    :table-kind="splitView.table.table.kind"
                    :read-only="true"
                    :driver="driverForConnection(splitView.table.connectionId)"
                    :database-label="scopeLabelFor(splitView.table)"
                    :database="splitView.table.database ?? undefined"
                    :schema="splitView.table.schema"
                  />
                  <TableStructureView
                    v-else
                    :key="`split-structure-${splitView.table.id}`"
                    :connection-id="splitView.table.connectionId ?? ''"
                    :driver="driverForConnection(splitView.table.connectionId)"
                    :read-only="true"
                    :kind="splitView.table.table.kind"
                    :table="splitView.table.table.name"
                    :focus-index="splitView.table.focusedIndex"
                    :database-label="scopeLabelFor(splitView.table)"
                    @refresh="refreshTables"
                  />
                </template>
              </div>
            </aside>
          </template>
          </template>

          <ResizeHandle
            v-if="viewMode !== 'empty' && historyOpen && !isRedisConnection"
            class="resize-handle-right"
            orientation="vertical"
            invert
            :value="effectiveWidths.right"
            :min="260"
            :max="520"
            label="调整右侧面板宽度"
            @update:value="layout.rightWidth = $event"
            @reset="layout.rightWidth = 320"
          />
          <HistoryPanel
            v-if="viewMode !== 'empty' && !isRedisConnection"
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
          :result="viewMode === 'sql' ? result : null"
          :database="
            isRedisConnection
              ? redisDatabase
              : activeDatabase ?? selected?.database ?? null
          "
          :schema="activeSchema"
          :active-label="statusActiveLabel"
          :active-kind="statusActiveKind"
          :connection-status="connectionStatus"
          :connection-status-label="connectionStatusLabel"
          :read-only="selected?.readOnly ?? false"
          :in-transaction="
            viewMode === 'sql' && (activeTab?.inTransaction ?? false)
          "
          :running="viewMode === 'sql' && (activeTab?.running ?? false)"
          :error="viewMode === 'sql' ? queryError : null"
        />
      </section>
    </div>

    <ShortcutHelpDialog
      :open="shortcutHelpOpen"
      @close="shortcutHelpOpen = false"
    />

    <CommandPalette
      :open="paletteOpen"
      :connections="paletteConnections"
      :objects="paletteObjects"
      :tabs="paletteTabs"
      :commands="paletteCommands"
      :recent="paletteRecent"
      :objects-loading="tablesLoading"
      :object-status="
        connectionStatus === 'failed'
          ? '连接不可用，无法搜索对象'
          : ''
      "
      :has-connections="connections.length > 0"
      :command-key="isMacPlatform ? '⌘' : 'Ctrl+'"
      @close="closePalette(true)"
      @execute="executePaletteItem"
      @clear-recent="clearPaletteRecentItems"
    />

    <div
      v-if="aboutOpen"
      class="modal-backdrop"
      @click.self="aboutOpen = false"
    >
      <div
        class="modal confirm-modal about-modal"
        role="dialog"
        aria-modal="true"
        aria-label="关于 mydb"
      >
        <h2>关于 mydb</h2>
        <p class="about-version">版本 {{ packageInfo.version }}</p>
        <p class="about-author">作者：蒙其同学</p>
        <p class="about-github">
          GitHub：
          <button
            class="link-button"
            @click="openAuthorGithub"
          >
            github.com/mengqi19990203-creator
          </button>
        </p>
        <p class="about-email">
          联系邮箱：
          <button
            class="link-button"
            @click="copyContactEmail"
          >
            jianglichun996@163.com
          </button>
        </p>
        <p class="about-desc">轻量、跨平台的数据库客户端</p>
        <p class="about-tech">
          Tauri 2 · Rust · Vue 3 · SQLite / MySQL / PostgreSQL / Redis
        </p>
        <div class="confirm-actions">
          <button class="button ghost" @click="openProjectRepo">
            项目仓库
          </button>
          <button class="button primary" @click="aboutOpen = false">
            关闭
          </button>
        </div>
      </div>
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
      :cancel-text="confirmRequest?.cancelText"
      :extra-text="confirmRequest?.extraText"
      :danger="confirmRequest?.danger !== false"
      @confirm="runConfirmed"
      @extra="runConfirmExtra"
      @cancel="confirmRequest = null"
    />

    <CreateTableDialog
      :open="createOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      @close="createOpen = false"
      @created="handleTableCreated"
    />

    <CreateObjectDialog
      v-if="selected"
      :open="createObjectOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :kind="createObjectKind"
      :scope-label="connectionChipScope || selected.name"
      :tables="tables"
      @close="createObjectOpen = false"
      @created="handleObjectCreated"
    />

    <ConnectionTransferDialog
      :open="connectionTransferOpen"
      :connections="connections"
      :groups="connectionGroups"
      @close="connectionTransferOpen = false"
      @imported="handleConnectionsImported"
    />

    <GlobalSearchDialog
      v-if="selected"
      :open="globalSearchOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :scope-label="connectionChipScope || selected.name"
      @close="globalSearchOpen = false"
      @choose="chooseGlobalSearchHit"
    />

    <ImportWizardDialog
      :open="importOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :tables="tables"
      :initial-table="importTable"
      :read-only="Boolean(selected?.readOnly)"
      @close="importOpen = false"
      @imported="handleImportedData"
    />

    <DangerousSqlDialog
      :open="Boolean(dangerousSqlRequest)"
      :sql="dangerousSqlRequest?.sql ?? ''"
      :message="dangerousSqlRequest?.finding.message ?? ''"
      :high-risk="dangerousSqlRequest?.finding.highRisk ?? false"
      :connection-name="dangerousConnectionName"
      :scope="dangerousScope"
      @confirm="confirmDangerousSql"
      @cancel="cancelDangerousSql"
    />

    <SchemaSyncDialog
      :open="syncOpen"
      :connections="connections"
      :current-connection-id="selectedId ?? ''"
      @close="syncOpen = false"
      @applied="handleSchemaSyncApplied"
    />

    <DatabaseExportDialog
      v-if="databaseExportScope"
      :open="databaseExportOpen"
      :connection-id="databaseExportScope.connectionId"
      :driver="databaseExportScope.driver"
      :scope-label="databaseExportScope.scopeLabel"
      :database="databaseExportScope.database"
      :schema="databaseExportScope.schema"
      @close="databaseExportOpen = false"
      @exported="handleDatabaseExported($event.objectCount)"
    />

    <SqlFileExecuteDialog
      v-if="selected"
      :open="sqlScriptOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :scope-label="connectionChipScope || selected.name"
      :read-only="Boolean(selected?.readOnly)"
      :initial-path="sqlScriptInitialPath"
      @close="
        sqlScriptOpen = false;
        sqlScriptInitialPath = null;
      "
      @executed="handleSqlScriptExecuted"
    />

    <BackupSettingsDialog
      :open="backupSettingsOpen"
      :connections="connections"
      :default-connection-id="selectedId"
      @close="backupSettingsOpen = false"
      @restore="restoreBackupFile"
    />

    <SqlSnippetDialog
      :open="snippetOpen"
      :driver="selected?.driver ?? 'sqlite'"
      :initial-body="snippetInitialBody"
      @close="
        snippetOpen = false;
        snippetInitialBody = '';
      "
      @insert="insertSnippetText"
    />

    <SqlFormatSettingsDialog
      :open="formatSettingsOpen"
      @close="formatSettingsOpen = false"
      @saved="handleFormatSettingsSaved"
    />

    <SettingsDialog
      :open="settingsOpen"
      @close="settingsOpen = false"
      @open-format-settings="
        settingsOpen = false;
        formatSettingsOpen = true;
      "
    />

    <ObjectSourceDialog
      v-if="selected && objectSource"
      :open="Boolean(objectSource)"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :kind="objectSource.kind"
      :name="objectSource.name"
      :read-only="Boolean(selected?.readOnly)"
      @close="objectSource = null"
      @saved="handleObjectDefinitionSaved"
    />

    <CreateDatabaseDialog
      v-if="selected && (selected.driver === 'mysql' || selected.driver === 'postgres')"
      ref="createNamespaceRef"
      :open="createNamespaceOpen"
      :driver="selected.driver === 'mysql' ? 'mysql' : 'postgres'"
      :mode="createNamespaceMode"
      @close="createNamespaceOpen = false"
      @confirm="handleCreateNamespace"
    />

    <GenerateDataDialog
      v-if="generateTarget"
      :open="Boolean(generateTarget)"
      :connection-id="generateTarget.connectionId"
      :table="generateTarget.table"
      :table-kind="generateTarget.tableKind"
      :driver="driverForConnection(generateTarget.connectionId)"
      :read-only="Boolean(connectionFor(generateTarget.connectionId)?.readOnly)"
      @close="generateTarget = null"
      @generated="handleGeneratedData"
    />

    <QueryBuilderDialog
      v-if="selected"
      :open="queryBuilderOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :scope-label="connectionChipScope || selected.name"
      @close="queryBuilderOpen = false"
      @open-sql="
        openGeneratedSql({ sql: $event, title: '查询构建器' });
        queryBuilderOpen = false;
      "
      @save-snippet="
        openSnippetSave($event);
        queryBuilderOpen = false;
      "
    />

    <DatabaseDocsDialog
      v-if="selected"
      :open="databaseDocsOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :connection-name="selected.name"
      :database="activeDatabase ?? ''"
      :schema="activeSchema ?? ''"
      @close="databaseDocsOpen = false"
    />

    <ServerActivityDialog
      v-if="selected"
      :open="serverActivityOpen"
      :connection-id="selectedId ?? ''"
      :driver="selected?.driver ?? 'sqlite'"
      :scope-label="connectionChipScope || selected.name"
      :read-only="Boolean(selected?.readOnly)"
      @close="serverActivityOpen = false"
    />

    <DataTransferDialog
      :open="dataTransferOpen"
      :connections="connections"
      :current-connection-id="selectedId ?? ''"
      @close="dataTransferOpen = false"
      @transferred="handleTransferred"
    />

    <DataSyncDialog
      :open="dataSyncOpen"
      :connections="connections"
      :current-connection-id="selectedId ?? ''"
      :initial-table="activeTable?.kind === 'table' ? activeTable.name : ''"
      @close="dataSyncOpen = false"
    />

    <div
      v-if="tabMenu"
      ref="tabMenuRef"
      class="context-menu tab-context-menu"
      role="menu"
      :style="{
        position: 'fixed',
        left: `${tabMenu.x}px`,
        top: `${tabMenu.y}px`,
      }"
      @click.stop
    >
      <button
        class="context-menu-item"
        role="menuitem"
        @click="closeTabFromMenu"
      >
        <span>关闭标签</span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="togglePinnedTab"
      >
        <Icon name="pin" :size="13" />
        <span>
          {{ tabMenu && isTabPinned(tabMenu.tabId) ? "取消固定" : "固定标签" }}
        </span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="tabMenu && openSplitView(tabMenu.tabId)"
      >
        <Icon name="structure" :size="13" />
        <span>在右侧分屏打开</span>
      </button>
      <button
        v-if="tabMenuQueryTab"
        class="context-menu-item"
        role="menuitem"
        @click="tabMenu && openQueryInNewWindow(tabMenu.tabId)"
      >
        <Icon name="external" :size="13" />
        <span>在新窗口打开</span>
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        :disabled="!tabMenuCounts.other"
        @click="requestCloseTabs('other')"
      >
        <span>关闭其他标签</span>
        <span class="context-menu-shortcut">
          {{ tabMenuCounts.other }}
        </span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        :disabled="!tabMenuCounts.right"
        @click="requestCloseTabs('right')"
      >
        <span>关闭右侧标签</span>
        <span class="context-menu-shortcut">
          {{ tabMenuCounts.right }}
        </span>
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        :disabled="!tabMenuCounts.all"
        @click="requestCloseTabs('all')"
      >
        <span>关闭全部标签</span>
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="reopenFromTabMenu"
      >
        <span>重新打开关闭的标签</span>
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="copyTabMenuSource"
      >
        <span>复制来源</span>
      </button>
    </div>

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
