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
import BatchSetDialog from "./BatchSetDialog.vue";
import ChangePreviewDialog from "./ChangePreviewDialog.vue";
import CellValueDialog from "./CellValueDialog.vue";
import FilterBuilderDialog from "./FilterBuilderDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import { qualifiedSqlName } from "../lib/generate-sql";
import {
  buildRowInsert,
  buildRowJson,
  buildRowUpdate,
} from "../lib/row-copy";
import { quoteIdentifier, sqlLiteral } from "../lib/sql-literal";
import { useVirtualRows } from "../lib/virtual-rows";
import type {
  ColumnInfo,
  ColumnValue,
  Driver,
  PendingTableChange,
  TableSort,
} from "../lib/types";

const props = defineProps<{
  connectionId: string;
  table: string;
  tableKind?: string;
  databaseLabel?: string;
  driver?: Driver;
  database?: string;
  schema?: string | null;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "generate-data": [];
  "import-data": [];
}>();

const vFocus = {
  mounted: (element: HTMLInputElement) => element.focus(),
};

interface EditingCell {
  rowIndex: number;
  colIndex: number;
  value: string;
}

interface CellMenu {
  x: number;
  y: number;
  rowIndex: number;
  colIndex: number;
}

interface ResizeState {
  name: string;
  startX: number;
  startWidth: number;
}

interface InsertDraft {
  id: number;
  values: Array<unknown | undefined>;
}

interface DisplayRow {
  values: unknown[];
  origin: number | null;
  insertId?: number;
  state: "clean" | "update" | "delete" | "insert";
}

interface PendingGuard {
  reason: string;
  action: () => void | Promise<void>;
}

const PAGE_SIZE_KEY = "mydb.tablePageSize.v1";

const columns = ref<ColumnInfo[]>([]);
const rows = ref<unknown[][]>([]);
const total = ref(0);
const offset = ref(0);
const limit = ref(100);
const loading = ref(false);
const saving = ref(false);
const importing = ref(false);
const error = ref("");
const message = ref("");
let messageTimer = 0;
const editing = ref<EditingCell | null>(null);
const insertOpen = ref(false);
const insertValues = reactive<Record<string, string>>({});
const sorts = ref<TableSort[]>([]);
const filterOpen = ref(false);
const filterDraft = ref("");
const appliedFilter = ref("");
const columnWidths = reactive<Record<string, number>>({});
const cellMenu = ref<CellMenu | null>(null);
const copyAsOpen = ref(false);
const importRequest = ref<{ path: string } | null>(null);
const importHasHeader = ref(true);
const updates = ref<Record<number, Record<string, unknown>>>({});
const pendingDeletes = ref<Set<number>>(new Set());
const pendingInserts = ref<InsertDraft[]>([]);
const selectedRows = ref<Set<number>>(new Set());
const pageInput = ref("1");
const previewOpen = ref(false);
const batchSetOpen = ref(false);
const filterBuilderOpen = ref(false);
const pendingGuard = ref<PendingGuard | null>(null);
const batchDeleteCount = ref<number | null>(null);
const valueViewer = ref<{
  rowIndex: number;
  colIndex: number;
} | null>(null);
const scrollRef = ref<HTMLElement | null>(null);
let insertDraftSequence = 0;
let selectionAnchor: number | null = null;
let resizeState: ResizeState | null = null;

const primaryKeys = computed(() =>
  columns.value.filter((column) => column.primaryKey),
);
const driverKind = computed<Driver>(() => props.driver ?? "sqlite");
const isTable = computed(() => (props.tableKind ?? "table") === "table");

function isBinaryColumn(column: ColumnInfo): boolean {
  return /(^|[^A-Z])(BLOB|BINARY|VARBINARY|BYTEA|BIT|IMAGE|RAW)([^A-Z]|$)/i.test(
    column.dataType,
  );
}

/**
 * Without a primary key we match the whole row. Binary values are decoded for
 * display, so they cannot be used to identify a row again; leave them out.
 */
const matchColumns = computed(() =>
  columns.value.filter((column) => !isBinaryColumn(column)),
);
const singleRowMatch = computed(() => primaryKeys.value.length === 0);
const canEdit = computed(
  () =>
    !props.readOnly &&
    columns.value.length > 0 &&
    (primaryKeys.value.length > 0 ||
      (isTable.value && matchColumns.value.length > 0)),
);

const displayRows = computed<DisplayRow[]>(() => {
  const list: DisplayRow[] = rows.value.map((row, origin) => {
    const overlay = updates.value[origin];
    let values = row;
    if (overlay) {
      values = row.slice();
      for (const [name, value] of Object.entries(overlay)) {
        const index = columnIndex(name);
        if (index >= 0) values[index] = value;
      }
    }
    return {
      values,
      origin,
      state: pendingDeletes.value.has(origin)
        ? "delete"
        : overlay && Object.keys(overlay).length
          ? "update"
          : "clean",
    };
  });
  for (const draft of pendingInserts.value) {
    list.push({
      values: draft.values as unknown[],
      origin: null,
      insertId: draft.id,
      state: "insert",
    });
  }
  return list;
});

const {
  enabled: virtualEnabled,
  start: virtualStart,
  end: virtualEnd,
  paddingTop: virtualPaddingTop,
  paddingBottom: virtualPaddingBottom,
  scrollToTop,
} = useVirtualRows({
  container: scrollRef,
  rowCount: computed(() => displayRows.value.length),
  rowHeight: 30,
  overscan: 10,
});

const virtualRows = computed(() => {
  const list = displayRows.value.slice(
    virtualStart.value,
    virtualEnd.value,
  );
  return list.map((row, offset) => ({
    row,
    index: virtualStart.value + offset,
  }));
});

// Do not recycle the row while a cell editor is open: commit the pending
// value first, matching the existing blur / Enter behaviour.
watch([virtualStart, virtualEnd], ([start, end]) => {
  const current = editing.value;
  if (current && (current.rowIndex < start || current.rowIndex >= end)) {
    commitEdit();
  }
});

const pendingCount = computed(() => {
  const updated = Object.keys(updates.value).filter(
    (key) => !pendingDeletes.value.has(Number(key)),
  ).length;
  return updated + pendingDeletes.value.size + pendingInserts.value.length;
});
const hasPendingChanges = computed(() => pendingCount.value > 0);
const selectedCount = computed(() => selectedRows.value.size);
const allDisplaySelected = computed(
  () =>
    displayRows.value.length > 0 &&
    displayRows.value.every((_, index) => selectedRows.value.has(index)),
);

const batchSetColumns = computed(() =>
  columns.value.filter((column) => !isBinaryColumn(column)),
);
const updateCopyReason = computed(() => {
  if (loading.value) return "数据加载中";
  if (saving.value) return "正在保存，暂不能复制 UPDATE";
  if (!primaryKeys.value.length) return "无主键，无法生成 UPDATE";
  if (
    !columns.value.some(
      (column) => !column.primaryKey && !column.autoIncrement,
    )
  ) {
    return "没有可更新的列";
  }
  return "";
});
const copyAsLeft = computed(
  () => (cellMenu.value?.x ?? 0) + 210 + 168 > window.innerWidth,
);
const page = computed(() => Math.floor(offset.value / limit.value) + 1);
const pageCount = computed(() =>
  Math.max(1, Math.ceil(total.value / limit.value)),
);
const pendingChanges = computed(() => buildPendingChanges());
const pendingSql = computed(() => buildPendingSql(pendingChanges.value));

function sortDirectionFor(column: string): "asc" | "desc" | null {
  return sorts.value.find((sort) => sort.column === column)?.direction ?? null;
}
const insertColumns = computed(() =>
  columns.value.filter((column) => !column.autoIncrement),
);

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

function isNull(value: unknown): boolean {
  return value === null || value === undefined;
}

function clearMessage() {
  window.clearTimeout(messageTimer);
  message.value = "";
}

function showMessage(text: string) {
  message.value = text;
  window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(clearMessage, 5000);
}

function columnIndex(name: string): number {
  return columns.value.findIndex((column) => column.name === name);
}

function defaultColumnWidth(column: ColumnInfo): number {
  const name = column.name.toLowerCase();
  if (name === "id" || name.endsWith("_id")) return 90;

  const type = column.dataType.toUpperCase();
  if (type.includes("TEXT") || type.includes("BLOB") || type.includes("JSON")) {
    return 200;
  }
  if (type.includes("DATE") || type.includes("TIME")) {
    return 170;
  }
  if (
    type.includes("DECIMAL") ||
    type.includes("NUMERIC") ||
    type.includes("REAL") ||
    type.includes("DOUBLE") ||
    type.includes("FLOAT")
  ) {
    return 110;
  }
  return 100;
}

function columnWidth(name: string): number {
  const stored = columnWidths[name];
  if (stored) return stored;
  const column = columns.value.find((item) => item.name === name);
  return column ? defaultColumnWidth(column) : 140;
}

function startResize(event: MouseEvent, name: string) {
  event.preventDefault();
  event.stopPropagation();
  resizeState = {
    name,
    startX: event.clientX,
    startWidth: columnWidth(name),
  };
  window.addEventListener("mousemove", moveResize);
  window.addEventListener("mouseup", stopResize);
}

function moveResize(event: MouseEvent) {
  if (!resizeState) return;
  columnWidths[resizeState.name] = Math.max(
    60,
    Math.min(
      600,
      resizeState.startWidth + event.clientX - resizeState.startX,
    ),
  );
}

function stopResize() {
  resizeState = null;
  window.removeEventListener("mousemove", moveResize);
  window.removeEventListener("mouseup", stopResize);
}

function conditionFor(column: ColumnInfo, value: unknown): string {
  const identifier = quoteIdentifier(column.name, driverKind.value);
  if (isNull(value)) return `${identifier} IS NULL`;
  return `${identifier} = ${sqlLiteral(value, driverKind.value)}`;
}

function openCellMenu(event: MouseEvent, rowIndex: number, colIndex: number) {
  event.preventDefault();
  cellMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 250)),
    rowIndex,
    colIndex,
  };
}

function closeCellMenu() {
  cellMenu.value = null;
  copyAsOpen.value = false;
}

function onGlobalKey(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeCellMenu();
    if (insertOpen.value) insertOpen.value = false;
    if (importRequest.value) importRequest.value = null;
  }
}

function menuValue(): unknown {
  if (!cellMenu.value) return null;
  return displayRows.value[cellMenu.value.rowIndex]?.values[
    cellMenu.value.colIndex
  ];
}

const viewerValue = computed<unknown>(() => {
  const viewer = valueViewer.value;
  if (!viewer) return null;
  return displayRows.value[viewer.rowIndex]?.values[viewer.colIndex];
});

const viewerColumn = computed(() => {
  const viewer = valueViewer.value;
  if (!viewer) return "";
  return columns.value[viewer.colIndex]?.name ?? "";
});

const viewerType = computed(() => {
  const viewer = valueViewer.value;
  if (!viewer) return "";
  return columns.value[viewer.colIndex]?.dataType ?? "";
});

const viewerRowLabel = computed(() => {
  const viewer = valueViewer.value;
  if (!viewer) return "";
  const row = displayRows.value[viewer.rowIndex];
  if (!row) return "";
  if (row.state === "insert") return "新增行（未提交）";
  return `第 ${offset.value + viewer.rowIndex + 1} 行`;
});

function openValueFromMenu() {
  const menu = cellMenu.value;
  if (!menu) return;
  valueViewer.value = {
    rowIndex: menu.rowIndex,
    colIndex: menu.colIndex,
  };
  closeCellMenu();
}

function closeValueViewer() {
  const viewer = valueViewer.value;
  valueViewer.value = null;
  if (!viewer) return;
  const row = viewer.rowIndex;
  const column = viewer.colIndex;
  void nextTick(() => {
    scrollRef.value
      ?.querySelector<HTMLElement>(
        `[data-cell="${row}:${column}"]`,
      )
      ?.focus();
  });
}

async function copyCellValue() {
  const value = menuValue();
  try {
    await writeClipboard(formatCell(value));
    showMessage("已复制单元格");
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
  closeCellMenu();
}

async function copyRowValues() {
  if (!cellMenu.value) return;
  const row = displayRows.value[cellMenu.value.rowIndex]?.values;
  if (!row) return;
  try {
    await writeClipboard(row.map(formatCell).join("\t"));
    showMessage("已复制整行");
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
  closeCellMenu();
}

function menuRow(): unknown[] | null {
  const menu = cellMenu.value;
  if (!menu) return null;
  return displayRows.value[menu.rowIndex]?.values ?? null;
}

function tableIdentifier(): string {
  return qualifiedSqlName(
    driverKind.value,
    props.table,
    props.database,
    props.schema,
  );
}

async function copyAs(kind: "insert" | "update" | "json") {
  if (loading.value || saving.value) return;
  const row = menuRow();
  if (!row) return;

  let text: string;
  let label: string;
  if (kind === "insert") {
    text = buildRowInsert(
      columns.value,
      row,
      driverKind.value,
      tableIdentifier(),
    );
    label = "INSERT";
  } else if (kind === "update") {
    if (updateCopyReason.value) return;
    const sql = buildRowUpdate(
      columns.value,
      row,
      driverKind.value,
      tableIdentifier(),
    );
    if (!sql) return;
    text = sql;
    label = "UPDATE";
  } else {
    text = buildRowJson(
      columns.value.map((column) => column.name),
      row,
    );
    label = "JSON";
  }

  closeCellMenu();
  try {
    await writeClipboard(text);
    showMessage(`已复制 ${label}`);
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

function filterByValue() {
  if (!cellMenu.value) return;
  const column = columns.value[cellMenu.value.colIndex];
  if (!column) return;
  const condition = conditionFor(column, menuValue());
  closeCellMenu();
  ensureClean("筛选", () => {
    filterDraft.value = condition;
    appliedFilter.value = condition;
    filterOpen.value = true;
    void load(0);
  });
}

function sortByValue(direction: "asc" | "desc") {
  if (!cellMenu.value) return;
  const column = columns.value[cellMenu.value.colIndex];
  if (!column) return;
  closeCellMenu();
  ensureClean("排序", () => {
    sorts.value = [{ column: column.name, direction }];
    void load(0);
  });
}

function deleteFromMenu() {
  if (!cellMenu.value) return;
  const rowIndex = cellMenu.value.rowIndex;
  closeCellMenu();
  void deleteRow(rowIndex);
}

onMounted(() => {
  window.addEventListener("click", closeCellMenu);
  window.addEventListener("keydown", onGlobalKey);
});

onBeforeUnmount(() => {
  window.clearTimeout(messageTimer);
  window.removeEventListener("click", closeCellMenu);
  window.removeEventListener("keydown", onGlobalKey);
  stopResize();
});

async function load(nextOffset = offset.value) {
  loading.value = true;
  error.value = "";
  try {
    const pageData = await api.readTableRows(
      props.connectionId,
      props.table,
      limit.value,
      Math.max(0, nextOffset),
      appliedFilter.value || undefined,
      undefined,
      undefined,
      sorts.value,
    );
    columns.value = pageData.columns;
    rows.value = pageData.rows;
    total.value = pageData.total;
    offset.value = pageData.offset;
    pageInput.value = String(
      Math.floor(pageData.offset / limit.value) + 1,
    );
    selectedRows.value = new Set();
    selectionAnchor = null;
    scrollToTop();
  } catch (reason) {
    rows.value = [];
    total.value = 0;
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

watch(
  [() => props.connectionId, () => props.table],
  () => {
    clearMessage();
    sorts.value = [];
    filterOpen.value = false;
    filterDraft.value = "";
    appliedFilter.value = "";
    for (const key of Object.keys(columnWidths)) {
      delete columnWidths[key];
    }
    cellMenu.value = null;
    columns.value = [];
    rows.value = [];
    total.value = 0;
    clearPendingState();
    rows.value = [];
    const saved = readSavedPageSize();
    limit.value = saved;
    pageInput.value = "1";
    void load(0);
  },
  { immediate: true },
);

function readSavedPageSize(): number {
  try {
    const stored = window.localStorage.getItem(PAGE_SIZE_KEY);
    if (!stored) return 100;
    const parsed = JSON.parse(stored) as Record<string, number>;
    const value = parsed[pageSizeKey()];
    return [50, 100, 200, 500].includes(value) ? value : 100;
  } catch {
    return 100;
  }
}

function pageSizeKey(): string {
  return `${driverKind.value}:${props.table}`;
}

function savePageSize(size: number) {
  try {
    const stored = window.localStorage.getItem(PAGE_SIZE_KEY);
    const parsed = stored ? (JSON.parse(stored) as Record<string, number>) : {};
    parsed[pageSizeKey()] = size;
    window.localStorage.setItem(PAGE_SIZE_KEY, JSON.stringify(parsed));
  } catch {
    // Page size persistence is best-effort.
  }
}

function applyFilter() {
  ensureClean("筛选", () => {
    appliedFilter.value = filterDraft.value.trim();
    void load(0);
  });
}

function clearFilter() {
  ensureClean("清除筛选", () => {
    filterDraft.value = "";
    appliedFilter.value = "";
    filterOpen.value = false;
    void load(0);
  });
}

function applyBuilderFilter(filter: string) {
  filterBuilderOpen.value = false;
  ensureClean("筛选", () => {
    filterDraft.value = filter;
    appliedFilter.value = filter;
    filterOpen.value = true;
    void load(0);
  });
}

function toggleSort(columnName: string, additive = false) {
  ensureClean("排序", () => {
    const existing = sorts.value.find((sort) => sort.column === columnName);
    if (additive) {
      if (!existing) {
        sorts.value = [
          ...sorts.value,
          { column: columnName, direction: "asc" },
        ];
      } else if (existing.direction === "asc") {
        sorts.value = sorts.value.map((sort) =>
          sort.column === columnName ? { ...sort, direction: "desc" } : sort,
        );
      } else {
        sorts.value = sorts.value.filter(
          (sort) => sort.column !== columnName,
        );
      }
    } else if (existing) {
      if (sorts.value.length === 1 && existing.direction === "asc") {
        sorts.value = sorts.value.map((sort) => ({
          ...sort,
          direction: "desc",
        }));
      } else {
        sorts.value = [];
      }
    } else {
      sorts.value = [{ column: columnName, direction: "asc" }];
    }
    void load(0);
  });
}

function removeSort(column: string) {
  ensureClean("排序", () => {
    sorts.value = sorts.value.filter((sort) => sort.column !== column);
    void load(0);
  });
}

function clearSorts() {
  ensureClean("清除排序", () => {
    sorts.value = [];
    void load(0);
  });
}

function startEdit(rowIndex: number, colIndex: number) {
  if (!canEdit.value || saving.value) return;
  const row = displayRows.value[rowIndex];
  if (!row || row.state === "delete") return;
  const value = row.values[colIndex];
  editing.value = {
    rowIndex,
    colIndex,
    value: isNull(value) ? "" : String(value),
  };
}

function parseInput(value: string, column: ColumnInfo): unknown {
  if (value.trim().toUpperCase() === "NULL") return null;
  if (value === "") return column.nullable ? null : "";

  const numeric =
    /^(INT|TINYINT|SMALLINT|MEDIUMINT|BIGINT|DECIMAL|NUMERIC|FLOAT|DOUBLE|REAL)/i.test(
      column.dataType,
    );
  if (numeric) {
    const parsed = Number(value);
    if (!Number.isNaN(parsed)) return parsed;
  }

  return value;
}

function rowKeys(row: unknown[]): ColumnValue[] {
  const keys = primaryKeys.value.length ? primaryKeys.value : matchColumns.value;
  return keys.map((column) => ({
    column: column.name,
    value: row[columnIndex(column.name)],
  }));
}

function sameValue(left: unknown, right: unknown): boolean {
  if (isNull(left) && isNull(right)) return true;
  return left === right;
}

function setCellPending(
  displayIndex: number,
  colIndex: number,
  value: unknown,
) {
  const row = displayRows.value[displayIndex];
  const column = columns.value[colIndex];
  if (!row || !column || row.state === "delete") return;
  if (row.origin === null) {
    const draftIndex = pendingInserts.value.findIndex(
      (draft) => draft.id === row.insertId,
    );
    if (draftIndex >= 0) {
      pendingInserts.value[draftIndex].values[colIndex] = value;
    }
    return;
  }
  const original = rows.value[row.origin]?.[colIndex];
  const overlay = { ...(updates.value[row.origin] ?? {}) };
  if (sameValue(original, value)) {
    delete overlay[column.name];
  } else {
    overlay[column.name] = value;
  }
  const next = { ...updates.value };
  if (Object.keys(overlay).length) {
    next[row.origin] = overlay;
  } else {
    delete next[row.origin];
  }
  updates.value = next;
}

function commitEdit() {
  if (!editing.value) return;
  const current = editing.value;
  const column = columns.value[current.colIndex];
  const row = displayRows.value[current.rowIndex];
  if (!column || !row) {
    editing.value = null;
    return;
  }
  const original = row.values[current.colIndex];
  if (current.value === (isNull(original) ? "" : String(original))) {
    editing.value = null;
    return;
  }
  setCellPending(
    current.rowIndex,
    current.colIndex,
    parseInput(current.value, column),
  );
  editing.value = null;
  showMessage("已加入待提交，尚未写入数据库");
}

function deleteRow(rowIndex: number) {
  if (!canEdit.value || saving.value) return;
  const row = displayRows.value[rowIndex];
  if (!row) return;
  if (row.origin === null) {
    pendingInserts.value = pendingInserts.value.filter(
      (draft) => draft.id !== row.insertId,
    );
    showMessage("已移除新增草稿");
    return;
  }
  if (pendingDeletes.value.has(row.origin)) {
    restoreRow(rowIndex);
    return;
  }
  pendingDeletes.value = new Set([
    ...pendingDeletes.value,
    row.origin,
  ]);
  if (updates.value[row.origin]) {
    const next = { ...updates.value };
    delete next[row.origin];
    updates.value = next;
  }
  editing.value = null;
  showMessage("已标记删除，提交后生效");
}

function restoreRow(rowIndex: number) {
  const row = displayRows.value[rowIndex];
  if (!row || row.origin === null) return;
  const next = new Set(pendingDeletes.value);
  next.delete(row.origin);
  pendingDeletes.value = next;
  showMessage("已撤销删除标记");
}

function requestBatchDelete() {
  const targets = [...selectedRows.value].filter(
    (index) => displayRows.value[index]?.state !== "delete",
  );
  if (!targets.length) return;
  batchDeleteCount.value = targets.length;
}

function confirmBatchDelete() {
  const count = batchDeleteCount.value;
  batchDeleteCount.value = null;
  if (!count) return;
  const deletes = new Set(pendingDeletes.value);
  const insertsToRemove = new Set<number>();
  for (const index of selectedRows.value) {
    const row = displayRows.value[index];
    if (!row || row.state === "delete") continue;
    if (row.origin === null) {
      if (row.insertId !== undefined) insertsToRemove.add(row.insertId);
    } else {
      deletes.add(row.origin);
      if (updates.value[row.origin]) {
        const next = { ...updates.value };
        delete next[row.origin];
        updates.value = next;
      }
    }
  }
  pendingDeletes.value = deletes;
  if (insertsToRemove.size) {
    pendingInserts.value = pendingInserts.value.filter(
      (draft) => !insertsToRemove.has(draft.id),
    );
  }
  selectedRows.value = new Set();
  showMessage(`已标记删除 ${count} 行，提交后生效`);
}

function openInsert() {
  for (const key of Object.keys(insertValues)) {
    delete insertValues[key];
  }
  for (const column of insertColumns.value) {
    insertValues[column.name] = "";
  }
  insertOpen.value = true;
}

function submitInsert() {
  if (saving.value) return;
  const values: Array<unknown | undefined> = columns.value.map(
    () => undefined,
  );
  for (const column of insertColumns.value) {
    const index = columnIndex(column.name);
    if (index >= 0) {
      values[index] = parseInput(insertValues[column.name] ?? "", column);
    }
  }
  pendingInserts.value = [
    ...pendingInserts.value,
    { id: ++insertDraftSequence, values },
  ];
  insertOpen.value = false;
  showMessage("已加入待提交，提交后写入数据库");
}

function buildPendingChanges(): PendingTableChange[] {
  const changes: PendingTableChange[] = [];
  for (const origin of pendingDeletes.value) {
    const row = rows.value[origin];
    if (!row) continue;
    changes.push({
      kind: "delete",
      keys: rowKeys(row),
      changes: [],
      values: [],
      singleRow: singleRowMatch.value,
    });
  }
  for (const [key, overlay] of Object.entries(updates.value)) {
    const origin = Number(key);
    if (pendingDeletes.value.has(origin)) continue;
    const row = rows.value[origin];
    if (!row) continue;
    const columnChanges = Object.entries(overlay).map(([column, value]) => ({
      column,
      value,
    }));
    if (!columnChanges.length) continue;
    changes.push({
      kind: "update",
      keys: rowKeys(row),
      changes: columnChanges,
      values: [],
      singleRow: singleRowMatch.value,
    });
  }
  for (const draft of pendingInserts.value) {
    const values: ColumnValue[] = [];
    draft.values.forEach((value, index) => {
      if (value === undefined) return;
      const column = columns.value[index];
      if (column) values.push({ column: column.name, value });
    });
    changes.push({
      kind: "insert",
      keys: [],
      changes: [],
      values,
      singleRow: false,
    });
  }
  return changes;
}

function buildPendingSql(changes: PendingTableChange[]): string[] {
  const tableRef = tableIdentifier();
  return changes.map((change) => {
    if (change.kind === "delete") {
      const where = change.keys
        .map((key) =>
          isNull(key.value)
            ? `${quoteIdentifier(key.column, driverKind.value)} IS NULL`
            : `${quoteIdentifier(key.column, driverKind.value)} = ${sqlLiteral(
                key.value,
                driverKind.value,
              )}`,
        )
        .join(" AND ");
      return `DELETE FROM ${tableRef} WHERE ${where};`;
    }
    if (change.kind === "update") {
      const sets = change.changes
        .map(
          (item) =>
            `${quoteIdentifier(item.column, driverKind.value)} = ${sqlLiteral(
              item.value,
              driverKind.value,
            )}`,
        )
        .join(", ");
      const where = change.keys
        .map((key) =>
          isNull(key.value)
            ? `${quoteIdentifier(key.column, driverKind.value)} IS NULL`
            : `${quoteIdentifier(key.column, driverKind.value)} = ${sqlLiteral(
                key.value,
                driverKind.value,
              )}`,
        )
        .join(" AND ");
      return `UPDATE ${tableRef} SET ${sets} WHERE ${where};`;
    }
    if (!change.values.length) {
      return driverKind.value === "mysql"
        ? `INSERT INTO ${tableRef} () VALUES ();`
        : `INSERT INTO ${tableRef} DEFAULT VALUES;`;
    }
    const fields = change.values
      .map((item) => quoteIdentifier(item.column, driverKind.value))
      .join(", ");
    const values = change.values
      .map((item) => sqlLiteral(item.value, driverKind.value))
      .join(", ");
    return `INSERT INTO ${tableRef} (${fields}) VALUES (${values});`;
  });
}

function clearPendingState() {
  updates.value = {};
  pendingDeletes.value = new Set();
  pendingInserts.value = [];
  selectedRows.value = new Set();
  selectionAnchor = null;
  editing.value = null;
}

function discardPendingChanges() {
  clearPendingState();
  previewOpen.value = false;
  showMessage("已放弃待提交变更");
}

function reopenPageAfterCommit() {
  const remaining = rows.value.length - pendingDeletes.value.size;
  const nextOffset =
    remaining <= 0 && offset.value > 0
      ? Math.max(0, offset.value - limit.value)
      : offset.value;
  return nextOffset;
}

async function submitPendingChanges(): Promise<boolean> {
  if (!hasPendingChanges.value || saving.value) return false;
  const changes = buildPendingChanges();
  if (!changes.length) return false;
  const nextOffset = reopenPageAfterCommit();
  saving.value = true;
  error.value = "";
  try {
    const result = await api.applyTableChanges(
      props.connectionId,
      props.table,
      changes,
    );
    clearPendingState();
    previewOpen.value = false;
    await load(nextOffset);
    showMessage(`已提交 ${result.applied} 项变更`);
    return true;
  } catch (reason) {
    error.value = String(reason);
    previewOpen.value = false;
    return false;
  } finally {
    saving.value = false;
  }
}

function ensureClean(reason: string, action: () => void | Promise<void>) {
  if (!hasPendingChanges.value) {
    void action();
    return;
  }
  pendingGuard.value = { reason, action };
}

async function guardSubmitAndContinue() {
  const guard = pendingGuard.value;
  if (!guard) return;
  const submitted = await submitPendingChanges();
  if (!submitted) return;
  pendingGuard.value = null;
  await guard.action();
}

async function guardDiscardAndContinue() {
  const guard = pendingGuard.value;
  if (!guard) return;
  discardPendingChanges();
  pendingGuard.value = null;
  await guard.action();
}

function cancelPendingGuard() {
  pendingGuard.value = null;
}

function rowState(index: number): DisplayRow["state"] {
  return displayRows.value[index]?.state ?? "clean";
}

function isCellModified(displayIndex: number, colIndex: number): boolean {
  const row = displayRows.value[displayIndex];
  if (!row || row.origin === null) return false;
  const column = columns.value[colIndex];
  return Boolean(column && updates.value[row.origin]?.[column.name] !== undefined);
}

function toggleRowSelection(index: number, event: MouseEvent) {
  const next = new Set(selectedRows.value);
  if (event.shiftKey && selectionAnchor !== null) {
    const start = Math.min(selectionAnchor, index);
    const end = Math.max(selectionAnchor, index);
    for (let cursor = start; cursor <= end; cursor += 1) next.add(cursor);
  } else if (next.has(index)) {
    next.delete(index);
  } else {
    next.add(index);
  }
  selectionAnchor = index;
  selectedRows.value = next;
}

function toggleSelectAll() {
  selectedRows.value = allDisplaySelected.value
    ? new Set()
    : new Set(displayRows.value.map((_, index) => index));
  selectionAnchor = null;
}

function clearSelection() {
  selectedRows.value = new Set();
  selectionAnchor = null;
}

function batchSetValue(columnName: string, rawValue: string, useNull: boolean) {
  const column = columns.value.find((item) => item.name === columnName);
  if (!column) return;
  const index = columnIndex(columnName);
  if (index < 0) return;
  const value = useNull ? null : parseInput(rawValue, column);
  for (const rowIndex of selectedRows.value) {
    if (rowState(rowIndex) === "delete") continue;
    const row = displayRows.value[rowIndex];
    if (!row) continue;
    if (row.origin === null) {
      const draft = pendingInserts.value.find(
        (item) => item.id === row.insertId,
      );
      if (draft) draft.values[index] = value;
    } else {
      setCellPending(rowIndex, index, value);
    }
  }
  batchSetOpen.value = false;
  showMessage(`已为 ${selectedRows.value.size} 行设置待提交值`);
}

function cloneSelectedRows() {
  const clones: InsertDraft[] = [];
  for (const rowIndex of selectedRows.value) {
    const row = displayRows.value[rowIndex];
    if (!row || row.state === "delete") continue;
    const values = columns.value.map((column, colIndex) =>
      column.autoIncrement || column.primaryKey
        ? undefined
        : row.values[colIndex],
    );
    clones.push({ id: ++insertDraftSequence, values });
  }
  if (!clones.length) return;
  pendingInserts.value = [...pendingInserts.value, ...clones];
  showMessage(`已克隆 ${clones.length} 行到待提交新增`);
}

async function copySelectedRows(as: "tsv" | "json") {
  const selected = [...selectedRows.value]
    .map((index) => displayRows.value[index])
    .filter((row): row is DisplayRow => Boolean(row));
  if (!selected.length) return;
  try {
    if (as === "tsv") {
      const header = columns.value
        .map((column) => column.name)
        .join("\t");
      const lines = selected.map((row) =>
        row.values.map(formatCell).join("\t"),
      );
      await writeClipboard([header, ...lines].join("\n"));
      showMessage(`已复制 ${selected.length} 行为 TSV`);
    } else {
      const payload = selected.map((row) =>
        Object.fromEntries(
          columns.value.map((column, index) => [
            column.name,
            row.values[index] ?? null,
          ]),
        ),
      );
      await writeClipboard(JSON.stringify(payload, null, 2));
      showMessage(`已复制 ${selected.length} 行为 JSON`);
    }
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

function goToPage() {
  const target = Math.max(
    1,
    Math.min(pageCount.value, Number(pageInput.value) || 1),
  );
  pageInput.value = String(target);
  if (target === page.value) return;
  ensureClean("切换页码", () => {
    void load((target - 1) * limit.value);
  });
}

function csvEscape(value: unknown): string {
  if (isNull(value)) return "";
  const text = typeof value === "object" ? JSON.stringify(value) : String(value);
  if (/[",\n\r]/.test(text)) {
    return `"${text.replaceAll('"', '""')}"`;
  }
  return text;
}

async function exportCsv() {
  const lines = [columns.value.map((column) => csvEscape(column.name)).join(",")];
  for (const row of rows.value) {
    lines.push(row.map(csvEscape).join(","));
  }

  const path = await saveDialog({
    defaultPath: `${props.table}.csv`,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (typeof path !== "string") return;

  try {
    await api.writeTextFile(path, lines.join("\n"));
    showMessage(`已导出当前页到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

async function exportJson() {
  const path = await saveDialog({
    defaultPath: `${props.table}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;

  try {
    const payload = {
      table: props.table,
      columns: columns.value,
      rows: rows.value,
    };
    await api.writeTextFile(path, JSON.stringify(payload, null, 2));
    showMessage(`已导出当前页到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

async function exportXlsx() {
  const path = await saveDialog({
    defaultPath: `${props.table}.xlsx`,
    filters: [{ name: "Excel 工作簿", extensions: ["xlsx"] }],
  });
  if (typeof path !== "string") return;

  try {
    const summary = await api.exportQueryResultXlsx(
      columns.value.map((column) => column.name),
      rows.value,
      path,
      props.table,
      true,
    );
    showMessage(`已导出当前页 ${summary.rows} 行到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

function confirmImport() {
  const request = importRequest.value;
  if (!request || importing.value) return;
  importRequest.value = null;
  ensureClean("导入 CSV", async () => {
    importing.value = true;
    error.value = "";
    try {
      const count = await api.importCsv(
        props.connectionId,
        props.table,
        request.path,
        importHasHeader.value,
      );
      await load(offset.value);
      showMessage(`已导入 ${count} 行`);
    } catch (reason) {
      error.value = String(reason);
    } finally {
      importing.value = false;
    }
  });
}

function changePage(next: number) {
  const target = Math.max(0, Math.min(pageCount.value - 1, next)) * limit.value;
  ensureClean("切换页码", () => {
    void load(target);
  });
}

function changeSize(event: Event) {
  const size = Number((event.target as HTMLSelectElement).value);
  ensureClean("调整每页行数", () => {
    limit.value = size;
    savePageSize(size);
    pageInput.value = "1";
    void load(0);
  });
}

function requestRefresh() {
  ensureClean("刷新", () => {
    void load(offset.value);
  });
}

defineExpose({
  hasPendingChanges,
  discardPendingChanges,
  submitPendingChanges,
});
</script>

<template>
  <section class="data-view">
    <header class="data-toolbar">
      <div class="data-title">
        <strong>{{ table }}</strong>
        <span v-if="databaseLabel" class="data-context">
          {{ databaseLabel }}
        </span>
        <span class="hint">{{ total }} 行</span>
        <span v-if="columns.length && !canEdit" class="hint warning">
          {{ isTable ? "没有可编辑的字段" : "视图不支持行内编辑和删除" }}
        </span>
        <span v-else-if="columns.length && singleRowMatch" class="hint warning">
          无主键，按整行匹配，仅修改 / 删除 1 行
        </span>
        <span v-else-if="columns.length" class="hint">
          双击单元格编辑
        </span>
      </div>
      <div class="data-actions">
        <template v-if="pendingCount">
          <span class="pending-badge">{{ pendingCount }} 项待提交</span>
          <button class="button ghost compact" :disabled="saving" @click="previewOpen = true">
            查看 SQL
          </button>
          <button class="button primary compact" :disabled="saving" @click="submitPendingChanges">
            {{ saving ? "提交中…" : "提交" }}
          </button>
          <button class="button ghost compact" :disabled="saving" @click="discardPendingChanges">
            放弃
          </button>
        </template>
        <button
          class="button ghost compact"
          :disabled="loading"
          @click="requestRefresh"
        >
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button
          class="button ghost compact"
          :disabled="!columns.length || readOnly"
          @click="openInsert"
        >
          <Icon name="plus" :size="14" />
          新增行
        </button>
        <button
          class="button ghost compact"
          :disabled="importing || readOnly"
          @click="emit('import-data')"
        >
          <Icon name="upload" :size="14" />
          {{ importing ? "导入中…" : "导入数据…" }}
        </button>
        <button
          class="button ghost compact"
          :disabled="!isTable || readOnly"
          :title="
            readOnly
              ? '当前连接为只读'
              : '按列生成测试数据，在一个事务中写入'
          "
          @click="emit('generate-data')"
        >
          <Icon name="bolt" :size="14" />
          生成数据…
        </button>
        <button
          class="button ghost compact"
          :disabled="!rows.length"
          @click="exportCsv"
        >
          <Icon name="download" :size="14" />
          导出当前页 CSV
        </button>
        <button
          class="button ghost compact"
          :disabled="!rows.length"
          @click="exportJson"
        >
          <Icon name="download" :size="14" />
          导出当前页 JSON
        </button>
        <button
          class="button ghost compact"
          :disabled="!rows.length"
          @click="exportXlsx"
        >
          <Icon name="download" :size="14" />
          导出当前页 XLSX
        </button>
      </div>
    </header>

    <div class="data-filterbar">
      <button
        class="button ghost compact filter-toggle"
        :class="{ active: filterOpen || appliedFilter }"
        @click="filterOpen = !filterOpen"
      >
        <Icon name="filter" :size="14" />
        WHERE
      </button>
      <input
        v-if="filterOpen"
        v-model="filterDraft"
        v-focus
        class="filter-input"
        placeholder="例如 status = 'paid'"
        @keydown.enter="applyFilter"
      />
      <button
        v-if="filterOpen"
        class="button ghost compact"
        @click="filterBuilderOpen = true"
      >
        构建筛选…
      </button>
      <button
        v-if="filterOpen"
        class="button primary compact"
        @click="applyFilter"
      >
        应用
      </button>
      <button
        v-if="appliedFilter"
        class="button ghost compact"
        @click="clearFilter"
      >
        清除
      </button>
      <span
        v-if="appliedFilter"
        class="filter-meta"
        :title="appliedFilter"
      >
        已筛选
      </span>
      <span v-else class="filter-meta">全部行</span>
      <div class="spacer"></div>
      <div v-if="sorts.length" class="sort-chips">
        <button
          v-for="sort in sorts"
          :key="sort.column"
          class="sort-chip"
          :title="`移除 ${sort.column} 排序`"
          @click="removeSort(sort.column)"
        >
          {{ sort.column }} {{ sort.direction.toUpperCase() }}
          <Icon name="close" :size="10" />
        </button>
        <button class="sort-chip clear" @click="clearSorts">清除排序</button>
      </div>
    </div>

    <div v-if="selectedCount" class="data-selectionbar">
      <span>已选 {{ selectedCount }} 行</span>
      <button class="button ghost compact" @click="toggleSelectAll">
        {{ allDisplaySelected ? "清空选择" : "全选当前页" }}
      </button>
      <button class="button ghost compact"
        @click="batchSetOpen = true"
        :disabled="readOnly"
      >
        批量设置值…
      </button>
      <button class="button ghost compact" @click="copySelectedRows('tsv')">
        复制 TSV
      </button>
      <button class="button ghost compact" @click="copySelectedRows('json')">
        复制 JSON
      </button>
      <button class="button ghost compact" @click="cloneSelectedRows" :disabled="readOnly">
        克隆为新行
      </button>
      <button
        class="button ghost compact danger"
        @click="requestBatchDelete"
        :disabled="readOnly"
      >
        删除所选
      </button>
      <button class="button ghost compact" @click="clearSelection">
        清空选择
      </button>
    </div>

    <p v-if="message" class="data-message success" role="status">
      <span class="data-message-text">{{ message }}</span>
      <button
        class="data-message-close"
        title="关闭提示"
        aria-label="关闭提示"
        @click="clearMessage"
      >
        <Icon name="close" :size="12" />
      </button>
    </p>
    <p v-if="error" class="data-message error" role="alert">
      <span class="data-message-text">{{ error }}</span>
      <button
        class="data-message-close"
        title="关闭提示"
        aria-label="关闭提示"
        @click="error = ''"
      >
        <Icon name="close" :size="12" />
      </button>
    </p>

    <div ref="scrollRef" class="data-grid-wrap">
      <div v-if="loading" class="panel-empty">加载中…</div>
      <div v-else-if="error" class="panel-empty">加载失败</div>
      <div v-else-if="!rows.length" class="panel-empty">
        暂无数据
      </div>
      <table
        v-else
        class="result-grid data-grid"
        :class="{ virtualized: virtualEnabled }"
      >
        <colgroup>
          <col style="width: 32px" />
          <col style="width: 44px" />
          <col
            v-for="column in columns"
            :key="column.name"
            :style="{ width: columnWidth(column.name) + 'px' }"
          />
        </colgroup>
        <thead>
          <tr>
            <th class="row-select">
              <input
                type="checkbox"
                :checked="allDisplaySelected"
                aria-label="全选当前页"
                @change="toggleSelectAll"
              />
            </th>
            <th class="row-index">#</th>
            <th
              v-for="column in columns"
              :key="column.name"
              class="sortable"
              tabindex="0"
              :aria-sort="
                sortDirectionFor(column.name) === 'asc'
                  ? 'ascending'
                  : sortDirectionFor(column.name) === 'desc'
                    ? 'descending'
                    : 'none'
              "
              @click="toggleSort(column.name, $event.shiftKey)"
              @keydown.enter.prevent="toggleSort(column.name, $event.shiftKey)"
              @keydown.space.prevent="toggleSort(column.name, $event.shiftKey)"
            >
              <span class="column-head">
                <span>{{ column.name }}</span>
                <span v-if="column.primaryKey" class="pk-badge">PK</span>
                <Icon
                  v-if="sortDirectionFor(column.name)"
                  :name="sortDirectionFor(column.name) === 'asc' ? 'chevron-up' : 'chevron-down'"
                  :size="12"
                  class="sort-icon"
                />
              </span>
              <span class="column-type">{{ column.dataType }}</span>
              <span
                class="col-resizer"
                title="拖拽调整列宽"
                @mousedown="startResize($event, column.name)"
                @click.stop
              ></span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-if="virtualEnabled && virtualPaddingTop > 0"
            class="virtual-spacer"
            aria-hidden="true"
          >
            <td
              :colspan="columns.length + 2"
              :style="{ height: `${virtualPaddingTop}px` }"
            ></td>
          </tr>
          <tr
            v-for="entry in virtualRows"
            :key="entry.row.origin ?? `new-${entry.row.insertId}`"
            class="data-row"
            :class="{
              'row-deleted': entry.row.state === 'delete',
              'row-inserted': entry.row.state === 'insert',
            }"
          >
            <td class="row-select">
              <input
                type="checkbox"
                :checked="selectedRows.has(entry.index)"
                :aria-label="`选择第 ${entry.index + 1} 行`"
                @click="toggleRowSelection(entry.index, $event)"
              />
            </td>
            <td class="row-index">{{ offset + entry.index + 1 }}</td>
            <td
              v-for="(cell, colIndex) in entry.row.values"
              :key="colIndex"
              :class="{
                null: isNull(cell),
                num: typeof cell === 'number',
                editable: canEdit,
                modified: isCellModified(entry.index, colIndex),
              }"
              :title="formatCell(cell)"
              tabindex="-1"
              :data-cell="`${entry.index}:${colIndex}`"
              @dblclick="startEdit(entry.index, colIndex)"
              @contextmenu.prevent="
                openCellMenu($event, entry.index, colIndex)
              "
            >
              <input
                v-if="
                  editing &&
                  editing.rowIndex === entry.index &&
                  editing.colIndex === colIndex
                "
                v-model="editing.value"
                v-focus
                class="data-edit-input"
                @keydown.enter.prevent="commitEdit"
                @keydown.esc.prevent="editing = null"
                @blur="commitEdit"
              />
              <template v-else>{{ formatCell(cell) }}</template>
            </td>
          </tr>
          <tr
            v-if="virtualEnabled && virtualPaddingBottom > 0"
            class="virtual-spacer"
            aria-hidden="true"
          >
            <td
              :colspan="columns.length + 2"
              :style="{ height: `${virtualPaddingBottom}px` }"
            ></td>
          </tr>
        </tbody>
      </table>
    </div>

    <footer class="data-pagination">
      <span>第 {{ page }} / {{ pageCount }} 页 · 共 {{ total }} 行</span>
      <div class="pagination-actions">
        <select :value="limit" @change="changeSize">
          <option :value="50">50 行/页</option>
          <option :value="100">100 行/页</option>
          <option :value="200">200 行/页</option>
          <option :value="500">500 行/页</option>
        </select>
        <label class="page-jump">
          跳到
          <input
            v-model="pageInput"
            inputmode="numeric"
            aria-label="页码"
            @keydown.enter.prevent="goToPage"
            @blur="goToPage"
          />
          页
        </label>
        <button
          class="button ghost compact icon-only"
          title="上一页"
          aria-label="上一页"
          :disabled="loading || page <= 1"
          @click="changePage(page - 2)"
        >
          <Icon name="chevron-left" :size="14" />
        </button>
        <button
          class="button ghost compact icon-only"
          title="下一页"
          aria-label="下一页"
          :disabled="loading || page >= pageCount"
          @click="changePage(page)"
        >
          <Icon name="chevron-right" :size="14" />
        </button>
      </div>
    </footer>

    <div
      v-if="importRequest"
      class="modal-backdrop"
      @click.self="importRequest = null"
    >
      <div
        class="modal connection-modal"
        role="dialog"
        aria-modal="true"
        aria-label="导入 CSV"
      >
        <header class="modal-header">
          <div>
            <h2>导入 CSV</h2>
            <p>{{ databaseLabel ? `${databaseLabel} · ` : "" }}{{ table }}</p>
          </div>
          <button
            class="icon-button"
            title="关闭"
            aria-label="关闭导入"
            @click="importRequest = null"
          >
            <Icon name="close" :size="14" />
          </button>
        </header>
        <div class="connection-form">
          <div class="form-grid">
            <label class="span-2">
              目标表
              <input
                :value="`${databaseLabel ? databaseLabel + ' · ' : ''}${table}`"
                readonly
              />
            </label>
            <label class="span-2">
              CSV 文件
              <input :value="importRequest.path" readonly />
            </label>
            <label class="checkbox-row span-2">
              <input v-model="importHasHeader" type="checkbox" />
              首行为列名
            </label>
          </div>
          <p class="message warning">
            数据将追加写入现有表，不会清空已有行。
          </p>
        </div>
        <footer class="modal-footer">
          <div class="spacer"></div>
          <button class="button ghost" @click="importRequest = null">
            取消
          </button>
          <button
            class="button primary"
            :disabled="importing"
            @click="confirmImport"
          >
            {{ importing ? "导入中…" : "导入" }}
          </button>
        </footer>
      </div>
    </div>

    <div v-if="insertOpen" class="modal-backdrop" @click.self="insertOpen = false">
      <div
        class="modal connection-modal"
        role="dialog"
        aria-modal="true"
        aria-label="新增行"
      >
        <header class="modal-header">
          <div>
            <h2>新增行</h2>
            <p>{{ table }}</p>
          </div>
          <button
          class="icon-button"
          title="关闭"
          aria-label="关闭新增行"
          @click="insertOpen = false"
        >
          <Icon name="close" :size="14" />
        </button>
        </header>
        <div class="connection-form">
          <div class="form-grid">
            <label
              v-for="column in insertColumns"
              :key="column.name"
              :class="{ 'span-2': insertColumns.length === 1 }"
            >
              {{ column.name }}
              <span class="column-hint">
                {{ column.dataType }}{{ column.nullable ? " · 可空" : "" }}
              </span>
              <input
                v-model="insertValues[column.name]"
                :placeholder="column.nullable ? '留空表示 NULL' : '必填'"
              />
            </label>
          </div>
        </div>
        <footer class="modal-footer">
          <div class="spacer"></div>
          <button class="button ghost" @click="insertOpen = false">取消</button>
          <button class="button primary" :disabled="saving" @click="submitInsert">
            {{ saving ? "插入中…" : "插入" }}
          </button>
        </footer>
      </div>
    </div>

    <ConfirmDialog
      :open="batchDeleteCount !== null"
      title="标记删除行"
      :message="`将选中的 ${batchDeleteCount ?? 0} 行标记为待删除？`"
      detail="只有点击「提交」后才会写入数据库；提交前可撤销。"
      confirm-text="标记删除"
      @confirm="confirmBatchDelete"
      @cancel="batchDeleteCount = null"
    />

    <ConfirmDialog
      :open="!!pendingGuard"
      title="存在待提交变更"
      :message="`当前有 ${pendingCount} 项待提交变更。${
        pendingGuard?.reason ?? '继续操作'
      }前需要提交或放弃。`"
      detail="提交失败时会保留全部变更并整体回滚。"
      cancel-text="取消"
      extra-text="放弃并继续"
      confirm-text="提交并继续"
      :danger="false"
      @confirm="guardSubmitAndContinue"
      @extra="guardDiscardAndContinue"
      @cancel="cancelPendingGuard"
    />

    <BatchSetDialog
      :open="batchSetOpen"
      :columns="batchSetColumns"
      :selected-count="selectedCount"
      @close="batchSetOpen = false"
      @apply="batchSetValue"
    />

    <ChangePreviewDialog
      :open="previewOpen"
      :table="table"
      :statements="pendingSql"
      :saving="saving"
      @close="previewOpen = false"
      @submit="submitPendingChanges"
    />

    <FilterBuilderDialog
      :open="filterBuilderOpen"
      :columns="columns"
      :driver="driverKind"
      @close="filterBuilderOpen = false"
      @apply="applyBuilderFilter"
    />

    <div
      v-if="cellMenu"
      class="context-menu"
      role="menu"
      :style="{ left: cellMenu.x + 'px', top: cellMenu.y + 'px' }"
      @click.stop
    >
      <button class="context-menu-item" @click="openValueFromMenu">
        <Icon name="view" :size="14" />
        查看值…
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyCellValue">
        <Icon name="copy" :size="14" />
        复制单元格
      </button>
      <button class="context-menu-item" @click="copyRowValues">
        <Icon name="copy" :size="14" />
        复制整行
      </button>
      <div
        class="context-submenu-host"
        @mouseenter="copyAsOpen = true"
        @mouseleave="copyAsOpen = false"
      >
        <button
          class="context-menu-item"
          :disabled="loading || saving"
          @click.stop="copyAsOpen = !copyAsOpen"
        >
          <Icon name="copy" :size="14" />
          <span>复制为</span>
          <span class="context-menu-shortcut">▸</span>
        </button>
        <div
          v-if="copyAsOpen"
          class="context-menu context-submenu"
          :class="{ 'context-submenu-left': copyAsLeft }"
          role="menu"
        >
          <button
            class="context-menu-item"
            :disabled="loading || saving"
            @click="copyAs('insert')"
          >
            INSERT
          </button>
          <button
            class="context-menu-item"
            :disabled="Boolean(updateCopyReason)"
            :title="updateCopyReason || '复制为 UPDATE'"
            @click="copyAs('update')"
          >
            UPDATE
          </button>
          <button
            class="context-menu-item"
            :disabled="loading || saving"
            @click="copyAs('json')"
          >
            JSON
          </button>
        </div>
      </div>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="filterByValue">
        <Icon name="filter" :size="14" />
        按此值筛选
      </button>
      <button class="context-menu-item" @click="sortByValue('asc')">
        <Icon name="chevron-up" :size="14" />
        升序排序
      </button>
      <button class="context-menu-item" @click="sortByValue('desc')">
        <Icon name="chevron-down" :size="14" />
        降序排序
      </button>
      <template v-if="canEdit">
        <div class="context-separator"></div>
        <button class="context-menu-item danger" @click="deleteFromMenu">
          <Icon name="trash" :size="14" />
          {{
            cellMenu && rowState(cellMenu.rowIndex) === "delete"
              ? "撤销删除标记"
              : "标记删除此行"
          }}
        </button>
      </template>
    </div>

    <CellValueDialog
      :open="Boolean(valueViewer)"
      :column="viewerColumn"
      :row-label="viewerRowLabel"
      :data-type="viewerType"
      :value="viewerValue"
      @close="closeValueViewer"
    />
  </section>
</template>
