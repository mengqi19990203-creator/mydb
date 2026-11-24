<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import ExportDialog from "./ExportDialog.vue";
import Icon from "./Icon.vue";
import PromptDialog from "./PromptDialog.vue";
import * as api from "../lib/api";
import type { ExportFormat } from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import {
  buildGeneratedSql,
  qualifiedDisplayName,
  type GeneratedSqlKind,
} from "../lib/generate-sql";
import type { ColumnInfo, Driver, TableInfo } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  driver: Driver;
  tables: TableInfo[];
  search: string;
  activeTable: string | null;
  database?: string;
  schema?: string | null;
  locateTarget?: string | null;
  locateNonce?: number;
  collapseNonce?: number;
}>();

const emit = defineEmits<{
  select: [table: TableInfo];
  "select-structure": [table: TableInfo];
  "select-index": [table: TableInfo];
  "open-new-tab": [table: TableInfo];
  "import-data": [table: TableInfo];
  create: [];
  refresh: [];
  "generate-sql": [payload: { sql: string; title: string }];
}>();

const collapsed = ref<Record<string, boolean>>({});
const expandedTables = reactive<Record<string, boolean>>({});
const locatedKey = ref<string | null>(null);
let locateTimer = 0;
const columnsByTable = reactive<Record<string, ColumnInfo[]>>({});
const columnsLoading = reactive<Record<string, boolean>>({});
const columnsError = reactive<Record<string, string>>({});

const objectMenu = ref<{
  x: number;
  y: number;
  table: TableInfo;
} | null>(null);
const renameTarget = ref<TableInfo | null>(null);
const renameOpen = ref(false);
const deleteTarget = ref<TableInfo | null>(null);
const deleteOpen = ref(false);
const exportTarget = ref<TableInfo | null>(null);
const exportOpen = ref(false);
const copyTarget = ref<TableInfo | null>(null);
const copyMode = ref<"table" | "view" | "snapshot" | null>(null);
const copyOpen = ref(false);
const generateOpen = ref(false);
const menuColumns = ref<ColumnInfo[] | null>(null);
const menuColumnsLoading = ref(false);
const menuHasPrimaryKey = computed(
  () => Boolean(menuColumns.value?.some((column) => column.primaryKey)),
);
const maintenanceTarget = ref<TableInfo | null>(null);
const maintenanceAction = ref<"empty" | "truncate" | null>(null);
const maintenanceOpen = ref(false);
const exporting = ref(false);
const exportVisible = ref(false);
const exportOperationId = ref<string | null>(null);
const exportCancelled = ref(false);
let exportShowTimer = 0;
const toast = ref("");
let toastTimer = 0;

const copyDialogTitle = computed(() => {
  switch (copyMode.value) {
    case "view":
      return "复制为新视图";
    case "snapshot":
      return "快照为新表";
    default:
      return "复制为新表";
  }
});

const copyDefaultName = computed(() =>
  copyTarget.value ? `${copyTarget.value.name}_copy` : "",
);

interface ObjectCategory {
  kind: string;
  label: string;
  icon:
    | "table"
    | "view"
    | "code"
    | "bolt"
    | "history"
    | "key"
    | "clock"
    | "list"
    | "hash";
}

const CATEGORIES: Record<string, ObjectCategory[]> = {
  mysql: [
    { kind: "table", label: "表", icon: "table" },
    { kind: "view", label: "视图", icon: "view" },
    { kind: "procedure", label: "存储过程", icon: "code" },
    { kind: "function", label: "函数", icon: "code" },
    { kind: "trigger", label: "触发器", icon: "bolt" },
    { kind: "event", label: "事件", icon: "clock" },
    { kind: "sequence", label: "序列", icon: "hash" },
  ],
  postgres: [
    { kind: "table", label: "表", icon: "table" },
    { kind: "view", label: "视图", icon: "view" },
    { kind: "matview", label: "物化视图", icon: "view" },
    { kind: "procedure", label: "存储过程", icon: "code" },
    { kind: "function", label: "函数", icon: "code" },
    { kind: "trigger", label: "触发器", icon: "bolt" },
    { kind: "sequence", label: "序列", icon: "hash" },
  ],
  sqlite: [
    { kind: "table", label: "表", icon: "table" },
    { kind: "view", label: "视图", icon: "view" },
    { kind: "index", label: "索引", icon: "list" },
    { kind: "trigger", label: "触发器", icon: "bolt" },
  ],
};

const KIND_LABELS: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
  procedure: "存储过程",
  function: "函数",
  trigger: "触发器",
  event: "事件",
  sequence: "序列",
  index: "索引",
};

const categories = computed(
  () => CATEGORIES[props.driver] ?? CATEGORIES.sqlite,
);

// Categories start collapsed; the user expands only what they need. The
// component remounts when the database/schema changes, so this also resets
// the collapsed state per database.
watch(
  () => props.driver,
  (driver) => {
    for (const category of CATEGORIES[driver] ?? CATEGORIES.sqlite) {
      if (collapsed.value[category.kind] === undefined) {
        collapsed.value[category.kind] = true;
      }
    }
  },
  { immediate: true },
);

const filteredTables = computed(() => {
  const query = props.search.trim().toLowerCase();
  if (!query) return props.tables;
  return props.tables.filter((table) =>
    table.name.toLowerCase().includes(query),
  );
});

const groups = computed(() =>
  categories.value.map((category) => ({
    ...category,
    total: props.tables.filter((table) => table.kind === category.kind)
      .length,
    items: filteredTables.value.filter(
      (table) => table.kind === category.kind,
    ),
  })),
);

function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? "对象";
}

function isExpandable(table: TableInfo): boolean {
  return (
    table.kind === "table" ||
    table.kind === "view" ||
    table.kind === "matview"
  );
}

function canFetchDdl(table: TableInfo): boolean {
  switch (props.driver) {
    case "sqlite":
      return ["table", "view", "index", "trigger"].includes(table.kind);
    case "mysql":
      return [
        "table",
        "view",
        "procedure",
        "function",
        "trigger",
        "event",
      ].includes(table.kind);
    case "postgres":
      return [
        "table",
        "view",
        "matview",
        "function",
        "procedure",
        "trigger",
        "index",
        "sequence",
      ].includes(table.kind);
    default:
      return false;
  }
}

function canRename(table: TableInfo): boolean {
  switch (props.driver) {
    case "sqlite":
      return table.kind === "table";
    case "mysql":
      return table.kind === "table" || table.kind === "view";
    case "postgres":
      return [
        "table",
        "view",
        "matview",
        "sequence",
        "index",
        "function",
        "procedure",
      ].includes(table.kind);
    default:
      return false;
  }
}

function canDrop(table: TableInfo): boolean {
  switch (props.driver) {
    case "sqlite":
      return ["table", "view", "index", "trigger"].includes(table.kind);
    case "mysql":
      return [
        "table",
        "view",
        "procedure",
        "function",
        "trigger",
        "event",
        "sequence",
      ].includes(table.kind);
    case "postgres":
      return [
        "table",
        "view",
        "matview",
        "sequence",
        "index",
        "function",
        "procedure",
        "trigger",
      ].includes(table.kind);
    default:
      return false;
  }
}

function tableKey(table: TableInfo): string {
  return `${table.kind}:${table.name}`;
}

function toggle(kind: string) {
  collapsed.value[kind] = !collapsed.value[kind];
}

function collapseAllCategories() {
  for (const category of CATEGORIES[props.driver] ?? CATEGORIES.sqlite) {
    collapsed.value[category.kind] = true;
  }
  for (const key of Object.keys(expandedTables)) {
    delete expandedTables[key];
  }
  locatedKey.value = null;
}

function locateTable(name: string): boolean {
  const table = props.tables.find((item) => item.name === name);
  if (!table) {
    flashToast("对象不在当前列表中");
    return false;
  }
  collapsed.value[table.kind] = false;
  const key = tableKey(table);
  locatedKey.value = key;
  window.clearTimeout(locateTimer);
  locateTimer = window.setTimeout(() => {
    if (locatedKey.value === key) locatedKey.value = null;
  }, 1600);
  void nextTick(() => {
    const element = Array.from(
      document.querySelectorAll<HTMLElement>("[data-object-key]"),
    ).find((item) => item.dataset.objectKey === key);
    if (!element) return;
    element.scrollIntoView({ block: "nearest" });
    element.focus({ preventScroll: true });
  });
  return true;
}

watch(
  () => props.collapseNonce,
  (value) => {
    if (value !== undefined) collapseAllCategories();
  },
);

watch(
  () => props.locateNonce,
  (value) => {
    if (value !== undefined && props.locateTarget) {
      locateTable(props.locateTarget);
    }
  },
  { immediate: true },
);

async function loadColumns(table: TableInfo, key: string) {
  columnsLoading[key] = true;
  columnsError[key] = "";
  try {
    columnsByTable[key] = await api.listColumns(
      props.connectionId,
      table.name,
    );
  } catch (reason) {
    columnsError[key] = String(reason);
  } finally {
    delete columnsLoading[key];
  }
}

function onTableClick(table: TableInfo) {
  if (table.kind === "index") {
    emit("select-index", table);
    return;
  }
  if (!isExpandable(table)) {
    emit("select", table);
    return;
  }

  const key = tableKey(table);
  if (expandedTables[key]) {
    delete expandedTables[key];
    return;
  }

  expandedTables[key] = true;
  if (!columnsByTable[key]) {
    void loadColumns(table, key);
  }
}

function resetColumns() {
  for (const key of Object.keys(expandedTables)) {
    delete expandedTables[key];
  }
  for (const key of Object.keys(columnsByTable)) {
    delete columnsByTable[key];
  }
  for (const key of Object.keys(columnsLoading)) {
    delete columnsLoading[key];
  }
  for (const key of Object.keys(columnsError)) {
    delete columnsError[key];
  }
}

function flashToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 1800);
}

async function copyText(text: string, message: string) {
  try {
    await writeClipboard(text);
    flashToast(message);
  } catch {
    flashToast("复制失败");
  }
}

function openObjectMenu(event: MouseEvent, table: TableInfo) {
  event.preventDefault();
  event.stopPropagation();
  generateOpen.value = false;
  menuColumns.value = null;
  menuColumnsLoading.value = false;
  objectMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 220)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 300)),
    table,
  };
}

function closeObjectMenu() {
  objectMenu.value = null;
  generateOpen.value = false;
  menuColumns.value = null;
  menuColumnsLoading.value = false;
}

function menuOpenData() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (table) emit("select", table);
}

function menuOpenStructure() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (table) emit("select-structure", table);
}

function menuOpenInNewTab() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (table) emit("open-new-tab", table);
}

function menuImportData() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (table) emit("import-data", table);
}

function menuCreateTable() {
  closeObjectMenu();
  emit("create");
}

function menuCopyName() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (table) void copyText(table.name, "已复制名称");
}

async function menuCopyDdl() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  flashToast("正在读取 DDL…");
  try {
    const ddl = await api.getObjectDdl(
      props.connectionId,
      table.kind,
      table.name,
    );
    await copyText(ddl, "已复制 DDL");
  } catch (reason) {
    flashToast(String(reason));
  }
}

function canExportData(table: TableInfo): boolean {
  return ["table", "view", "matview"].includes(table.kind);
}

function canCopyQualifiedName(): boolean {
  if (props.driver === "postgres") {
    return Boolean(props.schema);
  }
  return Boolean(props.database);
}

function menuCopyQualifiedName() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  void copyText(
    qualifiedDisplayName(
      props.driver,
      table.name,
      props.database,
      props.schema,
    ),
    "已复制限定名",
  );
}

function toggleGenerateMenu() {
  if (generateOpen.value) {
    generateOpen.value = false;
  } else {
    void openGenerateMenu();
  }
}

async function openGenerateMenu() {
  generateOpen.value = true;
  const table = objectMenu.value?.table;
  if (!table || menuColumns.value) return;

  const cached = columnsByTable[tableKey(table)];
  if (cached?.length) {
    menuColumns.value = cached;
    return;
  }
  if (menuColumnsLoading.value) return;

  menuColumnsLoading.value = true;
  try {
    const structure = await api.getTableStructure(
      props.connectionId,
      table.name,
    );
    if (objectMenu.value?.table.name === table.name) {
      menuColumns.value = structure.columns;
    }
  } catch {
    // The generate action reports load errors when an item is clicked.
  } finally {
    menuColumnsLoading.value = false;
  }
}

async function generateSql(kind: GeneratedSqlKind) {
  const table = objectMenu.value?.table;
  const cachedColumns = menuColumns.value;
  closeObjectMenu();
  if (!table) return;
  try {
    if (kind === "ddl") {
      const ddl = await api.getObjectDdl(
        props.connectionId,
        table.kind,
        table.name,
      );
      emit("generate-sql", { sql: ddl, title: `${table.name} DDL` });
      return;
    }
    const columns =
      cachedColumns ??
      (await api.getTableStructure(props.connectionId, table.name)).columns;
    if (
      (kind === "update" || kind === "delete") &&
      !columns.some((column) => column.primaryKey)
    ) {
      flashToast("该表没有主键，无法生成安全的 UPDATE/DELETE");
      return;
    }
    const sql = buildGeneratedSql(
      kind,
      props.driver,
      table.name,
      columns,
      props.database,
      props.schema,
    );
    emit("generate-sql", {
      sql,
      title: `${table.name} ${kind.toUpperCase()}`,
    });
  } catch (reason) {
    flashToast(String(reason));
  }
}

const maintenanceTitle = computed(() =>
  maintenanceAction.value === "truncate" ? "截断表" : "清空表",
);

const maintenanceMessage = computed(() => {
  const name = maintenanceTarget.value?.name ?? "";
  return maintenanceAction.value === "truncate"
    ? `确定截断表「${name}」？`
    : `确定清空表「${name}」中的所有数据？`;
});

const maintenanceDetail = computed(() => {
  const sql = maintenanceSqlPreview();
  if (maintenanceAction.value !== "truncate") {
    return `使用 DELETE FROM 删除全部数据，不重置自增计数。\n\n${sql}`;
  }
  switch (props.driver) {
    case "postgres":
      return `TRUNCATE TABLE ... RESTART IDENTITY 会删除全部数据并重置序列。\n\n${sql}`;
    case "sqlite":
      return `SQLite 使用 DELETE FROM 并清理 sqlite_sequence，重置 AUTOINCREMENT 计数。\n\n${sql}`;
    default:
      return `TRUNCATE TABLE 会删除全部数据并重置自增计数。\n\n${sql}`;
  }
});

function maintenanceSqlPreview(): string {
  const table = maintenanceTarget.value?.name ?? "";
  const quoted =
    props.driver === "mysql"
      ? `\`${table.replaceAll("`", "``")}\``
      : `"${table.replaceAll('"', '""')}"`;
  if (maintenanceAction.value !== "truncate") {
    return `DELETE FROM ${quoted};`;
  }
  if (props.driver === "postgres") {
    return `TRUNCATE TABLE ${quoted} RESTART IDENTITY;`;
  }
  if (props.driver === "sqlite") {
    return `DELETE FROM ${quoted};`;
  }
  return `TRUNCATE TABLE ${quoted};`;
}

function menuEmptyTable() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  maintenanceTarget.value = table;
  maintenanceAction.value = "empty";
  maintenanceOpen.value = true;
}

function menuTruncateTable() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  maintenanceTarget.value = table;
  maintenanceAction.value = "truncate";
  maintenanceOpen.value = true;
}

async function confirmMaintenance() {
  const table = maintenanceTarget.value;
  const action = maintenanceAction.value;
  maintenanceOpen.value = false;
  maintenanceTarget.value = null;
  maintenanceAction.value = null;
  if (!table || !action) return;

  try {
    const affected = await api.maintainTable(
      props.connectionId,
      table.name,
      action,
    );
    flashToast(
      action === "empty"
        ? `已清空 ${affected} 行`
        : `已截断表 ${table.name}`,
    );
    emit("refresh");
  } catch (reason) {
    flashToast(String(reason));
  }
}

function menuRefresh() {
  closeObjectMenu();
  emit("refresh");
}

function menuExport() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  exportTarget.value = table;
  exportOpen.value = true;
}

async function confirmExport(
  format: ExportFormat,
  limit: number | null,
  withHeader: boolean,
) {
  const table = exportTarget.value;
  exportOpen.value = false;
  if (!table) return;

  const extensions: Record<ExportFormat, string> = {
    csv: "csv",
    json: "json",
    sql: "sql",
    ddl: "sql",
  };
  const labels: Record<ExportFormat, string> = {
    csv: "CSV",
    json: "JSON",
    sql: "SQL",
    ddl: "DDL",
  };
  const extension = extensions[format];
  const path = await saveDialog({
    defaultPath: `${table.name}.${extension}`,
    filters: [{ name: labels[format], extensions: [extension] }],
  });
  if (typeof path !== "string") return;

  const operationId =
    globalThis.crypto?.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  exportOperationId.value = operationId;
  exportCancelled.value = false;
  exporting.value = true;
  window.clearTimeout(exportShowTimer);
  exportShowTimer = window.setTimeout(() => {
    if (exporting.value) {
      exportVisible.value = true;
    }
  }, 300);

  try {
    const count = await api.exportObject(
      props.connectionId,
      table.kind,
      table.name,
      format,
      path,
      limit,
      withHeader,
      operationId,
    );
    flashToast(
      format === "ddl" ? "已导出 DDL" : `已导出 ${count} 行`,
    );
  } catch (reason) {
    if (exportCancelled.value) {
      flashToast("已取消导出");
    } else {
      flashToast(String(reason));
    }
  } finally {
    window.clearTimeout(exportShowTimer);
    exporting.value = false;
    exportVisible.value = false;
    exportOperationId.value = null;
  }
}

function cancelExport() {
  const operationId = exportOperationId.value;
  if (!operationId) return;
  exportCancelled.value = true;
  void api.cancelExport(operationId);
  flashToast("正在取消…");
}

function menuDuplicate() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  copyTarget.value = table;
  copyMode.value = table.kind === "view" ? "view" : "table";
  copyOpen.value = true;
}

function menuSnapshot() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  copyTarget.value = table;
  copyMode.value = "snapshot";
  copyOpen.value = true;
}

async function confirmCopy(targetName: string, withData: boolean) {
  const source = copyTarget.value;
  const mode = copyMode.value;
  copyOpen.value = false;
  copyTarget.value = null;
  copyMode.value = null;
  if (!source || !mode) return;

  try {
    if (mode === "snapshot") {
      await api.snapshotViewToTable(
        props.connectionId,
        source.name,
        targetName,
        withData,
      );
      flashToast(`已创建表 ${targetName}`);
    } else {
      await api.duplicateObject(
        props.connectionId,
        mode,
        source.name,
        targetName,
      );
      flashToast(
        mode === "table"
          ? `已复制为 ${targetName}`
          : `已复制视图为 ${targetName}`,
      );
    }
    emit("refresh");
  } catch (reason) {
    flashToast(String(reason));
  }
}

function menuRename() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  renameTarget.value = table;
  renameOpen.value = true;
}

function menuDelete() {
  const table = objectMenu.value?.table;
  closeObjectMenu();
  if (!table) return;
  deleteTarget.value = table;
  deleteOpen.value = true;
}

async function confirmRename(newName: string) {
  const table = renameTarget.value;
  renameOpen.value = false;
  if (!table) return;
  try {
    await api.renameObject(
      props.connectionId,
      table.kind,
      table.name,
      newName,
    );
    flashToast(`已重命名为 ${newName}`);
    emit("refresh");
  } catch (reason) {
    flashToast(String(reason));
  }
}

async function confirmDelete() {
  const table = deleteTarget.value;
  deleteOpen.value = false;
  if (!table) return;
  try {
    await api.dropObject(props.connectionId, table.kind, table.name);
    flashToast(`已删除 ${table.name}`);
    emit("refresh");
  } catch (reason) {
    flashToast(String(reason));
  }
}

function onWindowClick() {
  closeObjectMenu();
}

function onWindowKey(event: KeyboardEvent) {
  if (event.key === "Escape") closeObjectMenu();
}

function onWindowResize() {
  closeObjectMenu();
}

onMounted(() => {
  window.addEventListener("click", onWindowClick);
  window.addEventListener("keydown", onWindowKey);
  window.addEventListener("resize", onWindowResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", onWindowClick);
  window.removeEventListener("keydown", onWindowKey);
  window.removeEventListener("resize", onWindowResize);
  window.clearTimeout(toastTimer);
  window.clearTimeout(locateTimer);
});

watch(
  () => [props.connectionId, props.driver],
  () => resetColumns(),
);
</script>

<template>
  <div
    v-for="group in groups"
    :key="group.kind"
    class="object-group"
  >
    <button
      class="object-group-header"
      data-tree-nav
      :aria-expanded="search.trim() ? true : !collapsed[group.kind]"
      @click="toggle(group.kind)"
    >
      <Icon
        :name="collapsed[group.kind] ? 'chevron-right' : 'chevron-down'"
        :size="12"
      />
      <span>{{ group.label }}</span>
      <span class="group-count">{{ group.total }}</span>
    </button>

    <div
      v-show="search.trim() || !collapsed[group.kind]"
      class="object-group-items"
    >
      <div v-if="!group.items.length" class="tree-hint">
        {{ group.total ? "没有匹配" : "暂无" }}
      </div>
      <div
        v-for="table in group.items"
        :key="tableKey(table)"
        class="tree-table"
      >
        <div
          class="object-item tree-table-row"
          :data-object-key="tableKey(table)"
          role="button"
          tabindex="0"
          data-tree-nav
          :aria-expanded="
            isExpandable(table)
              ? Boolean(expandedTables[tableKey(table)])
              : undefined
          "
          :class="{
            active: isExpandable(table) && table.name === activeTable,
            located: locatedKey === tableKey(table),
          }"
          :title="
            !isExpandable(table)
              ? `${table.name}（暂不支持查看）`
              : `${table.name} · 单击展开列，右键查看更多操作`
          "
          @click="onTableClick(table)"
          @dblclick="emit('select', table)"
          @contextmenu.prevent.stop="openObjectMenu($event, table)"
        >
          <Icon
            v-if="isExpandable(table)"
            class="tree-chevron"
            :name="
              expandedTables[tableKey(table)]
                ? 'chevron-down'
                : 'chevron-right'
            "
            :size="12"
          />
          <Icon class="object-kind" :name="group.icon" :size="14" />
          <span class="object-name">{{ table.name }}</span>
          <button
            v-if="isExpandable(table)"
            class="tree-open"
            tabindex="-1"
            title="查看数据"
            aria-label="查看数据"
            @click.stop="emit('select', table)"
            @dblclick.stop
          >
            <Icon name="table" :size="13" />
          </button>
        </div>

        <div
          v-if="isExpandable(table) && expandedTables[tableKey(table)]"
          class="column-list"
        >
          <div
            v-if="columnsLoading[tableKey(table)]"
            class="tree-hint"
          >
            加载列…
          </div>
          <div
            v-else-if="columnsError[tableKey(table)]"
            class="tree-hint error"
          >
            {{ columnsError[tableKey(table)] }}
          </div>
          <div
            v-else-if="!columnsByTable[tableKey(table)]?.length"
            class="tree-hint"
          >
            没有列
          </div>
          <div
            v-for="column in columnsByTable[tableKey(table)]"
            :key="column.name"
            class="column-item"
          >
            <span class="column-name" :title="column.name">{{ column.name }}</span>
            <span class="column-type" :title="column.dataType">{{ column.dataType }}</span>
            <span v-if="column.primaryKey" class="pk-badge">PK</span>
          </div>
        </div>
      </div>
    </div>
  </div>

  <div
    v-if="objectMenu"
    class="context-menu object-context-menu"
    role="menu"
    :style="{
      left: `${objectMenu.x}px`,
      top: `${objectMenu.y}px`,
    }"
    @click.stop
  >
    <button
      v-if="isExpandable(objectMenu.table)"
      class="context-menu-item"
      role="menuitem"
      @click="menuOpenData"
    >
      <Icon name="play" :size="14" />
      <span>打开数据</span>
    </button>
    <button
      v-if="isExpandable(objectMenu.table)"
      class="context-menu-item"
      role="menuitem"
      @click="menuOpenStructure"
    >
      <Icon name="structure" :size="14" />
      <span>查看结构</span>
    </button>
    <template v-if="isExpandable(objectMenu.table)">
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuOpenInNewTab"
      >
        <Icon name="table" :size="14" />
        <span>在新标签打开</span>
      </button>
      <button
        v-if="objectMenu.table.kind === 'table'"
        class="context-menu-item"
        role="menuitem"
        @click="menuImportData"
      >
        <Icon name="upload" :size="14" />
        <span>导入数据</span>
      </button>
    </template>
    <div
      v-if="isExpandable(objectMenu.table)"
      class="context-submenu-host"
      @mouseenter="openGenerateMenu"
      @mouseleave="generateOpen = false"
    >
      <button
        class="context-menu-item"
        role="menuitem"
        :aria-expanded="generateOpen"
        @click.stop="toggleGenerateMenu"
      >
        <Icon name="code" :size="14" />
        <span>生成 SQL</span>
        <span class="context-menu-shortcut">▸</span>
      </button>
      <div
        v-if="generateOpen"
        class="context-menu context-submenu"
        role="menu"
      >
        <button
          class="context-menu-item"
          role="menuitem"
          @click="generateSql('select')"
        >
          SELECT
        </button>
        <button
          v-if="objectMenu.table.kind === 'table'"
          class="context-menu-item"
          role="menuitem"
          @click="generateSql('insert')"
        >
          INSERT
        </button>
        <button
          v-if="objectMenu.table.kind === 'table'"
          class="context-menu-item"
          role="menuitem"
          :disabled="menuColumns !== null && !menuHasPrimaryKey"
          @click="generateSql('update')"
        >
          UPDATE
        </button>
        <button
          v-if="objectMenu.table.kind === 'table'"
          class="context-menu-item"
          role="menuitem"
          :disabled="menuColumns !== null && !menuHasPrimaryKey"
          @click="generateSql('delete')"
        >
          DELETE
        </button>
        <div
          v-if="
            objectMenu.table.kind === 'table' &&
            menuColumns !== null &&
            !menuHasPrimaryKey
          "
          class="context-menu-hint"
        >
          无主键，UPDATE/DELETE 不可用
        </div>
        <template v-if="canFetchDdl(objectMenu.table)">
          <div class="context-separator"></div>
          <button
            class="context-menu-item"
            role="menuitem"
            @click="generateSql('ddl')"
          >
            DDL
          </button>
        </template>
      </div>
    </div>
    <button
      v-if="canFetchDdl(objectMenu.table)"
      class="context-menu-item"
      role="menuitem"
      @click="menuCopyDdl"
    >
      <Icon name="copy" :size="14" />
      <span>复制 DDL</span>
    </button>
    <button
      class="context-menu-item"
      role="menuitem"
      @click="menuCopyName"
    >
      <Icon name="copy" :size="14" />
      <span>复制名称</span>
    </button>
    <button
      v-if="canCopyQualifiedName()"
      class="context-menu-item"
      role="menuitem"
      @click="menuCopyQualifiedName"
    >
      <Icon name="copy" :size="14" />
      <span>复制限定名</span>
    </button>

    <template v-if="canExportData(objectMenu.table)">
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuExport"
      >
        <Icon name="download" :size="14" />
        <span>导出…</span>
      </button>
    </template>

    <template v-if="objectMenu.table.kind === 'table'">
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuEmptyTable"
      >
        <Icon name="trash" :size="14" />
        <span>清空表…</span>
      </button>
      <button
        class="context-menu-item danger"
        role="menuitem"
        @click="menuTruncateTable"
      >
        <Icon name="trash" :size="14" />
        <span>截断表…</span>
      </button>
    </template>

    <template v-if="isExpandable(objectMenu.table)">
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        role="menuitem"
        @click="menuCreateTable"
      >
        <Icon name="plus" :size="14" />
        <span>新建表</span>
      </button>
      <button
        v-if="objectMenu.table.kind === 'table'"
        class="context-menu-item"
        role="menuitem"
        @click="menuDuplicate"
      >
        <Icon name="copy" :size="14" />
        <span>复制为新表…</span>
      </button>
      <button
        v-if="objectMenu.table.kind === 'view'"
        class="context-menu-item"
        role="menuitem"
        @click="menuDuplicate"
      >
        <Icon name="copy" :size="14" />
        <span>复制为新视图…</span>
      </button>
      <button
        v-if="objectMenu.table.kind === 'view'"
        class="context-menu-item"
        role="menuitem"
        @click="menuSnapshot"
      >
        <Icon name="table" :size="14" />
        <span>快照为新表…</span>
      </button>
    </template>

    <template v-if="canRename(objectMenu.table) || canDrop(objectMenu.table)">
      <div class="context-separator"></div>
      <button
        v-if="canRename(objectMenu.table)"
        class="context-menu-item"
        role="menuitem"
        @click="menuRename"
      >
        <Icon name="edit" :size="14" />
        <span>重命名…</span>
      </button>
      <button
        v-if="canDrop(objectMenu.table)"
        class="context-menu-item danger"
        role="menuitem"
        @click="menuDelete"
      >
        <Icon name="trash" :size="14" />
        <span>删除{{ kindLabel(objectMenu.table.kind) }}</span>
      </button>
    </template>

    <div class="context-separator"></div>
    <button
      class="context-menu-item"
      role="menuitem"
      @click="menuRefresh"
    >
      <Icon name="refresh" :size="14" />
      <span>刷新对象</span>
    </button>
  </div>

  <PromptDialog
    :open="renameOpen"
    :title="`重命名${renameTarget ? kindLabel(renameTarget.kind) : ''}`"
    :message="`将「${renameTarget?.name ?? ''}」重命名为：`"
    :value="renameTarget?.name ?? ''"
    confirm-text="重命名"
    @confirm="confirmRename"
    @cancel="renameOpen = false"
  />

  <ExportDialog
    :open="exportOpen"
    :kind="exportTarget?.kind ?? 'table'"
    :name="exportTarget?.name ?? ''"
    @confirm="confirmExport"
    @cancel="exportOpen = false"
  />

  <PromptDialog
    :open="copyOpen"
    :title="copyDialogTitle"
    :message="`将「${copyTarget?.name ?? ''}」复制为：`"
    :value="copyDefaultName"
    :confirm-text="copyMode === 'snapshot' ? '创建快照' : '复制'"
    :checkbox-label="
      copyMode === 'snapshot'
        ? '包含数据（取消则只创建空表结构）'
        : undefined
    "
    :checkbox-default="true"
    @confirm="confirmCopy"
    @cancel="copyOpen = false"
  />

  <ConfirmDialog
    :open="deleteOpen"
    title="删除对象"
    :message="`确定删除${deleteTarget ? kindLabel(deleteTarget.kind) : ''}「${deleteTarget?.name ?? ''}」？`"
    detail="此操作不可撤销，请确认对象已不再使用。"
    :confirm-text="`删除${deleteTarget ? kindLabel(deleteTarget.kind) : ''}`"
    @confirm="confirmDelete"
    @cancel="deleteOpen = false"
  />

  <ConfirmDialog
    :open="maintenanceOpen"
    :title="maintenanceTitle"
    :message="maintenanceMessage"
    :detail="maintenanceDetail"
    :confirm-text="maintenanceTitle"
    @confirm="confirmMaintenance"
    @cancel="maintenanceOpen = false"
  />

  <div v-if="exportVisible" class="export-progress" role="status" aria-live="polite">
    <span class="spinner"></span>
    <span>正在导出…</span>
    <button class="button ghost compact" @click="cancelExport">取消</button>
  </div>

  <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
    {{ toast }}
  </div>
</template>