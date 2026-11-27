<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import AiSqlDialog from "./AiSqlDialog.vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import MongoCollectionStatsDialog from "./MongoCollectionStatsDialog.vue";
import MongoConsole from "./MongoConsole.vue";
import MongoCreateDatabaseDialog from "./MongoCreateDatabaseDialog.vue";
import MongoDropDatabaseDialog from "./MongoDropDatabaseDialog.vue";
import MongoGridFsPanel from "./MongoGridFsPanel.vue";
import MongoRenameCollectionDialog from "./MongoRenameCollectionDialog.vue";
import MongoUsersDialog from "./MongoUsersDialog.vue";
import PromptDialog from "./PromptDialog.vue";
import * as api from "../lib/api";
import { detectDelimiter, parseDelimitedText } from "../lib/clipboard-table";
import type {
  MongoCollectionInfo,
  MongoDocumentPage,
  MongoIndexInfo,
} from "../lib/types";

const props = defineProps<{
  connectionId: string;
  connectionName?: string;
  database?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "database-change": [database: string];
  status: [message: string];
  "open-settings": [];
}>();

const databases = ref<string[]>([]);
const database = ref("");
const databaseLoading = ref(false);
const collections = ref<MongoCollectionInfo[]>([]);
const collectionSearch = ref("");
const collectionsLoading = ref(false);
const collectionsError = ref("");
const selectedCollection = ref<MongoCollectionInfo | null>(null);
const mode = ref<"documents" | "console" | "aggregation" | "indexes" | "gridfs">("documents");
const vFocus = {
  mounted: (element: HTMLInputElement) => element.focus(),
};
interface PendingMongoDocument {
  id: unknown;
  updates: Record<string, unknown>;
  deleted: boolean;
}
const pendingMongo = reactive<Record<string, PendingMongoDocument>>({});
const gridEdit = ref<{ key: string; field: string; value: string } | null>(null);
const pendingPreviewOpen = ref(false);
const pendingSaving = ref(false);
const pendingError = ref("");

const filterDraft = ref("{}");
const projectionDraft = ref("");
const sortDraft = ref("");
const limit = ref(50);
const skip = ref(0);
const page = ref<MongoDocumentPage | null>(null);
const docsLoading = ref(false);
const docsError = ref("");
const elapsedMs = ref(0);
const aggregationPipeline = ref(`[
  { "$match": {} },
  { "$limit": 50 }
]`);
const aggregationPage = ref<MongoDocumentPage | null>(null);
const aggregationLoading = ref(false);
const aggregationError = ref("");
const aggregationElapsedMs = ref(0);
const importing = ref(false);
const importPending = ref<{ documents: string; count: number; format: string } | null>(null);

const indexes = ref<MongoIndexInfo[]>([]);
const indexesLoading = ref(false);
const indexesError = ref("");

const docDialogOpen = ref(false);
const docDialogMode = ref<"view" | "insert" | "edit">("view");
const docDraft = ref("");
const docOriginal = ref<Record<string, unknown> | null>(null);
const docSaving = ref(false);
const docError = ref("");

const deleteDoc = ref<Record<string, unknown> | null>(null);
const deleteCollection = ref<MongoCollectionInfo | null>(null);
const createCollectionOpen = ref(false);
const collectionMenu = ref<{
  x: number;
  y: number;
  item: MongoCollectionInfo;
} | null>(null);
const databaseMenu = ref<{ x: number; y: number } | null>(null);
const createDatabaseOpen = ref(false);
const createDatabaseBusy = ref(false);
const createDatabaseError = ref("");
const dropDatabaseOpen = ref(false);
const dropDatabaseCount = ref<number | null>(null);
const dropDatabaseBusy = ref(false);
const dropDatabaseError = ref("");
const renameTarget = ref<MongoCollectionInfo | null>(null);
const renameBusy = ref(false);
const renameError = ref("");
const statsTarget = ref<MongoCollectionInfo | null>(null);
const usersOpen = ref(false);
const consoleRef = ref<InstanceType<typeof MongoConsole> | null>(null);
const aiOpen = ref(false);
const mongoAiSql = ref("");
const pendingMongoAi = ref<{ text: string; replace: boolean } | null>(null);
const indexDialogOpen = ref(false);
const indexKeys = ref('{ "field": 1 }');
const indexName = ref("");
const indexUnique = ref(false);
const dropIndexName = ref("");
const toast = ref("");
let toastTimer = 0;

const filteredCollections = computed(() => {
  const search = collectionSearch.value.trim().toLocaleLowerCase();
  if (!search) return collections.value;
  return collections.value.filter((collection) =>
    collection.name.toLocaleLowerCase().includes(search),
  );
});

const displayPage = computed(() =>
  mode.value === "aggregation" ? aggregationPage.value : page.value,
);
const displayLoading = computed(() =>
  mode.value === "aggregation" ? aggregationLoading.value : docsLoading.value,
);
const displayError = computed(() =>
  mode.value === "aggregation" ? aggregationError.value : docsError.value,
);
const displayElapsedMs = computed(() =>
  mode.value === "aggregation" ? aggregationElapsedMs.value : elapsedMs.value,
);

const fields = computed(() => {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const document of displayPage.value?.documents ?? []) {
    for (const key of Object.keys(document)) {
      if (seen.has(key)) continue;
      seen.add(key);
      result.push(key);
    }
  }
  result.sort((left, right) => {
    if (left === "_id") return -1;
    if (right === "_id") return 1;
    return left.localeCompare(right, "zh-CN");
  });
  return result;
});

const pageCount = computed(() => {
  const total = page.value?.total ?? 0;
  return Math.max(1, Math.ceil(total / limit.value));
});
const currentPage = computed(() =>
  Math.min(pageCount.value, Math.floor(skip.value / limit.value) + 1),
);
const hasPrevious = computed(() => skip.value > 0);
const hasNext = computed(
  () => (page.value?.total ?? 0) > skip.value + (page.value?.documents.length ?? 0),
);
const selectedName = computed(() => selectedCollection.value?.name ?? "");
const canEdit = computed(() => !props.readOnly);
const mongoAiContext = computed(() => {
  const collectionNames = collections.value.map((item) => item.name).slice(0, 100);
  return [
    `数据库：${database.value || "—"}`,
    `集合：${collectionNames.join(", ") || "—"}`,
    selectedCollection.value
      ? `当前集合：${selectedCollection.value.name}`
      : "",
    fields.value.length ? `已知字段：${fields.value.slice(0, 120).join(", ")}` : "",
    "不要读取或发送文档内容；只根据集合 / 字段结构生成命令。",
  ]
    .filter(Boolean)
    .join("\n");
});

function openMongoAi() {
  mongoAiSql.value = consoleRef.value?.currentScript() ?? "";
  aiOpen.value = true;
}

async function applyMongoAi(text: string, replace: boolean) {
  if (mode.value !== "console") {
    pendingMongoAi.value = { text, replace };
    await switchMode("console");
    await nextTick();
    // The console mounts its CodeMirror view on its own next tick; wait for
    // the ready event. Fall back to one more tick for already-mounted views.
    if (!flushPendingMongoAi()) {
      await nextTick();
      flushPendingMongoAi();
    }
    return;
  }
  const applied = replace
    ? consoleRef.value?.replaceScript(text)
    : consoleRef.value?.insertScript(text);
  if (!applied) {
    pendingMongoAi.value = { text, replace };
    flashToast("正在打开 MongoDB 查询控制台…");
  }
}

function insertMongoAi(text: string) {
  void applyMongoAi(text, false);
}

function replaceMongoAi(text: string) {
  void applyMongoAi(text, true);
}

function flushPendingMongoAi(): boolean {
  const pending = pendingMongoAi.value;
  if (!pending || !consoleRef.value) return false;
  const applied = pending.replace
    ? consoleRef.value.replaceScript(pending.text)
    : consoleRef.value.insertScript(pending.text);
  if (applied) {
    pendingMongoAi.value = null;
    flashToast(pending.replace ? "已替换 MongoDB 控制台命令" : "已插入 MongoDB 控制台命令");
  }
  return applied;
}
const isSystemDatabase = computed(() =>
  ["admin", "local", "config"].includes(database.value),
);

function flashToast(message: string) {
  toast.value = message;
  emit("status", message);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 2200);
}

function normalizeDatabase(value: string | null | undefined): string {
  return (value ?? "").trim();
}

async function loadDatabases() {
  databaseLoading.value = true;
  try {
    const result = await api.mongoListDatabases(props.connectionId);
    databases.value = result;
    const preferred = normalizeDatabase(props.database);
    if (preferred) {
      // The URI / form database may not exist yet; MongoDB creates it on the
      // first write, so keep it instead of silently switching to another DB.
      database.value = preferred;
      if (!result.includes(preferred)) {
        databases.value = [preferred, ...result];
      }
    } else if (!result.includes(database.value)) {
      database.value = result[0] ?? "";
    }
    if (!result.length) {
      database.value = "";
    }
    if (database.value) emit("database-change", database.value);
  } catch (reason) {
    docsError.value = String(reason);
    databases.value = [];
    database.value = "";
  } finally {
    databaseLoading.value = false;
    if (database.value) await loadCollections();
  }
}

async function loadCollections() {
  if (!database.value) {
    collections.value = [];
    selectedCollection.value = null;
    page.value = null;
    return;
  }
  collectionsLoading.value = true;
  collectionsError.value = "";
  try {
    collections.value = await api.mongoListCollections(
      props.connectionId,
      database.value,
    );
    if (
      selectedCollection.value &&
      !collections.value.some(
        (item) => item.name === selectedCollection.value?.name,
      )
    ) {
      selectedCollection.value = null;
      page.value = null;
      indexes.value = [];
    }
  } catch (reason) {
    collections.value = [];
    selectedCollection.value = null;
    collectionsError.value = String(reason);
  } finally {
    collectionsLoading.value = false;
  }
}

function changeDatabase(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  discardPendingMongo();
  database.value = value;
  selectedCollection.value = null;
  page.value = null;
  indexes.value = [];
  skip.value = 0;
  emit("database-change", value);
  void loadCollections();
}

async function handleConsoleDatabaseChange(next: string) {
  if (next === database.value) return;
  discardPendingMongo();
  database.value = next;
  selectedCollection.value = null;
  page.value = null;
  aggregationPage.value = null;
  indexes.value = [];
  skip.value = 0;
  emit("database-change", next);
  await loadCollections();
}

async function refreshAfterConsoleWrite() {
  await loadCollections();
}

function closeManagementMenus() {
  collectionMenu.value = null;
  databaseMenu.value = null;
}

function toggleDatabaseMenu(event: MouseEvent) {
  if (databaseMenu.value) {
    databaseMenu.value = null;
    return;
  }
  const target = event.currentTarget as HTMLElement | null;
  const rect = target?.getBoundingClientRect();
  collectionMenu.value = null;
  databaseMenu.value = {
    x: Math.max(8, (rect?.right ?? 260) - 200),
    y: (rect?.bottom ?? 60) + 4,
  };
}

function openCollectionMenu(event: MouseEvent, item: MongoCollectionInfo) {
  databaseMenu.value = null;
  collectionMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 190)),
    item,
  };
}

function openCreateDatabase() {
  closeManagementMenus();
  createDatabaseError.value = "";
  createDatabaseOpen.value = true;
}

async function refreshDatabaseList() {
  try {
    const result = await api.mongoListDatabases(props.connectionId);
    databases.value = result;
    if (database.value && !result.includes(database.value)) {
      database.value = result[0] ?? "";
      selectedCollection.value = null;
      page.value = null;
      emit("database-change", database.value);
      await loadCollections();
    }
  } catch (reason) {
    docsError.value = String(reason);
  }
}

function openDropDatabase() {
  closeManagementMenus();
  dropDatabaseError.value = "";
  dropDatabaseCount.value = null;
  dropDatabaseOpen.value = true;
  void api
    .mongoListCollections(props.connectionId, database.value)
    .then((items) => {
      dropDatabaseCount.value = items.length;
    })
    .catch(() => {
      dropDatabaseCount.value = null;
    });
}

function openRenameCollection(item: MongoCollectionInfo) {
  closeManagementMenus();
  if (props.readOnly || item.kind !== "collection") return;
  renameError.value = "";
  renameTarget.value = item;
}

function openCollectionStats(item: MongoCollectionInfo) {
  closeManagementMenus();
  statsTarget.value = item;
}

async function confirmCreateDatabase(databaseName: string, firstCollection: string) {
  createDatabaseBusy.value = true;
  createDatabaseError.value = "";
  try {
    await api.mongoCreateDatabase(
      props.connectionId,
      databaseName,
      firstCollection,
    );
    createDatabaseOpen.value = false;
    const result = await api.mongoListDatabases(props.connectionId);
    databases.value = result.includes(databaseName)
      ? result
      : [databaseName, ...result];
    database.value = databaseName;
    selectedCollection.value = null;
    page.value = null;
    emit("database-change", databaseName);
    await loadCollections();
    const created = collections.value.find((item) => item.name === firstCollection);
    if (created) {
      await selectCollection(created);
    }
    flashToast(`已创建数据库 ${databaseName} 和集合 ${firstCollection}`);
  } catch (reason) {
    createDatabaseError.value = String(reason);
  } finally {
    createDatabaseBusy.value = false;
  }
}

async function confirmDropDatabase(confirmName: string) {
  dropDatabaseBusy.value = true;
  dropDatabaseError.value = "";
  try {
    await api.mongoDropDatabase(
      props.connectionId,
      database.value,
      confirmName,
    );
    const dropped = database.value;
    dropDatabaseOpen.value = false;
    const result = await api.mongoListDatabases(props.connectionId);
    databases.value = result;
    database.value = result[0] ?? "";
    selectedCollection.value = null;
    page.value = null;
    aggregationPage.value = null;
    indexes.value = [];
    emit("database-change", database.value);
    await loadCollections();
    flashToast(`已删除数据库 ${dropped}`);
  } catch (reason) {
    dropDatabaseError.value = String(reason);
  } finally {
    dropDatabaseBusy.value = false;
  }
}

async function confirmRenameCollection(to: string) {
  const source = renameTarget.value;
  if (!source || !database.value) return;
  renameBusy.value = true;
  renameError.value = "";
  try {
    await api.mongoRenameCollection(
      props.connectionId,
      database.value,
      source.name,
      to,
    );
    renameTarget.value = null;
    await loadCollections();
    const renamed = collections.value.find((item) => item.name === to);
    if (renamed) {
      await selectCollection(renamed);
    }
    flashToast(`已重命名集合 ${source.name} → ${to}`);
  } catch (reason) {
    renameError.value = String(reason);
  } finally {
    renameBusy.value = false;
  }
}

async function selectCollection(collection: MongoCollectionInfo) {
  discardPendingMongo();
  selectedCollection.value = collection;
  mode.value = "documents";
  skip.value = 0;
  filterDraft.value = "{}";
  projectionDraft.value = "";
  sortDraft.value = "";
  page.value = null;
  aggregationPage.value = null;
  aggregationError.value = "";
  indexes.value = [];
  await loadDocuments(true);
}

function parseFilter(): string | null {
  try {
    const value = JSON.parse(filterDraft.value.trim() || "{}");
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      docsError.value = "筛选条件必须是 JSON 对象";
      return null;
    }
    return filterDraft.value.trim() || "{}";
  } catch (reason) {
    docsError.value = `筛选条件不是合法 JSON：${reason}`;
    return null;
  }
}

async function loadDocuments(reset = false) {
  if (!selectedCollection.value || !database.value) return;
  if (reset) skip.value = 0;
  const filter = parseFilter();
  if (filter === null) return;
  docsLoading.value = true;
  docsError.value = "";
  try {
    const result = await api.mongoFindDocuments(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      filter,
      projectionDraft.value.trim(),
      sortDraft.value.trim(),
      skip.value,
      limit.value,
    );
    page.value = result;
    elapsedMs.value = result.elapsedMs;
    if (result.documents.length === 0 && skip.value > 0) {
      skip.value = Math.max(0, skip.value - limit.value);
      docsLoading.value = false;
      await loadDocuments();
      return;
    }
  } catch (reason) {
    page.value = null;
    docsError.value = String(reason);
  } finally {
    docsLoading.value = false;
  }
}

function resetFilters() {
  filterDraft.value = "{}";
  projectionDraft.value = "";
  sortDraft.value = "";
  skip.value = 0;
  void loadDocuments(true);
}

function goPage(direction: -1 | 1) {
  const next = skip.value + direction * limit.value;
  if (next < 0) return;
  skip.value = next;
  void loadDocuments();
}

function setLimit(event: Event) {
  limit.value = Number((event.target as HTMLSelectElement).value) || 50;
  skip.value = 0;
  void loadDocuments(true);
}

async function runAggregation() {
  if (!selectedCollection.value || !database.value) return;
  aggregationLoading.value = true;
  aggregationError.value = "";
  try {
    const result = await api.mongoAggregateDocuments(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      aggregationPipeline.value,
      500,
    );
    aggregationPage.value = result;
    aggregationElapsedMs.value = result.elapsedMs;
  } catch (reason) {
    aggregationPage.value = null;
    aggregationError.value = String(reason);
  } finally {
    aggregationLoading.value = false;
  }
}

function escapeCsv(value: string): string {
  if (/[",\r\n]/.test(value)) return `"${value.replace(/"/g, '""')}"`;
  return value;
}

function exportCell(value: unknown): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

async function collectExportDocuments(): Promise<Record<string, unknown>[]> {
  if (!selectedCollection.value || !database.value) return [];
  if (mode.value === "aggregation") {
    return (aggregationPage.value?.documents ?? []).slice(0, 10_000);
  }
  const filterText = parseFilter();
  if (filterText === null) return [];
  const all: Record<string, unknown>[] = [];
  let offset = 0;
  while (all.length < 10_000) {
    const result = await api.mongoFindDocuments(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      filterText,
      projectionDraft.value.trim(),
      sortDraft.value.trim(),
      offset,
      1000,
    );
    all.push(...result.documents);
    if (
      result.documents.length === 0 ||
      all.length >= result.total ||
      result.documents.length < 1000
    ) {
      break;
    }
    offset += result.documents.length;
  }
  return all.slice(0, 10_000);
}

async function exportDocuments() {
  if (!selectedCollection.value || !database.value) return;
  let documents: Record<string, unknown>[];
  try {
    documents = await collectExportDocuments();
  } catch (reason) {
    docsError.value = String(reason);
    return;
  }
  if (!documents.length) {
    flashToast("没有可导出的文档");
    return;
  }
  const path = await saveDialog({
    defaultPath: `${selectedCollection.value.name}.json`,
    filters: [
      { name: "JSON", extensions: ["json"] },
      { name: "CSV", extensions: ["csv"] },
    ],
  });
  if (typeof path !== "string") return;
  const isCsv = path.toLocaleLowerCase().endsWith(".csv");
  let contents: string;
  if (isCsv) {
    const keys: string[] = [];
    const seen = new Set<string>();
    for (const document of documents) {
      for (const key of Object.keys(document)) {
        if (seen.has(key)) continue;
        seen.add(key);
        keys.push(key);
      }
    }
    keys.sort((left, right) => {
      if (left === "_id") return -1;
      if (right === "_id") return 1;
      return left.localeCompare(right, "zh-CN");
    });
    const lines = [keys.map(escapeCsv).join(",")];
    for (const document of documents) {
      lines.push(
        keys
          .map((key) => escapeCsv(exportCell(document[key])))
          .join(","),
      );
    }
    contents = `\ufeff${lines.join("\r\n")}`;
  } else {
    contents = JSON.stringify(documents, null, 2);
  }
  try {
    await api.writeTextFile(path, contents);
    flashToast(
      `已导出 ${documents.length} 个文档到 ${path}${documents.length >= 10_000 ? "（上限 10,000）" : ""}`,
    );
  } catch (reason) {
    docsError.value = String(reason);
  }
}

function coerceCsvValue(value: string): unknown {
  if (value === "") return null;
  if (value === "true") return true;
  if (value === "false") return false;
  if (value === "null") return null;
  if (/^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(value.trim())) {
    return Number(value);
  }
  return value;
}

async function importDocuments() {
  if (!selectedCollection.value || !database.value || !canEdit.value) return;
  const path = await openDialog({
    multiple: false,
    filters: [
      { name: "MongoDB 文档", extensions: ["json", "csv"] },
    ],
  });
  if (typeof path !== "string") return;
  let text: string;
  try {
    text = await api.readTextFile(path);
  } catch (reason) {
    docsError.value = String(reason);
    return;
  }
  let documents: Record<string, unknown>[] = [];
  const format = path.toLocaleLowerCase().endsWith(".csv") ? "CSV" : "JSON";
  try {
    if (format === "CSV") {
      const rows = parseDelimitedText(text, detectDelimiter(text));
      if (rows.length < 2) {
        throw new Error("CSV 至少需要表头和一行数据");
      }
      const headers = rows[0].map((header, index) => header.trim() || `column_${index + 1}`);
      documents = rows.slice(1).filter((row) => row.some((value) => value !== "")).map((row) => {
        const document: Record<string, unknown> = {};
        headers.forEach((header, index) => {
          document[header] = coerceCsvValue(row[index] ?? "");
        });
        return document;
      });
    } else {
      try {
        const parsed = JSON.parse(text);
        documents = Array.isArray(parsed) ? parsed : [parsed];
      } catch {
        documents = text
          .split(/\r?\n/)
          .map((line) => line.trim())
          .filter(Boolean)
          .map((line) => JSON.parse(line));
      }
      if (!documents.every((item) => item && typeof item === "object" && !Array.isArray(item))) {
        throw new Error("JSON 导入必须由对象或对象数组组成");
      }
    }
  } catch (reason) {
    docsError.value = `导入解析失败：${reason}`;
    return;
  }
  if (!documents.length) {
    docsError.value = "没有可导入的文档";
    return;
  }
  if (documents.length > 10_000) {
    docsError.value = "单次最多导入 10,000 个文档";
    return;
  }
  importPending.value = {
    documents: JSON.stringify(documents),
    count: documents.length,
    format,
  };
}

async function confirmImportDocuments() {
  if (importing.value) return;
  const pending = importPending.value;
  if (!pending || !selectedCollection.value || !database.value) return;
  importing.value = true;
  try {
    const count = await api.mongoInsertDocuments(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      pending.documents,
    );
    importPending.value = null;
    flashToast(`已导入 ${count} 个文档`);
    await loadDocuments(true);
  } catch (reason) {
    docsError.value = String(reason);
  } finally {
    importing.value = false;
  }
}

async function loadIndexes() {
  if (!selectedCollection.value || !database.value) return;
  indexesLoading.value = true;
  indexesError.value = "";
  try {
    indexes.value = await api.mongoListIndexes(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
    );
  } catch (reason) {
    indexes.value = [];
    indexesError.value = String(reason);
  } finally {
    indexesLoading.value = false;
  }
}

async function switchMode(
  next: "documents" | "console" | "aggregation" | "indexes" | "gridfs",
) {
  mode.value = next;
  if (next === "indexes") await loadIndexes();
}

function formatDocument(document: Record<string, unknown>): string {
  return JSON.stringify(document, null, 2);
}

function cellText(document: Record<string, unknown>, field: string): string {
  const value = cellValue(document, field);
  if (value === undefined) return "";
  if (value === null) return "NULL";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function documentKey(document: Record<string, unknown>): string {
  if (document._id === undefined) return "";
  try {
    return JSON.stringify(document._id);
  } catch {
    return String(document._id);
  }
}

function pendingDocument(document: Record<string, unknown>): PendingMongoDocument | null {
  const key = documentKey(document);
  return key ? pendingMongo[key] ?? null : null;
}

function cellValue(document: Record<string, unknown>, field: string): unknown {
  const pending = pendingDocument(document);
  if (pending && Object.prototype.hasOwnProperty.call(pending.updates, field)) {
    return pending.updates[field];
  }
  return document[field];
}

const canEditGrid = computed(
  () =>
    canEdit.value &&
    mode.value === "documents" &&
    selectedCollection.value?.kind === "collection" &&
    !projectionDraft.value.trim(),
);

function pendingItems(): PendingMongoDocument[] {
  return Object.values(pendingMongo) as PendingMongoDocument[];
}

const pendingCount = computed(() =>
  pendingItems().reduce(
    (total, item) => total + (item.deleted ? 1 : Object.keys(item.updates).length),
    0,
  ),
);

function pendingPreview(): string {
  if (!selectedCollection.value) return "";
  const name = selectedCollection.value.name.replaceAll('"', '\\"');
  const lines: string[] = [];
  for (const item of pendingItems()) {
    const id = JSON.stringify(item.id);
    if (item.deleted) {
      lines.push(
        `db.getCollection("${name}").deleteOne({ "_id": ${id} })`,
      );
      continue;
    }
    if (!Object.keys(item.updates).length) continue;
    const set = JSON.stringify(item.updates);
    lines.push(
      `db.getCollection("${name}").updateOne({ "_id": ${id} }, { "$set": ${set} })`,
    );
  }
  return lines.join("\n");
}

function startGridEdit(document: Record<string, unknown>, field: string) {
  if (!canEditGrid.value || field === "_id") return;
  const pending = pendingDocument(document);
  if (pending?.deleted) return;
  const value = cellValue(document, field);
  if (value !== null && typeof value === "object") return;
  const key = documentKey(document);
  if (!key) return;
  gridEdit.value = {
    key,
    field,
    value: value === null || value === undefined ? "" : String(value),
  };
}

function parseGridInput(text: string, original: unknown): unknown {
  if (text === "") return null;
  if (typeof original === "number") {
    const number = Number(text);
    return Number.isFinite(number) ? number : text;
  }
  if (typeof original === "boolean") {
    if (text.toLowerCase() === "true") return true;
    if (text.toLowerCase() === "false") return false;
    return text;
  }
  return text;
}

function commitGridEdit() {
  const edit = gridEdit.value;
  gridEdit.value = null;
  if (!edit) return;
  const document = displayPage.value?.documents.find(
    (item) => documentKey(item) === edit.key,
  );
  if (!document) return;
  const pending = pendingMongo[edit.key];
  const original =
    pending && Object.prototype.hasOwnProperty.call(pending.updates, edit.field)
      ? pending.updates[edit.field]
      : document[edit.field];
  const value = parseGridInput(edit.value, original);
  if (value === original || (value === null && original === null)) {
    if (pending) {
      delete pending.updates[edit.field];
      if (!Object.keys(pending.updates).length && !pending.deleted) {
        delete pendingMongo[edit.key];
      }
    }
    return;
  }
  const next = pending ?? { id: document._id, updates: {}, deleted: false };
  next.updates[edit.field] = value;
  pendingMongo[edit.key] = next;
}

function togglePendingDelete(document: Record<string, unknown>) {
  if (!canEditGrid.value) return;
  const key = documentKey(document);
  if (!key) return;
  const existing = pendingMongo[key];
  if (existing?.deleted) {
    delete pendingMongo[key];
    return;
  }
  pendingMongo[key] = { id: document._id, updates: {}, deleted: true };
  if (gridEdit.value?.key === key) gridEdit.value = null;
}

function discardPendingMongo() {
  for (const key of Object.keys(pendingMongo)) delete pendingMongo[key];
  gridEdit.value = null;
  pendingError.value = "";
}

async function commitPendingMongo() {
  if (!selectedCollection.value || !database.value || pendingSaving.value) return;
  pendingPreviewOpen.value = false;
  pendingSaving.value = true;
  pendingError.value = "";
  let changed = 0;
  try {
    for (const [key, item] of Object.entries({ ...pendingMongo }) as Array<
      [string, PendingMongoDocument]
    >) {
      if (item.deleted) {
        await api.mongoDeleteDocument(
          props.connectionId,
          database.value,
          selectedCollection.value.name,
          item.id,
        );
      } else if (Object.keys(item.updates).length) {
        await api.mongoUpdateDocumentFields(
          props.connectionId,
          database.value,
          selectedCollection.value.name,
          item.id,
          item.updates,
        );
      }
      delete pendingMongo[key];
      changed += 1;
    }
    flashToast(`已提交 ${changed} 项 MongoDB 变更`);
    await loadDocuments();
  } catch (reason) {
    pendingError.value = String(reason);
    docsError.value = pendingError.value;
    await loadDocuments();
  } finally {
    pendingSaving.value = false;
  }
}

function openInsert() {
  if (!canEdit.value || !selectedCollection.value) return;
  docDialogMode.value = "insert";
  docOriginal.value = null;
  docDraft.value = '{\n  \n}';
  docError.value = "";
  docDialogOpen.value = true;
}

function openDocument(document: Record<string, unknown>, edit: boolean) {
  docDialogMode.value = edit && canEdit.value ? "edit" : "view";
  docOriginal.value = document;
  docDraft.value = formatDocument(document);
  docError.value = "";
  docDialogOpen.value = true;
}

function beginEditDocument() {
  if (!canEdit.value) return;
  docDialogMode.value = "edit";
  docError.value = "";
}

async function saveDocument() {
  if (!selectedCollection.value || !database.value) return;
  let parsed: unknown;
  try {
    parsed = JSON.parse(docDraft.value);
  } catch (reason) {
    docError.value = `文档不是合法 JSON：${reason}`;
    return;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    docError.value = "文档必须是 JSON 对象";
    return;
  }
  docSaving.value = true;
  docError.value = "";
  try {
    if (docDialogMode.value === "insert") {
      await api.mongoInsertDocument(
        props.connectionId,
        database.value,
        selectedCollection.value.name,
        docDraft.value,
      );
      flashToast("已插入文档");
    } else {
      const id = docOriginal.value?._id;
      if (id === undefined) {
        docError.value = "文档缺少 _id，无法替换";
        return;
      }
      const changed = await api.mongoReplaceDocument(
        props.connectionId,
        database.value,
        selectedCollection.value.name,
        id,
        docDraft.value,
      );
      flashToast(changed ? "已保存文档" : "文档内容未变化");
    }
    docDialogOpen.value = false;
    await loadDocuments();
  } catch (reason) {
    docError.value = String(reason);
  } finally {
    docSaving.value = false;
  }
}

function askDeleteDocument(document: Record<string, unknown>) {
  if (!canEdit.value) return;
  docDialogOpen.value = false;
  deleteDoc.value = document;
}

async function confirmDeleteDocument() {
  const document = deleteDoc.value;
  deleteDoc.value = null;
  if (!document || !selectedCollection.value || !database.value) return;
  try {
    await api.mongoDeleteDocument(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      document._id,
    );
    flashToast("已删除文档");
    await loadDocuments();
  } catch (reason) {
    docsError.value = String(reason);
  }
}

async function createCollection(name: string, _checked: boolean) {
  createCollectionOpen.value = false;
  if (!database.value) return;
  try {
    await api.mongoCreateCollection(props.connectionId, database.value, name);
    flashToast(`已创建集合 ${name}`);
    await loadCollections();
  } catch (reason) {
    docsError.value = String(reason);
  }
}

async function confirmDropCollection() {
  const collection = deleteCollection.value;
  deleteCollection.value = null;
  if (!collection || !database.value) return;
  try {
    await api.mongoDropCollection(
      props.connectionId,
      database.value,
      collection.name,
    );
    selectedCollection.value = null;
    page.value = null;
    flashToast(`已删除集合 ${collection.name}`);
    await loadCollections();
  } catch (reason) {
    docsError.value = String(reason);
  }
}

async function createIndex() {
  if (!selectedCollection.value || !database.value) return;
  try {
    await api.mongoCreateIndex(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      indexKeys.value,
      indexName.value,
      indexUnique.value,
    );
    indexDialogOpen.value = false;
    indexName.value = "";
    indexUnique.value = false;
    flashToast("已创建索引");
    await loadIndexes();
  } catch (reason) {
    indexesError.value = String(reason);
  }
}

async function confirmDropIndex() {
  const name = dropIndexName.value;
  dropIndexName.value = "";
  if (!name || !selectedCollection.value || !database.value) return;
  try {
    await api.mongoDropIndex(
      props.connectionId,
      database.value,
      selectedCollection.value.name,
      name,
    );
    flashToast("已删除索引");
    await loadIndexes();
  } catch (reason) {
    indexesError.value = String(reason);
  }
}

watch(
  () => props.database,
  (value) => {
    const preferred = normalizeDatabase(value);
    if (!preferred || preferred === database.value) return;
    database.value = preferred;
    void loadCollections();
  },
);

onMounted(() => {
  document.addEventListener("click", closeManagementMenus);
  document.addEventListener("contextmenu", closeManagementMenus);
  void loadDatabases();
});

onBeforeUnmount(() => {
  document.removeEventListener("click", closeManagementMenus);
  document.removeEventListener("contextmenu", closeManagementMenus);
  window.clearTimeout(toastTimer);
});
</script>

<template>
  <section class="mongo-workspace">
    <aside class="mongo-sidebar">
      <header class="mongo-sidebar-head">
        <span class="mongo-sidebar-title">MongoDB</span>
        <button
          class="icon-button"
          title="刷新数据库与集合"
          aria-label="刷新数据库与集合"
          :disabled="databaseLoading || collectionsLoading"
          @click="loadDatabases"
        >
          <Icon name="refresh" :size="13" />
        </button>
      </header>
      <div class="mongo-database-row">
        <Icon name="database" :size="14" />
        <select
          :value="database"
          :disabled="databaseLoading || !databases.length"
          aria-label="MongoDB 数据库"
          @change="changeDatabase"
        >
          <option v-if="!databases.length" value="">无数据库</option>
          <option v-for="item in databases" :key="item" :value="item">
            {{ item }}
          </option>
        </select>
        <button
          class="icon-button"
          title="数据库操作"
          aria-label="数据库操作"
          @click.stop="toggleDatabaseMenu"
        >
          <Icon name="more" :size="13" />
        </button>
      </div>
      <div class="mongo-collection-search">
        <Icon name="search" :size="12" />
        <input
          v-model="collectionSearch"
          placeholder="搜索集合"
          aria-label="搜索 MongoDB 集合"
        />
      </div>
      <div class="mongo-collection-actions">
        <span>{{ filteredCollections.length }} 个集合</span>
        <button
          class="icon-button"
          title="新建集合"
          aria-label="新建集合"
          :disabled="!database || !canEdit"
          @click="createCollectionOpen = true"
        >
          <Icon name="plus" :size="13" />
        </button>
      </div>
      <ul class="mongo-collection-list" role="list">
        <li v-if="databaseLoading || collectionsLoading" class="mongo-list-hint">
          读取中…
        </li>
        <li
          v-else-if="collectionsError"
          class="mongo-list-hint warning"
        >
          {{ collectionsError }}
        </li>
        <li
          v-else-if="!filteredCollections.length"
          class="mongo-list-hint"
        >
          没有集合
        </li>
        <li v-for="item in filteredCollections" :key="item.name">
          <button
            class="mongo-collection-row"
            :class="{ active: selectedCollection?.name === item.name }"
            @click="selectCollection(item)"
            @contextmenu.prevent.stop="openCollectionMenu($event, item)"
          >
            <Icon
              :name="item.kind === 'view' ? 'view' : 'table'"
              :size="12"
            />
            <span class="mongo-collection-name" :title="item.name">
              {{ item.name }}
            </span>
            <span v-if="item.kind !== 'collection'" class="mongo-kind">
              {{ item.kind }}
            </span>
          </button>
        </li>
      </ul>
      <footer class="mongo-sidebar-foot">
        <span v-if="readOnly">只读连接 · 右键可查看统计</span>
        <span v-else>右键集合可管理</span>
        <span>{{ connectionName }}</span>
      </footer>
    </aside>

    <div
      v-if="databaseMenu"
      class="context-menu mongo-management-menu"
      :style="{ left: `${databaseMenu.x}px`, top: `${databaseMenu.y}px` }"
      @click.stop
    >
      <button
        class="context-menu-item"
        :disabled="readOnly"
        @click="openCreateDatabase"
      >
        <Icon name="plus" :size="12" /> 新建数据库…
      </button>
      <button
        class="context-menu-item danger"
        :disabled="readOnly || isSystemDatabase || !database"
        @click="openDropDatabase"
      >
        <Icon name="trash" :size="12" /> 删除当前数据库…
      </button>
      <button class="context-menu-item" @click="refreshDatabaseList(); closeManagementMenus()">
        <Icon name="refresh" :size="12" /> 刷新数据库列表
      </button>
      <button
        class="context-menu-item"
        :disabled="!database"
        @click="closeManagementMenus(); usersOpen = true"
      >
        <Icon name="list" :size="12" /> 用户与角色…
      </button>
      <p v-if="isSystemDatabase" class="context-menu-hint">
        系统数据库 admin / local / config 不可删除
      </p>
      <p v-else-if="readOnly" class="context-menu-hint">
        只读连接不能新建或删除数据库
      </p>
    </div>

    <div
      v-if="collectionMenu"
      class="context-menu mongo-management-menu"
      :style="{ left: `${collectionMenu.x}px`, top: `${collectionMenu.y}px` }"
      @click.stop
    >
      <button
        class="context-menu-item"
        :disabled="readOnly || collectionMenu.item.kind !== 'collection'"
        @click="openRenameCollection(collectionMenu.item)"
      >
        <Icon name="edit" :size="12" /> 重命名集合…
      </button>
      <button
        class="context-menu-item"
        @click="openCollectionStats(collectionMenu.item)"
      >
        <Icon name="structure" :size="12" /> 集合统计…
      </button>
      <button
        class="context-menu-item danger"
        :disabled="readOnly"
        @click="deleteCollection = collectionMenu.item; closeManagementMenus()"
      >
        <Icon name="trash" :size="12" /> 删除集合…
      </button>
      <p
        v-if="readOnly"
        class="context-menu-hint"
      >
        只读连接不能重命名或删除集合
      </p>
      <p
        v-else-if="collectionMenu.item.kind !== 'collection'"
        class="context-menu-hint"
      >
        视图 / 时间序列集合暂不支持从此处重命名
      </p>
    </div>

    <main class="mongo-main">
      <header class="mongo-toolbar">
        <div class="mongo-toolbar-title">
          <Icon
            :name="
              mode === 'console'
                ? 'code'
                : selectedCollection?.kind === 'view'
                  ? 'view'
                  : 'table'
            "
            :size="14"
          />
          <strong>
            {{
              mode === "console"
                ? "MongoDB 查询控制台"
                : selectedName || "选择一个集合"
            }}
          </strong>
          <span v-if="selectedCollection" class="mongo-scope">
            {{ database }} · {{ selectedCollection.kind }}
          </span>
        </div>
        <div class="mongo-toolbar-actions">
          <span v-if="displayPage && mode !== 'indexes' && mode !== 'console' && mode !== 'gridfs'" class="hint">
            {{
              mode === "aggregation"
                ? `${displayPage.documents.length} 个结果`
                : `${displayPage.total} 个文档`
            }}
            · {{ displayElapsedMs }} ms
          </span>
          <button
            class="button ghost compact"
            title="AI 助手（基于集合 / 字段结构，不发送文档内容）"
            @click="openMongoAi"
          >
            <Icon name="bolt" :size="13" />
            AI
          </button>
          <button
            v-if="mode !== 'console' && mode !== 'gridfs'"
            class="button ghost compact"
            :disabled="
              !selectedCollection ||
              (mode === 'aggregation'
                ? aggregationLoading
                : mode === 'indexes'
                  ? indexesLoading
                  : docsLoading)
            "
            @click="
              mode === 'aggregation'
                ? runAggregation()
                : mode === 'indexes'
                  ? loadIndexes()
                  : loadDocuments(true)
            "
          >
            <Icon name="refresh" :size="13" />
            刷新
          </button>
          <button
            v-if="mode !== 'indexes' && mode !== 'console' && mode !== 'gridfs'"
            class="button ghost compact"
            :disabled="!selectedCollection || importing"
            title="从 JSON 或 CSV 文件批量插入文档"
            @click="importDocuments"
          >
            <Icon name="upload" :size="13" />
            导入…
          </button>
          <button
            v-if="mode !== 'indexes' && mode !== 'console' && mode !== 'gridfs'"
            class="button ghost compact"
            :disabled="!displayPage?.documents.length"
            title="导出当前筛选 / 聚合结果（最多 10,000 个文档）"
            @click="exportDocuments"
          >
            <Icon name="download" :size="13" />
            导出…
          </button>
          <button
            v-if="mode !== 'console' && mode !== 'gridfs'"
            class="button primary compact"
            :disabled="!canEdit || !selectedCollection"
            :title="
              canEdit
                ? '插入一个 JSON 文档'
                : '只读连接不能插入文档'
            "
            @click="openInsert"
          >
            <Icon name="plus" :size="13" />
            插入文档
          </button>
        </div>
      </header>

      <div
        v-if="!selectedCollection && mode !== 'console' && mode !== 'gridfs'"
        class="mongo-empty"
      >
        <Icon name="database" :size="24" />
        <strong>选择左侧集合开始浏览</strong>
        <span>
          MongoDB 文档按动态字段展示；支持 JSON 筛选、投影、排序和分页。
        </span>
        <button class="button ghost compact" @click="switchMode('console')">
          <Icon name="code" :size="13" />
          打开查询控制台
        </button>
        <button class="button ghost compact" @click="switchMode('gridfs')">
          <Icon name="upload" :size="13" />
          打开 GridFS
        </button>
        <span v-if="docsError" class="mongo-empty-error">{{ docsError }}</span>
      </div>

      <template v-else>
        <div class="mongo-mode-tabs" role="tablist" aria-label="MongoDB 集合视图">
          <button
            class="segment"
            :class="{ active: mode === 'documents' }"
            role="tab"
            :aria-selected="mode === 'documents'"
            @click="switchMode('documents')"
          >
            文档
          </button>
          <button
            class="segment"
            :class="{ active: mode === 'console' }"
            role="tab"
            :aria-selected="mode === 'console'"
            @click="switchMode('console')"
          >
            查询
          </button>
          <button
            class="segment"
            :class="{ active: mode === 'aggregation' }"
            role="tab"
            :aria-selected="mode === 'aggregation'"
            @click="switchMode('aggregation')"
          >
            聚合
          </button>
          <button
            class="segment"
            :class="{ active: mode === 'indexes' }"
            role="tab"
            :aria-selected="mode === 'indexes'"
            @click="switchMode('indexes')"
          >
            索引
          </button>
          <button
            class="segment"
            :class="{ active: mode === 'gridfs' }"
            role="tab"
            :aria-selected="mode === 'gridfs'"
            @click="switchMode('gridfs')"
          >
            GridFS
          </button>
        </div>

        <template v-if="mode === 'console'">
          <MongoConsole
            ref="consoleRef"
            :connection-id="connectionId"
            :database="database"
            :databases="databases"
            :collections="collections"
            :fields="fields"
            :read-only="readOnly"
            @database-change="handleConsoleDatabaseChange"
            @ready="flushPendingMongoAi"
            @refresh-collections="refreshAfterConsoleWrite"
            @notice="flashToast"
          />
        </template>

        <template v-else-if="mode === 'gridfs'">
          <MongoGridFsPanel
            :connection-id="connectionId"
            :database="database"
            :read-only="readOnly"
            @status="flashToast"
          />
        </template>

        <template v-else-if="mode === 'documents'">
          <div class="mongo-filterbar">
            <label class="mongo-filter-field">
              <span>筛选 JSON</span>
              <input
                v-model="filterDraft"
                class="mono"
                placeholder='{ "status": "paid" }'
                @keydown.enter.prevent="loadDocuments(true)"
              />
            </label>
            <label class="mongo-filter-field narrow">
              <span>投影</span>
              <input
                v-model="projectionDraft"
                class="mono"
                placeholder='{ "name": 1 }'
              />
            </label>
            <label class="mongo-filter-field narrow">
              <span>排序</span>
              <input
                v-model="sortDraft"
                class="mono"
                placeholder='{ "createdAt": -1 }'
              />
            </label>
            <button
              class="button primary compact"
              :disabled="docsLoading"
              @click="loadDocuments(true)"
            >
              查询
            </button>
            <button class="button ghost compact" @click="resetFilters">
              重置
            </button>
          </div>
        </template>

        <template v-else-if="mode === 'aggregation'">
          <div class="mongo-aggbar">
            <label class="mongo-agg-editor">
              <span>聚合管道 JSON（数组，最多 500 条结果；禁止 $out / $merge）</span>
              <textarea
                v-model="aggregationPipeline"
                class="mono"
                rows="7"
                spellcheck="false"
              ></textarea>
            </label>
            <div class="mongo-agg-actions">
              <span class="hint">
                示例：$match → $group → $sort，结果以只读网格展示。
              </span>
              <span class="spacer"></span>
              <button
                class="button primary compact"
                :disabled="aggregationLoading"
                @click="runAggregation"
              >
                <Icon name="play" :size="13" />
                {{ aggregationLoading ? "运行中…" : "运行聚合" }}
              </button>
            </div>
          </div>
        </template>

        <template v-if="mode !== 'indexes' && mode !== 'console'">
          <p v-if="displayError" class="data-message warning mongo-error">
            {{ displayError }}
          </p>
          <div
            v-if="mode === 'documents' && pendingCount"
            class="mongo-pending-bar"
          >
            <span>
              待提交 <strong>{{ pendingCount }}</strong> 项变更
              <span v-if="projectionDraft.trim()" class="hint">
                （使用投影时不可网格编辑）
              </span>
            </span>
            <span class="spacer"></span>
            <button
              class="button ghost compact"
              :disabled="pendingSaving"
              @click="pendingPreviewOpen = true"
            >
              查看变更
            </button>
            <button
              class="button ghost compact"
              :disabled="pendingSaving"
              @click="discardPendingMongo"
            >
              放弃
            </button>
            <button
              class="button primary compact"
              :disabled="pendingSaving"
              @click="pendingPreviewOpen = true"
            >
              {{ pendingSaving ? "提交中…" : "提交变更" }}
            </button>
          </div>
          <p v-if="pendingError" class="data-message warning mongo-error">
            {{ pendingError }}
          </p>
          <div class="mongo-grid-wrap">
            <table class="mongo-grid">
              <thead>
                <tr>
                  <th class="mongo-index-col">#</th>
                  <th v-for="field in fields" :key="field">{{ field }}</th>
                  <th class="mongo-actions-col">
                    <span class="sr-only">操作</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                <template v-if="displayLoading">
                  <tr>
                    <td :colspan="fields.length + 2" class="mongo-grid-hint">
                      读取中…
                    </td>
                  </tr>
                </template>
                <template v-else-if="!displayPage?.documents.length">
                  <tr class="mongo-grid-empty">
                    <td :colspan="fields.length + 2">
                      <Icon name="search" :size="16" />
                      {{
                        mode === "aggregation"
                          ? "没有聚合结果"
                          : "没有匹配的文档"
                      }}
                    </td>
                  </tr>
                </template>
                <template v-else>
                  <tr
                    v-for="(document, index) in displayPage.documents"
                    :key="String(document._id ?? index)"
                    class="mongo-doc-row"
                    :class="{
                      'mongo-doc-deleted': pendingDocument(document)?.deleted,
                    }"
                  >
                    <td class="mongo-index-col">
                      {{
                        mode === "documents" ? skip + index + 1 : index + 1
                      }}
                    </td>
                    <td
                      v-for="field in fields"
                      :key="field"
                      :class="{
                        'mongo-null': cellValue(document, field) === null,
                        mono: typeof cellValue(document, field) !== 'string',
                        'mongo-cell-modified':
                          pendingDocument(document) &&
                          Object.prototype.hasOwnProperty.call(
                            pendingDocument(document)?.updates ?? {},
                            field,
                          ),
                        'mongo-cell-editable':
                          canEditGrid &&
                          field !== '_id' &&
                          typeof cellValue(document, field) !== 'object' &&
                          !pendingDocument(document)?.deleted,
                      }"
                      :title="cellText(document, field)"
                      @dblclick.stop="startGridEdit(document, field)"
                    >
                      <input
                        v-if="
                          gridEdit &&
                          gridEdit.key === documentKey(document) &&
                          gridEdit.field === field
                        "
                        v-model="gridEdit.value"
                        v-focus
                        class="mongo-cell-input"
                        @keydown.enter.prevent="commitGridEdit"
                        @keydown.esc.prevent="gridEdit = null"
                        @blur="commitGridEdit"
                      />
                      <template v-else>
                        {{
                          cellText(document, field) ||
                          (cellValue(document, field) === "" ? "(空串)" : "—")
                        }}
                      </template>
                    </td>
                    <td class="mongo-actions-col">
                      <button
                        class="icon-button"
                        title="查看文档 JSON"
                        aria-label="查看文档 JSON"
                        @click="openDocument(document, false)"
                      >
                        <Icon name="code" :size="12" />
                      </button>
                      <button
                        v-if="canEdit && mode === 'documents' && document._id !== undefined"
                        class="icon-button"
                        title="编辑文档"
                        aria-label="编辑文档"
                        @click="openDocument(document, true)"
                      >
                        <Icon name="edit" :size="12" />
                      </button>
                      <button
                        v-if="
                          canEditGrid &&
                          document._id !== undefined &&
                          !pendingDocument(document)?.deleted
                        "
                        class="icon-button danger"
                        title="标记删除（提交前可放弃）"
                        aria-label="标记删除文档"
                        @click="togglePendingDelete(document)"
                      >
                        <Icon name="trash" :size="12" />
                      </button>
                      <button
                        v-if="pendingDocument(document)?.deleted"
                        class="icon-button"
                        title="取消标记删除"
                        aria-label="取消标记删除"
                        @click="togglePendingDelete(document)"
                      >
                        <Icon name="undo" :size="12" />
                      </button>
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
          </div>
          <footer v-if="mode === 'documents'" class="mongo-pagination">
            <span>
              第 {{ currentPage }} / {{ pageCount }} 页
              <template v-if="page?.truncated"> · 还有更多</template>
            </span>
            <span class="spacer"></span>
            <label>
              每页
              <select :value="limit" @change="setLimit">
                <option :value="20">20</option>
                <option :value="50">50</option>
                <option :value="100">100</option>
                <option :value="200">200</option>
                <option :value="500">500</option>
              </select>
            </label>
            <button
              class="button ghost compact"
              :disabled="!hasPrevious || docsLoading"
              @click="goPage(-1)"
            >
              上一页
            </button>
            <button
              class="button ghost compact"
              :disabled="!hasNext || docsLoading"
              @click="goPage(1)"
            >
              下一页
            </button>
          </footer>
          <footer v-else class="mongo-pagination">
            <span>
              {{ displayPage?.documents.length ?? 0 }} 条聚合结果
              <template v-if="displayPage?.truncated">
                · 已达到 500 条上限，可能还有更多
              </template>
            </span>
            <span class="spacer"></span>
            <span class="hint">聚合结果只读</span>
          </footer>
        </template>

        <template v-else-if="mode === 'indexes'">
          <div class="mongo-index-toolbar">
            <span class="hint">
              索引只作用于当前集合；删除默认 _id 索引会被后端拒绝。
            </span>
            <span class="spacer"></span>
            <button
              class="button primary compact"
              :disabled="!canEdit"
              @click="indexDialogOpen = true"
            >
              <Icon name="plus" :size="13" />
              新建索引
            </button>
            <button
              class="button ghost compact"
              :disabled="indexesLoading"
              @click="loadIndexes"
            >
              <Icon name="refresh" :size="13" />
              刷新
            </button>
          </div>
          <p v-if="indexesError" class="data-message warning mongo-error">
            {{ indexesError }}
          </p>
          <div class="mongo-index-list">
            <table class="mongo-grid">
              <thead>
                <tr>
                  <th>名称</th>
                  <th>字段</th>
                  <th>属性</th>
                  <th class="mongo-actions-col">
                    <span class="sr-only">操作</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                <template v-if="indexesLoading">
                  <tr><td colspan="4" class="mongo-grid-hint">读取中…</td></tr>
                </template>
                <template v-else-if="!indexes.length">
                  <tr><td colspan="4" class="mongo-grid-hint">没有索引</td></tr>
                </template>
                <template v-else>
                <tr v-for="index in indexes" :key="index.name">
                  <td class="mono">{{ index.name }}</td>
                  <td class="mono">{{ JSON.stringify(index.keys) }}</td>
                  <td>
                    <span v-if="index.unique" class="mongo-kind">unique</span>
                    <span v-if="index.sparse" class="mongo-kind">sparse</span>
                    <span
                      v-if="index.expireAfterSeconds !== null"
                      class="mongo-kind"
                    >
                      TTL {{ index.expireAfterSeconds }}s
                    </span>
                    <span
                      v-if="!index.unique && !index.sparse && index.expireAfterSeconds === null"
                      class="hint"
                    >
                      —
                    </span>
                  </td>
                  <td class="mongo-actions-col">
                    <button
                      v-if="canEdit && index.name !== '_id_'"
                      class="icon-button danger"
                      title="删除索引"
                      aria-label="删除索引"
                      @click="dropIndexName = index.name"
                    >
                      <Icon name="trash" :size="12" />
                    </button>
                  </td>
                </tr>
                </template>
              </tbody>
            </table>
          </div>
        </template>
      </template>
    </main>

    <div v-if="docDialogOpen" class="modal-backdrop" @click.self="docDialogOpen = false">
      <div
        class="modal mongo-doc-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="MongoDB 文档"
      >
        <header class="modal-header">
          <div>
            <h2>
              {{
                docDialogMode === "insert"
                  ? "插入文档"
                  : docDialogMode === "edit"
                    ? "编辑文档"
                    : "文档 JSON"
              }}
            </h2>
            <p class="hint">
              {{
                docDialogMode === "view"
                  ? "只读预览；双击网格行也可以打开。"
                  : "使用 MongoDB Extended JSON（如 {\"$oid\":\"...\"}）；保存时 _id 不会被修改。"
              }}
            </p>
          </div>
          <button
            class="icon-button"
            title="关闭"
            aria-label="关闭文档"
            @click="docDialogOpen = false"
          >
            <Icon name="close" :size="14" />
          </button>
        </header>
        <textarea
          v-model="docDraft"
          class="mongo-doc-editor mono"
          spellcheck="false"
          :readonly="docDialogMode === 'view'"
        ></textarea>
        <p v-if="docError" class="data-message warning">{{ docError }}</p>
        <footer class="modal-actions">
          <span class="hint">
            {{ database }} / {{ selectedName }}
          </span>
          <span class="spacer"></span>
          <button
            v-if="docDialogMode === 'view' && canEdit"
            class="button ghost"
            @click="beginEditDocument"
          >
            <Icon name="edit" :size="13" />
            编辑
          </button>
          <button
            v-if="docOriginal && canEdit && docDialogMode !== 'insert'"
            class="button ghost danger-text"
            @click="askDeleteDocument(docOriginal)"
          >
            <Icon name="trash" :size="13" />
            删除文档…
          </button>
          <button
            v-if="docDialogMode !== 'view'"
            class="button primary"
            :disabled="docSaving"
            @click="saveDocument"
          >
            {{ docSaving ? "保存中…" : "保存" }}
          </button>
          <button class="button ghost" @click="docDialogOpen = false">
            关闭
          </button>
        </footer>
      </div>
    </div>

    <ConfirmDialog
      :open="Boolean(deleteDoc)"
      title="删除 MongoDB 文档"
      :message="`确定删除 _id 为 ${JSON.stringify(deleteDoc?._id ?? '')} 的文档？`"
      detail="此操作不可撤销。"
      confirm-text="删除文档"
      danger
      @confirm="confirmDeleteDocument"
      @cancel="deleteDoc = null"
    />

    <ConfirmDialog
      :open="Boolean(deleteCollection)"
      title="删除 MongoDB 集合"
      :message="`确定删除${deleteCollection?.kind === 'view' ? '视图' : deleteCollection?.kind === 'timeseries' ? '时间序列集合' : '集合'}「${deleteCollection?.name ?? ''}」？`"
      :detail="`集合类型：${deleteCollection?.kind ?? 'collection'}；集合及其全部文档和索引会被删除，此操作不可撤销。`"
      confirm-text="删除集合"
      danger
      @confirm="confirmDropCollection"
      @cancel="deleteCollection = null"
    />

    <ConfirmDialog
      :open="Boolean(dropIndexName)"
      title="删除 MongoDB 索引"
      :message="`确定删除索引「${dropIndexName}」？`"
      detail="只删除索引，不会删除文档。"
      confirm-text="删除索引"
      danger
      @confirm="confirmDropIndex"
      @cancel="dropIndexName = ''"
    />

    <PromptDialog
      :open="createCollectionOpen"
      title="新建 MongoDB 集合"
      message="输入集合名称；集合会立即创建。"
      placeholder="例如：events"
      confirm-text="创建"
      @confirm="createCollection"
      @cancel="createCollectionOpen = false"
    />

    <div
      v-if="indexDialogOpen"
      class="modal-backdrop"
      @click.self="indexDialogOpen = false"
    >
      <div class="modal mongo-index-dialog" role="dialog" aria-modal="true">
        <header class="modal-header">
          <div>
            <h2>新建索引</h2>
            <p class="hint">键方向：1 升序，-1 降序；例：{ "email": 1 }</p>
          </div>
          <button
            class="icon-button"
            title="关闭"
            aria-label="关闭新建索引"
            @click="indexDialogOpen = false"
          >
            <Icon name="close" :size="14" />
          </button>
        </header>
        <div class="form-grid">
          <label class="span-2">
            索引键（JSON）
            <textarea
              v-model="indexKeys"
              class="mono mongo-index-keys"
              rows="4"
              spellcheck="false"
            ></textarea>
          </label>
          <label>
            索引名称（可选）
            <input v-model="indexName" placeholder="自动生成" />
          </label>
          <label class="checkbox-row">
            <input v-model="indexUnique" type="checkbox" />
            唯一索引
          </label>
        </div>
        <p v-if="indexesError" class="data-message warning">{{ indexesError }}</p>
        <footer class="modal-actions">
          <button class="button ghost" @click="indexDialogOpen = false">
            取消
          </button>
          <button class="button primary" @click="createIndex">
            创建索引
          </button>
        </footer>
      </div>
    </div>

    <AiSqlDialog
      :open="aiOpen"
      :sql="mongoAiSql"
      selected-sql=""
      driver="mongodb"
      :database="database"
      :schema="null"
      :schema-context="mongoAiContext"
      language="mongodb"
      :error="docsError || aggregationError"
      @close="aiOpen = false"
      @insert="insertMongoAi"
      @replace="replaceMongoAi"
      @open-settings="emit('open-settings')"
    />

    <MongoUsersDialog
      :open="usersOpen"
      :connection-id="connectionId"
      :database="database"
      :read-only="readOnly"
      @close="usersOpen = false"
      @status="flashToast"
    />

    <MongoCreateDatabaseDialog
      :open="createDatabaseOpen"
      :busy="createDatabaseBusy"
      :error="createDatabaseError"
      @cancel="createDatabaseOpen = false"
      @confirm="confirmCreateDatabase"
    />

    <MongoDropDatabaseDialog
      :open="dropDatabaseOpen"
      :database="database"
      :collection-count="dropDatabaseCount"
      :busy="dropDatabaseBusy"
      :error="dropDatabaseError"
      @cancel="dropDatabaseOpen = false"
      @confirm="confirmDropDatabase"
    />

    <MongoRenameCollectionDialog
      :open="Boolean(renameTarget)"
      :database="database"
      :from="renameTarget?.name ?? ''"
      :existing="collections.map((item) => item.name)"
      :busy="renameBusy"
      :error="renameError"
      @cancel="renameTarget = null"
      @confirm="confirmRenameCollection"
    />

    <MongoCollectionStatsDialog
      :open="Boolean(statsTarget)"
      :connection-id="connectionId"
      :database="database"
      :collection="statsTarget?.name ?? ''"
      :kind="statsTarget?.kind"
      @close="statsTarget = null"
    />

    <ConfirmDialog
      :open="pendingPreviewOpen"
      title="提交 MongoDB 待提交变更"
      :message="`将执行 ${pendingCount} 项 updateOne / deleteOne 操作。`"
      :detail="pendingPreview() || '没有待提交变更'"
      :confirm-text="pendingSaving ? '提交中…' : '提交变更'"
      danger
      @confirm="commitPendingMongo"
      @cancel="pendingPreviewOpen = false"
    />

    <ConfirmDialog
      :open="Boolean(importPending)"
      title="导入 MongoDB 文档"
      :message="`将向 ${selectedName} 插入 ${importPending?.count ?? 0} 个 ${importPending?.format ?? 'JSON'} 文档。`"
      detail="导入会立即写入集合；文件中的 _id 也会按原值写入，可能与已有文档冲突。"
      :confirm-text="importing ? '导入中…' : '开始导入'"
      @confirm="confirmImportDocuments"
      @cancel="importPending = null"
    />

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>
  </section>
</template>
