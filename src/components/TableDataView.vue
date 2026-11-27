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
import ColumnFormatDialog from "./ColumnFormatDialog.vue";
import ColumnVisibilityPanel from "./ColumnVisibilityPanel.vue";
import FilterBuilderDialog from "./FilterBuilderDialog.vue";
import GridFindBar from "./GridFindBar.vue";
import Icon from "./Icon.vue";
import PasteRowsDialog from "./PasteRowsDialog.vue";
import ResultProfile from "./ResultProfile.vue";
import RowDetailPanel from "./RowDetailPanel.vue";
import TableDataExportDialog from "./TableDataExportDialog.vue";
import TableViewMenu from "./TableViewMenu.vue";
import {
  countMatchRows,
  findGridMatches,
  formatGridCell,
  highlightSegments,
  type GridMatch,
} from "../lib/grid-search";
import {
  columnFormatScopeKey,
  formatDisplayValue,
  formatRuleSummary,
  loadTableFormats,
  pruneFormats,
  saveTableFormats,
  type ColumnFormatEntry,
  type ColumnFormatRule,
} from "../lib/column-format";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import {
  autoHiddenKeys as computeAutoHiddenKeys,
  findEmptyColumnKeys,
  keepOneAutoVisible,
  type AutoHideMode,
  type ColumnVisibilityColumn,
} from "../lib/column-visibility";
import { qualifiedSqlName } from "../lib/generate-sql";
import {
  loadScopeForeignKeys,
  peekScopeForeignKeys,
} from "../lib/foreign-key-cache";
import {
  applyColumnOrder,
  columnOrderScopeKey,
  loadColumnOrder,
  moveColumnKey,
  normalizeColumnOrder,
  saveColumnOrder,
} from "../lib/column-order";
import {
  frozenOffsets,
  frozenScopeKey,
  loadFrozenNames,
  normalizeFrozenNames,
  saveFrozenNames,
} from "../lib/frozen-columns";
import {
  createTableViewProfile,
  loadTableViewState,
  profileColumnKeys,
  saveTableViewState,
  tableViewScopeKey,
  tableViewSignature,
  MAX_PROFILES_PER_TABLE,
  TABLE_VIEW_PAGE_SIZES,
  type TableViewProfile,
} from "../lib/table-views";
import {
  clonePendingSnapshot,
  pushPendingHistory,
  takePendingHistory,
  type PendingSnapshot,
} from "../lib/pending-history";
import {
  buildRowInsert,
  buildRowJson,
  buildRowUpdate,
} from "../lib/row-copy";
import { quoteIdentifier, sqlLiteral } from "../lib/sql-literal";
import type { ChartData } from "../lib/result-chart";
import { useVirtualRows } from "../lib/virtual-rows";
import type {
  ColumnInfo,
  ColumnValue,
  Driver,
  PendingTableChange,
  RowDetailField,
  ScopeForeignKey,
  TableInfo,
  TableSort,
} from "../lib/types";

const props = withDefaults(
  defineProps<{
    connectionId: string;
    table: string;
    tableKind?: string;
    databaseLabel?: string;
    driver?: Driver;
    database?: string;
    schema?: string | null;
    readOnly?: boolean;
    /// Split panes pass false so they never overwrite the main table's pins.
    persistFrozen?: boolean;
    tables?: TableInfo[];
    initialFilter?: string;
    linkedFilterLabel?: string;
  }>(),
  { persistFrozen: true },
);

const emit = defineEmits<{
  "generate-data": [];
  "import-data": [];
  "open-linked-table": [
    payload: { table: string; filter: string; label: string },
  ];
  "connection-error": [message: string];
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
  key: string;
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
const undoStack = ref<PendingSnapshot[]>([]);
const redoStack = ref<PendingSnapshot[]>([]);
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
const detailOpen = ref(false);
const detailRowIndex = ref(0);
const frozenIndices = ref<number[]>([]);
const frozenNames = ref<string[]>([]);
const hiddenNames = ref<string[]>([]);
const columnOrder = ref<string[]>([]);
let columnOrderSaveTimer = 0;
let pendingColumnOrderSave: { scope: string; order: string[] } | null = null;
let columnDrag: {
  index: number;
  startX: number;
  startY: number;
  frozen: boolean;
  moved: boolean;
  target: number | null;
  after: boolean;
} | null = null;
const columnDragState = ref<{
  index: number;
  target: number | null;
  after: boolean;
} | null>(null);
let suppressHeaderClick = false;
const viewProfiles = ref<TableViewProfile[]>([]);
const activeViewId = ref<string | null>(null);
const defaultViewId = ref<string | null>(null);
const viewBaseline = ref<string | null>(null);
let pendingColumnView: TableViewProfile | null = null;
const headerMenu = ref<{
  x: number;
  y: number;
  colIndex: number;
} | null>(null);
const formatEntries = ref<ColumnFormatEntry[]>([]);
const formatDialog = ref<{ colIndex: number } | null>(null);
const columnPanelOpen = ref(false);
const autoHideMode = ref<AutoHideMode>("off");
const autoExceptions = reactive(new Set<string>());
const pasteOpen = ref(false);
const tableExportOpen = ref(false);
const profileOpen = ref(false);
const foreignKeys = ref<ScopeForeignKey[]>([]);
const foreignKeysLoading = ref(false);
const foreignKeysError = ref("");
const scrollRef = ref<HTMLElement | null>(null);
const rootRef = ref<HTMLElement | null>(null);
const findOpen = ref(false);
const findQuery = ref("");
const findTerm = ref("");
const findCaseSensitive = ref(false);
const findIndex = ref(0);
const findBar = ref<InstanceType<typeof GridFindBar> | null>(null);
let findTimer = 0;
let restoreFocus: HTMLElement | null = null;
let insertDraftSequence = 0;
let selectionAnchor: number | null = null;
let resizeState: ResizeState | null = null;

const primaryKeys = computed(() =>
  columns.value.filter((column) => column.primaryKey),
);
const profileKeys = computed(() => profileColumnKeys(columns.value));
const hiddenSet = computed(() => new Set(hiddenNames.value));
const orderedColumnIndices = computed(() => {
  const keys = profileKeys.value;
  const orderedKeys = applyColumnOrder(keys, columnOrder.value);
  const indexByKey = new Map(keys.map((key, index) => [key, index]));
  return orderedKeys
    .map((key) => indexByKey.get(key))
    .filter((index): index is number => index !== undefined);
});

const visibilityColumns = computed<ColumnVisibilityColumn[]>(() =>
  columns.value.map((column, index) => ({
    key: profileKeys.value[index] ?? column.name,
    name: column.name,
    index,
    comment: column.comment,
  })),
);

const autoAnalysisRows = computed(() =>
  displayRows.value
    .filter((row) => row.state !== "delete")
    .map((row) => row.values),
);

const emptyColumnKeys = computed(() =>
  findEmptyColumnKeys(
    autoAnalysisRows.value,
    visibilityColumns.value,
    { mode: autoHideMode.value },
  ),
);

const manualHiddenKeys = computed(() =>
  visibilityColumns.value
    .filter((column) => hiddenSet.value.has(column.name))
    .map((column) => column.key),
);

const autoHiddenKeys = computed(() => {
  if (autoHideMode.value === "off") return new Set<string>();
  return autoHiddenKeysForColumns();
});

function autoHiddenKeysForColumns(): Set<string> {
  const exceptions = new Set(
    visibilityColumns.value
      .filter((column) => autoExceptions.has(column.key))
      .map((column) => column.key),
  );
  const keys = computeAutoHiddenKeys(
    visibilityColumns.value,
    emptyColumnKeys.value,
    exceptions,
  );
  keepOneAutoVisible(visibilityColumns.value, new Set(manualHiddenKeys.value), keys);
  return keys;
}

const autoHiddenIndexSet = computed(() => {
  const keys = autoHiddenKeys.value;
  return new Set(
    visibilityColumns.value
      .filter((column) => keys.has(column.key))
      .map((column) => column.index),
  );
});

const visibleColumnEntries = computed(() => {
  const entries = orderedColumnIndices.value
    .map((index) => ({ column: columns.value[index], index }))
    .filter(
      (entry) =>
        entry.column &&
        !hiddenSet.value.has(entry.column.name) &&
        !autoHiddenIndexSet.value.has(entry.index),
    );
  const frozen = entries.filter((entry) =>
    frozenIndices.value.includes(entry.index),
  );
  const rest = entries.filter(
    (entry) => !frozenIndices.value.includes(entry.index),
  );
  return [...frozen, ...rest];
});
const visibleColumns = computed(() =>
  visibleColumnEntries.value.map((entry) => entry.column),
);
const visibleIndices = computed(() =>
  visibleColumnEntries.value.map((entry) => entry.index),
);
const visibleColumnNames = computed(() =>
  visibleColumnEntries.value.map((entry) => entry.column.name),
);
const hiddenColumnCount = computed(
  () => columns.value.length - visibleColumns.value.length,
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

const pasteDisabledReason = computed(() => {
  if (props.readOnly) return "当前连接为只读";
  if (!isTable.value) return "视图不支持新增行";
  if (!columns.value.length) return "当前表没有可编辑字段";
  if (pendingInserts.value.length >= 10_000) {
    return "待提交新增行已达 10,000 行上限";
  }
  return "";
});

function openPasteDialog() {
  if (pasteDisabledReason.value) {
    showMessage(pasteDisabledReason.value);
    return;
  }
  commitEdit();
  pasteOpen.value = true;
}

function applyPastedRows(
  drafts: Array<Array<unknown | undefined>>,
  skipped: number,
) {
  if (!drafts.length) return;
  if (pendingInserts.value.length + drafts.length > 10_000) {
    showMessage("待提交新增行超过 10,000 行，已取消本次粘贴");
    return;
  }
  recordHistory();
  const created = drafts.map((values) => ({
    id: ++insertDraftSequence,
    values,
  }));
  pendingInserts.value = [...pendingInserts.value, ...created];
  selectedRows.value = new Set();
  selectionAnchor = null;
  pasteOpen.value = false;
  showMessage(
    `已粘贴 ${created.length} 行到待提交${
      skipped ? `（跳过 ${skipped} 行）` : ""
    }`,
  );
}

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
  scrollToRow,
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

const findMatches = computed<GridMatch[]>(() =>
  findGridMatches(
    displayRows.value.map((row) => row.values),
    visibleIndices.value.map((index) => ({ index })),
    findTerm.value,
    {
      caseSensitive: findCaseSensitive.value,
      display: (value, columnIndex) => displayCellText(value, columnIndex),
    },
  ),
);
const findRowCount = computed(() => countMatchRows(findMatches.value));
const currentMatch = computed(
  () => findMatches.value[findIndex.value] ?? null,
);
const matchCellKeys = computed(() => {
  const keys = new Set<string>();
  for (const match of findMatches.value) {
    keys.add(`${match.rowIndex}:${match.columnIndex}`);
  }
  return keys;
});
const findScopeHint = computed(
  () =>
    `仅搜索当前页 ${displayRows.value.length} 行的可见列，全表请用 WHERE；不改变查询与导出范围`,
);
const findHighlights = computed(() => {
  const highlights = new Map<
    string,
    ReturnType<typeof highlightSegments>
  >();
  if (!findTerm.value.trim()) return highlights;
  for (const entry of virtualRows.value) {
    for (const columnIndex of visibleIndices.value) {
      const key = `${entry.index}:${columnIndex}`;
      if (!matchCellKeys.value.has(key)) continue;
      highlights.set(
        key,
        highlightSegments(
          displayCellText(entry.row.values[columnIndex], columnIndex),
          findTerm.value,
          findCaseSensitive.value,
        ),
      );
    }
  }
  return highlights;
});

watch(
  [findQuery, findCaseSensitive],
  ([query]) => {
    window.clearTimeout(findTimer);
    findTimer = window.setTimeout(() => {
      findTerm.value = query;
      findIndex.value = 0;
      void nextTick(() => {
        if (findMatches.value.length) jumpToMatch(0, false);
      });
    }, 100);
  },
);

watch(findMatches, (list) => {
  if (!list.length) {
    findIndex.value = 0;
    return;
  }
  if (findIndex.value >= list.length) findIndex.value = 0;
});

function highlightFor(rowIndex: number, columnIndex: number) {
  return findHighlights.value.get(`${rowIndex}:${columnIndex}`) ?? null;
}

function jumpToMatch(index: number, scroll = true) {
  const list = findMatches.value;
  if (!list.length) return;
  const next = ((index % list.length) + list.length) % list.length;
  findIndex.value = next;
  if (!scroll) return;
  const match = list[next];
  void nextTick(() => scrollToRow(match.rowIndex, { center: true }));
}

function nextMatch() {
  jumpToMatch(findIndex.value + 1);
}

function previousMatch() {
  jumpToMatch(findIndex.value - 1);
}

async function focusSearch() {
  if (!findOpen.value) {
    restoreFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    findOpen.value = true;
    if (findQuery.value.trim()) findTerm.value = findQuery.value;
  }
  await nextTick();
  findBar.value?.focusInput();
}

function closeSearch() {
  window.clearTimeout(findTimer);
  findOpen.value = false;
  findTerm.value = "";
  findIndex.value = 0;
  const target = restoreFocus;
  restoreFocus = null;
  void nextTick(() => target?.focus());
}

function resetSearch() {
  window.clearTimeout(findTimer);
  findOpen.value = false;
  findQuery.value = "";
  findTerm.value = "";
  findIndex.value = 0;
}

watch(
  () => [props.connectionId, props.table, props.database, props.schema],
  () => {
    resetSearch();
    detailOpen.value = false;
    pasteOpen.value = false;
    foreignKeys.value = [];
    foreignKeysError.value = "";
    foreignKeysLoading.value = false;
    frozenIndices.value = [];
    frozenNames.value = [];
    headerMenu.value = null;
  },
);

watch(displayRows, (list) => {
  if (!detailOpen.value) return;
  if (!list.length) {
    detailOpen.value = false;
    return;
  }
  if (detailRowIndex.value >= list.length) {
    detailRowIndex.value = list.length - 1;
  }
});

const pageExportRows = computed(() =>
  displayRows.value
    .filter((row) => row.state !== "delete")
    .map((row) => [...row.values]),
);
const pageVisibleRows = computed(() =>
  pageExportRows.value.map((row) =>
    visibleIndices.value.map((index) => row[index] ?? null),
  ),
);
const selectedExportRows = computed(() =>
  [...selectedRows.value]
    .map((index) => displayRows.value[index])
    .filter((row) => row && row.state !== "delete")
    .map((row) => [...row!.values]),
);

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
    !primaryKeys.value.every((key) =>
      visibleColumns.value.some((column) => column.name === key.name),
    )
  ) {
    return "主键列已隐藏，无法生成 UPDATE";
  }
  if (
    !visibleColumns.value.some(
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
const tableProfileData = computed<ChartData>(() => ({
  columns: columns.value.map((column) => column.name),
  visibleColumns: visibleColumnEntries.value.map((entry) => ({
    name: entry.column.name,
    index: entry.index,
  })),
  rows: rows.value,
  truncated: false,
}));
const tableProfilePage = computed(() => ({
  page: page.value,
  pageCount: pageCount.value,
  total: total.value,
  pending: hasPendingChanges.value,
}));
const pendingChanges = computed(() => buildPendingChanges());
const pendingSql = computed(() => buildPendingSql(pendingChanges.value));
const canUndo = computed(
  () => canEdit.value && undoStack.value.length > 0,
);
const canRedo = computed(
  () => canEdit.value && redoStack.value.length > 0,
);

function pendingSnapshot(): PendingSnapshot {
  return clonePendingSnapshot({
    updates: updates.value,
    deletes: [...pendingDeletes.value],
    inserts: pendingInserts.value.map((draft) => ({
      id: draft.id,
      values: draft.values,
    })),
  });
}

function recordHistory() {
  pushPendingHistory(undoStack.value, pendingSnapshot());
  redoStack.value = [];
}

function applyPendingSnapshot(snapshot: PendingSnapshot) {
  const copy = clonePendingSnapshot(snapshot);
  updates.value = copy.updates;
  pendingDeletes.value = new Set(copy.deletes);
  pendingInserts.value = copy.inserts.map((draft) => ({
    id: draft.id,
    values: draft.values,
  }));
  editing.value = null;
  selectedRows.value = new Set();
  selectionAnchor = null;
  previewOpen.value = false;
}

function undo() {
  if (!canUndo.value) return;
  const target = takePendingHistory(undoStack.value);
  if (!target) return;
  pushPendingHistory(redoStack.value, pendingSnapshot());
  applyPendingSnapshot(target);
  showMessage("已撤销上一步");
}

function redo() {
  if (!canRedo.value) return;
  const target = takePendingHistory(redoStack.value);
  if (!target) return;
  pushPendingHistory(undoStack.value, pendingSnapshot());
  applyPendingSnapshot(target);
  showMessage("已重做");
}

function resetHistory() {
  undoStack.value = [];
  redoStack.value = [];
}

const detailRow = computed(
  () => displayRows.value[detailRowIndex.value] ?? null,
);
const detailEditable = computed(
  () =>
    Boolean(
      canEdit.value &&
        detailRow.value &&
        detailRow.value.state !== "delete",
    ),
);
const detailReadOnlyReason = computed(() => {
  if (props.readOnly) return "只读连接";
  if (!isTable.value) return "视图不支持编辑";
  if (detailRow.value?.state === "delete") {
    return "该行已标记删除，撤销删除后可编辑";
  }
  if (!canEdit.value) return "当前表不可编辑";
  return "";
});
const detailRowLabel = computed(() => {
  if (!detailRow.value) return "";
  return `当前页第 ${detailRowIndex.value + 1} / ${
    displayRows.value.length
  } 行 · 全表第 ${offset.value + detailRowIndex.value + 1} 行`;
});
const detailFields = computed<RowDetailField[]>(() => {
  const row = detailRow.value;
  if (!row) return [];
  return columns.value.map((column, index) => {
    const binary = isBinaryColumn(column);
    let editable = Boolean(canEdit.value && !binary && row.state !== "delete");
    let disabledReason = "";
    if (!canEdit.value) {
      disabledReason = props.readOnly
        ? "只读连接"
        : !isTable.value
          ? "视图不支持编辑"
          : "当前表不可编辑";
      editable = false;
    } else if (binary) {
      disabledReason = "二进制字段不支持编辑";
    } else if (row.state === "delete") {
      disabledReason = "该行已标记删除，撤销删除后可编辑";
    }
    return {
      name: column.name,
      value: row.values[index],
      type: column.dataType,
      nullable: column.nullable,
      primaryKey: column.primaryKey,
      autoIncrement: column.autoIncrement,
      comment: column.comment,
      hidden: hiddenSet.value.has(column.name),
      modified: isCellModified(detailRowIndex.value, index),
      editable,
      disabledReason,
      binary,
    };
  });
});

function openRowDetail(index: number) {
  const row = displayRows.value[index];
  if (!row) return;
  commitEdit();
  detailRowIndex.value = index;
  detailOpen.value = true;
}

function openDetailFromToolbar() {
  const selected = [...selectedRows.value].sort((left, right) => left - right);
  openRowDetail(selected[0] ?? 0);
}

function closeRowDetail() {
  detailOpen.value = false;
}

function moveRowDetail(delta: number) {
  const next = detailRowIndex.value + delta;
  if (next < 0 || next >= displayRows.value.length) return;
  detailRowIndex.value = next;
}

function setDetailValue(index: number, value: unknown) {
  const column = columns.value[index];
  const row = detailRow.value;
  if (!column || !row || row.state === "delete" || !canEdit.value) return;
  if (isBinaryColumn(column)) return;
  if (sameValue(row.values[index], value)) return;
  recordHistory();
  setCellPending(detailRowIndex.value, index, value);
  showMessage("已加入待提交，尚未写入数据库");
}

function applyDetailEdit(index: number, raw: string) {
  const column = columns.value[index];
  if (!column) return;
  setDetailValue(index, parseInput(raw, column));
}

function setDetailNull(index: number) {
  const column = columns.value[index];
  if (!column?.nullable) return;
  setDetailValue(index, null);
}

function viewDetailValue(index: number) {
  valueViewer.value = { rowIndex: detailRowIndex.value, colIndex: index };
}

async function copyDetailValue(index: number) {
  const field = detailFields.value[index];
  if (!field) return;
  try {
    await writeClipboard(formatCell(field.value));
    showMessage(`已复制 ${field.name}`);
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

async function copyDetailRowJson() {
  const row = detailRow.value;
  if (!row) return;
  try {
    await writeClipboard(
      buildRowJson(
        columns.value.map((column) => column.name),
        row.values.map((value) => value ?? null),
      ),
    );
    showMessage("已复制整行 JSON（全部字段）");
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

function sortDirectionFor(column: string): "asc" | "desc" | null {
  return sorts.value.find((sort) => sort.column === column)?.direction ?? null;
}
const insertColumns = computed(() =>
  columns.value.filter((column) => !column.autoIncrement),
);

const formatCell = formatGridCell;

function tableFormatScopeKey(): string {
  return columnFormatScopeKey({
    driver: driverKind.value,
    connectionId: props.connectionId,
    database: props.database,
    schema: props.schema,
    table: props.table,
    tableKind: props.tableKind,
  });
}

const formatRuleMap = computed(
  () =>
    new Map(
      formatEntries.value.map((entry) => [entry.columnKey, entry.rule] as const),
    ),
);

function columnFormatRule(index: number): ColumnFormatRule | null {
  const key = profileKeys.value[index];
  if (!key) return null;
  return formatRuleMap.value.get(key) ?? null;
}

function columnFormatSummary(index: number): string {
  const rule = columnFormatRule(index);
  return rule ? formatRuleSummary(rule) : "";
}

function displayCell(value: unknown, index: number): { text: string; warning: string | null } {
  const rule = columnFormatRule(index);
  if (!rule) return { text: formatCell(value), warning: null };
  return formatDisplayValue(value, rule);
}

function displayCellText(value: unknown, index: number): string {
  return displayCell(value, index).text;
}

function displayCellTitle(value: unknown, index: number): string {
  const formatted = displayCell(value, index);
  return formatted.warning ? `${formatted.text} · ${formatted.warning}` : formatted.text;
}

function formatWarning(value: unknown, index: number): string | null {
  return displayCell(value, index).warning;
}

function persistTableFormats() {
  saveTableFormats(tableFormatScopeKey(), formatEntries.value);
}

function openColumnFormat(index: number) {
  headerMenu.value = null;
  formatDialog.value = { colIndex: index };
}

function clearColumnFormatEntry(index: number) {
  const key = profileKeys.value[index];
  if (!key) return;
  formatEntries.value = formatEntries.value.filter((entry) => entry.columnKey !== key);
  persistTableFormats();
}

function applyColumnFormat(rule: ColumnFormatRule | null) {
  const dialog = formatDialog.value;
  if (!dialog) return;
  const key = profileKeys.value[dialog.colIndex];
  if (!key) return;
  formatEntries.value = formatEntries.value.filter((entry) => entry.columnKey !== key);
  if (rule) formatEntries.value.push({ columnKey: key, rule });
  persistTableFormats();
  formatDialog.value = null;
}

const formatSamples = computed(() => {
  const dialog = formatDialog.value;
  if (!dialog) return [];
  return displayRows.value
    .map((row) => row.values[dialog.colIndex])
    .filter((value) => value !== null && value !== undefined)
    .slice(0, 10);
});

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

function columnWidthAt(index: number): number {
  const column = columns.value[index];
  if (!column) return 140;
  const key = profileKeys.value[index] ?? column.name;
  const stored = columnWidths[key] ?? columnWidths[column.name];
  if (stored) return stored;
  return defaultColumnWidth(column);
}

const tableOrderScope = computed(() =>
  columnOrderScopeKey(
    driverKind.value,
    props.connectionId,
    props.database,
    props.schema,
    props.table,
    props.tableKind ?? "table",
  ),
);
const frozenScope = computed(() =>
  frozenScopeKey(
    driverKind.value,
    props.connectionId,
    props.database,
    props.schema,
    props.table,
  ),
);
const visibleFrozenIndices = computed(() =>
  visibleColumnEntries.value
    .filter((entry) => frozenIndices.value.includes(entry.index))
    .map((entry) => entry.index),
);
const frozenColumns = computed(() =>
  columns.value.filter((_, index) => visibleFrozenIndices.value.includes(index)),
);
const lastFrozenIndex = computed(
  () => visibleFrozenIndices.value[visibleFrozenIndices.value.length - 1] ?? null,
);
const frozenLeft = computed(() =>
  frozenOffsets(
    columns.value.map((column, index) =>
      hiddenSet.value.has(column.name) ? 0 : columnWidthAt(index),
    ),
    visibleFrozenIndices.value,
    76,
  ),
);

function applyFrozenNames(names: string[]) {
  const normalized = normalizeFrozenNames(
    names,
    columns.value.map((column) => column.name),
  );
  frozenNames.value = normalized;
  frozenIndices.value = normalized
    .map((name) => columns.value.findIndex((column) => column.name === name))
    .filter((index) => index >= 0);
}

function restoreFrozenColumns() {
  if (props.persistFrozen === false) {
    // Split panes keep their session-local frozen columns across paging.
    applyFrozenNames(frozenNames.value);
    return;
  }
  applyFrozenNames(loadFrozenNames(frozenScope.value));
}

function persistFrozenColumns() {
  if (props.persistFrozen === false) return;
  saveFrozenNames(frozenScope.value, frozenNames.value);
}

function freezeColumn(index: number) {
  if (!canFreezeColumn(index)) {
    showMessage(
      visibleColumns.value.length <= 1
        ? "至少保留一列可滚动"
        : "当前无需横向滚动",
    );
    return;
  }
  const position = visibleIndices.value.indexOf(index);
  if (position < 0) return;
  frozenNames.value = visibleIndices.value
    .slice(0, position + 1)
    .map((columnIndex) => columns.value[columnIndex]?.name ?? "")
    .filter(Boolean);
  frozenIndices.value = frozenNames.value
    .map((name) => columns.value.findIndex((column) => column.name === name))
    .filter((columnIndex) => columnIndex >= 0);
  persistFrozenColumns();
  closeCellMenu();
  headerMenu.value = null;
}

function unfreezeColumns() {
  frozenIndices.value = [];
  frozenNames.value = [];
  persistFrozenColumns();
  closeCellMenu();
  headerMenu.value = null;
}

function canFreezeColumn(index: number): boolean {
  if (visibleColumns.value.length <= 1) return false;
  const position = visibleIndices.value.indexOf(index);
  if (position < 0 || position >= visibleColumns.value.length - 1) return false;
  const element = scrollRef.value;
  if (element && element.scrollWidth <= element.clientWidth + 1) {
    return false;
  }
  return true;
}

function canHideColumn(index: number): boolean {
  return (
    visibleIndices.value.includes(index) && visibleColumns.value.length > 1
  );
}

function hideColumn(index: number) {
  const column = columns.value[index];
  if (!column) return;
  if (!canHideColumn(index)) {
    showMessage("至少保留一列可见");
    return;
  }
  commitEdit();
  hiddenNames.value = [...hiddenNames.value, column.name];
  headerMenu.value = null;
  closeCellMenu();
}

function showAllColumns() {
  hiddenNames.value = [];
  autoHideMode.value = "off";
  autoExceptions.clear();
  headerMenu.value = null;
  closeCellMenu();
}

function toggleVisibilityColumn(key: string, visible: boolean) {
  const column = visibilityColumns.value.find((item) => item.key === key);
  if (!column) return;
  if (visible) {
    hiddenNames.value = hiddenNames.value.filter((name) => name !== column.name);
    autoExceptions.add(column.key);
  } else if (autoHiddenIndexSet.value.has(column.index)) {
    autoExceptions.delete(column.key);
  } else {
    autoExceptions.delete(column.key);
    if (!hiddenNames.value.includes(column.name)) {
      hiddenNames.value = [...hiddenNames.value, column.name];
    }
  }
}

function setAutoHideMode(mode: AutoHideMode) {
  autoHideMode.value = mode;
  autoExceptions.clear();
}

function normalizeHiddenColumns(): boolean {
  const names = columns.value.map((column) => column.name);
  const kept = normalizeFrozenNames(hiddenNames.value, names);
  const changed = kept.length !== hiddenNames.value.length;
  if (names.length && kept.length >= names.length) {
    // Keep the first column visible even if a stored view hid everything.
    hiddenNames.value = kept.filter((name) => name !== names[0]);
    return true;
  }
  hiddenNames.value = kept;
  return changed;
}

function frozenMenuLabel(index: number): string {
  const position = visibleIndices.value.indexOf(index);
  const names =
    position < 0
      ? []
      : visibleIndices.value
          .slice(0, position + 1)
          .map((columnIndex) => columns.value[columnIndex]?.name ?? "");
  const preview =
    names.length <= 3
      ? names.join("、")
      : `${names.slice(0, 2).join("、")}… 等 ${names.length} 列`;
  return `冻结到此列（${preview}）`;
}

function flushColumnOrderSave() {
  window.clearTimeout(columnOrderSaveTimer);
  const pending = pendingColumnOrderSave;
  pendingColumnOrderSave = null;
  if (!pending) return;
  if (!saveColumnOrder(pending.scope, pending.order)) {
    showMessage("列顺序未保存（本地存储不可用或已满）");
  }
}

function scheduleColumnOrderSave() {
  if (props.persistFrozen === false) return;
  pendingColumnOrderSave = {
    scope: tableOrderScope.value,
    order: normalizeColumnOrder(columnOrder.value, profileKeys.value),
  };
  window.clearTimeout(columnOrderSaveTimer);
  columnOrderSaveTimer = window.setTimeout(flushColumnOrderSave, 300);
}

function tableColumnBlock(
  index: number,
): Array<{ column: ColumnInfo; index: number }> {
  const frozen = frozenIndices.value.includes(index);
  return visibleColumnEntries.value.filter(
    (entry) => frozenIndices.value.includes(entry.index) === frozen,
  );
}

function reorderTableColumn(from: number, to: number, after: boolean) {
  const fromKey = profileKeys.value[from];
  const toKey = profileKeys.value[to];
  if (!fromKey || !toKey || from === to) return;
  const frozenFrom = frozenIndices.value.includes(from);
  const frozenTo = frozenIndices.value.includes(to);
  if (frozenFrom !== frozenTo) {
    showMessage("请先取消冻结或先冻结该列，再跨冻结块移动");
    return;
  }
  const order = normalizeColumnOrder(columnOrder.value, profileKeys.value);
  columnOrder.value = moveColumnKey(order, fromKey, toKey, after);
  frozenNames.value = visibleColumnEntries.value
    .filter((entry) => frozenIndices.value.includes(entry.index))
    .map((entry) => entry.column.name);
  scheduleColumnOrderSave();
}

function cleanupColumnDrag() {
  columnDrag = null;
  columnDragState.value = null;
  window.removeEventListener("mousemove", moveColumnDrag);
  window.removeEventListener("mouseup", finishColumnDrag);
  window.removeEventListener("keydown", cancelColumnDragOnEscape, true);
}

function startColumnDrag(event: MouseEvent, index: number) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest(".col-resizer")) return;
  commitEdit();
  columnDrag = {
    index,
    startX: event.clientX,
    startY: event.clientY,
    frozen: frozenIndices.value.includes(index),
    moved: false,
    target: null,
    after: false,
  };
  window.addEventListener("mousemove", moveColumnDrag);
  window.addEventListener("mouseup", finishColumnDrag);
  window.addEventListener("keydown", cancelColumnDragOnEscape, true);
}

function moveColumnDrag(event: MouseEvent) {
  if (!columnDrag) return;
  if (
    !columnDrag.moved &&
    Math.abs(event.clientX - columnDrag.startX) < 4 &&
    Math.abs(event.clientY - columnDrag.startY) < 4
  ) {
    return;
  }
  columnDrag.moved = true;
  event.preventDefault();
  const element = document.elementFromPoint(event.clientX, event.clientY);
  const header = element?.closest<HTMLElement>("th[data-col-index]");
  if (!header) {
    columnDragState.value = {
      index: columnDrag.index,
      target: null,
      after: false,
    };
    return;
  }
  const target = Number(header.dataset.colIndex);
  if (!Number.isInteger(target) || target === columnDrag.index) {
    columnDragState.value = {
      index: columnDrag.index,
      target: null,
      after: false,
    };
    return;
  }
  const rect = header.getBoundingClientRect();
  columnDragState.value = {
    index: columnDrag.index,
    target,
    after: event.clientX > rect.left + rect.width / 2,
  };
}

function finishColumnDrag() {
  const drag = columnDrag;
  const state = columnDragState.value;
  cleanupColumnDrag();
  if (!drag) return;
  if (drag.moved) {
    suppressHeaderClick = true;
    window.setTimeout(() => {
      suppressHeaderClick = false;
    }, 0);
  }
  if (!drag.moved || !state || state.target === null) return;
  reorderTableColumn(drag.index, state.target, state.after);
}

function cancelColumnDrag() {
  if (!columnDrag) return;
  cleanupColumnDrag();
  suppressHeaderClick = true;
  window.setTimeout(() => {
    suppressHeaderClick = false;
  }, 0);
}

function cancelColumnDragOnEscape(event: KeyboardEvent) {
  if (event.key !== "Escape" || !columnDrag) return;
  event.preventDefault();
  event.stopPropagation();
  cancelColumnDrag();
}

function handleHeaderClick(name: string, additive: boolean) {
  if (suppressHeaderClick) {
    suppressHeaderClick = false;
    return;
  }
  toggleSort(name, additive);
}

function canMoveHeader(
  direction: "left" | "right" | "first" | "last",
): boolean {
  const menu = headerMenu.value;
  if (!menu) return false;
  const block = tableColumnBlock(menu.colIndex);
  const position = block.findIndex((entry) => entry.index === menu.colIndex);
  if (position < 0 || block.length < 2) return false;
  if (direction === "left" || direction === "first") return position > 0;
  return position < block.length - 1;
}

function moveHeaderColumn(
  direction: "left" | "right" | "first" | "last",
) {
  const menu = headerMenu.value;
  if (!menu || !canMoveHeader(direction)) return;
  const block = tableColumnBlock(menu.colIndex);
  const position = block.findIndex((entry) => entry.index === menu.colIndex);
  let target: { index: number } | undefined;
  let after = false;
  if (direction === "left") {
    target = block[position - 1];
  } else if (direction === "right") {
    target = block[position + 1];
    after = true;
  } else if (direction === "first") {
    target = block[0];
  } else {
    target = block[block.length - 1];
    after = true;
  }
  headerMenu.value = null;
  if (target) reorderTableColumn(menu.colIndex, target.index, after);
}

function openHeaderMenu(event: MouseEvent, colIndex: number) {
  event.preventDefault();
  event.stopPropagation();
  closeCellMenu();
  headerMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 230)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 380)),
    colIndex,
  };
}

function resetColumnWidth(index: number) {
  const column = columns.value[index];
  if (!column) return;
  delete columnWidths[profileKeys.value[index] ?? column.name];
  delete columnWidths[column.name];
}

function startResize(event: MouseEvent, index: number) {
  event.preventDefault();
  event.stopPropagation();
  const key = profileKeys.value[index] ?? String(index);
  resizeState = {
    key,
    startX: event.clientX,
    startWidth: columnWidthAt(index),
  };
  window.addEventListener("mousemove", moveResize);
  window.addEventListener("mouseup", stopResize);
}

function moveResize(event: MouseEvent) {
  if (!resizeState) return;
  columnWidths[resizeState.key] = Math.max(
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
  headerMenu.value = null;
}

function onGlobalKey(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (findOpen.value) {
      closeSearch();
      return;
    }
    if (detailOpen.value) {
      detailOpen.value = false;
      return;
    }
    closeCellMenu();
    if (insertOpen.value) insertOpen.value = false;
    if (importRequest.value) importRequest.value = null;
    return;
  }
  const modifier = event.metaKey || event.ctrlKey;
  if (modifier && !event.altKey) {
    const key = event.key.toLowerCase();
    const wantsUndo = key === "z" && !event.shiftKey;
    const wantsRedo =
      (key === "z" && event.shiftKey) || (key === "y" && !event.shiftKey);
    if (
      (wantsUndo || wantsRedo) &&
      (wantsUndo ? canUndo.value : canRedo.value)
    ) {
      const root = rootRef.value;
      const target = event.target;
      if (!root || !(target instanceof Node) || !root.contains(target)) {
        return;
      }
      const element = target instanceof Element ? target : null;
      if (element?.closest('.modal-backdrop, [role="dialog"]')) return;
      if (
        element?.closest(
          'input, textarea, select, [contenteditable="true"], .cm-editor',
        )
      ) {
        return;
      }
      event.preventDefault();
      if (wantsUndo) undo();
      else redo();
      return;
    }
    if (event.shiftKey && key === "v") {
      const root = rootRef.value;
      const target = event.target;
      if (!root || !(target instanceof Node) || !root.contains(target)) {
        return;
      }
      const element = target instanceof Element ? target : null;
      if (element?.closest('.modal-backdrop, [role="dialog"]')) return;
      if (
        element?.closest(
          'input, textarea, select, [contenteditable="true"], .cm-editor',
        )
      ) {
        return;
      }
      event.preventDefault();
      openPasteDialog();
      return;
    }
  }
  if (!modifier || event.key.toLowerCase() !== "f") {
    return;
  }
  const root = rootRef.value;
  const target = event.target;
  if (!root || !(target instanceof Node) || !root.contains(target)) return;
  const element = target instanceof Element ? target : null;
  if (element?.closest(".grid-findbar")) {
    event.preventDefault();
    void focusSearch();
    return;
  }
  if (element?.closest('.modal-backdrop, [role="dialog"]')) return;
  if (
    element?.closest(
      'input, textarea, select, [contenteditable="true"], .cm-editor',
    )
  ) {
    return;
  }
  event.preventDefault();
  void focusSearch();
}

function menuValue(): unknown {
  if (!cellMenu.value) return null;
  return displayRows.value[cellMenu.value.rowIndex]?.values[
    cellMenu.value.colIndex
  ];
}

function openDetailFromMenu() {
  const menu = cellMenu.value;
  if (!menu) return;
  const index = menu.rowIndex;
  closeCellMenu();
  openRowDetail(index);
}

async function ensureForeignKeys(force = false) {
  if (!isTable.value || foreignKeysLoading.value) return;
  const cached = force
    ? null
    : peekScopeForeignKeys(props.connectionId, props.database, props.schema);
  if (cached) {
    foreignKeys.value = cached;
    return;
  }
  foreignKeysLoading.value = true;
  foreignKeysError.value = "";
  try {
    foreignKeys.value = await loadScopeForeignKeys(
      props.connectionId,
      props.database,
      props.schema,
      force,
    );
  } catch (reason) {
    foreignKeysError.value = String(reason);
  } finally {
    foreignKeysLoading.value = false;
  }
}

function tableAvailable(name: string): boolean {
  if (!props.tables?.length) return true;
  return props.tables.some(
    (table) =>
      table.name === name &&
      (table.kind === "table" || table.kind === "view" || table.kind === "matview"),
  );
}

function stableLinkValues(values: unknown[]): boolean {
  return values.every(
    (value) =>
      value !== null &&
      value !== undefined &&
      typeof value !== "object",
  );
}

function linkFilter(columns: string[], values: unknown[]): string {
  return columns
    .map(
      (column, index) =>
        `${quoteIdentifier(column, driverKind.value)} = ${sqlLiteral(
          values[index],
          driverKind.value,
        )}`,
    )
    .join(" AND ");
}

interface OutboundLink {
  key: string;
  foreignKey: ScopeForeignKey;
  values: unknown[];
  enabled: boolean;
  reason: string;
}

interface InboundLink {
  key: string;
  foreignKey: ScopeForeignKey;
  values: unknown[];
  enabled: boolean;
  reason: string;
}

const outgoingLinks = computed<OutboundLink[]>(() => {
  const menu = cellMenu.value;
  if (!menu || !isTable.value) return [];
  const row = displayRows.value[menu.rowIndex];
  const column = columns.value[menu.colIndex];
  if (!row || !column) return [];
  return foreignKeys.value
    .filter(
      (foreignKey) =>
        foreignKey.table === props.table &&
        foreignKey.columns.includes(column.name),
    )
    .map((foreignKey) => {
      const values = foreignKey.columns.map(
        (name) => row.values[columnIndex(name)],
      );
      const enabled =
        stableLinkValues(values) && tableAvailable(foreignKey.referencedTable);
      const reason = !stableLinkValues(values)
        ? "外键列包含 NULL 或非标量值，无法定位引用行"
        : !tableAvailable(foreignKey.referencedTable)
          ? "目标表不在当前库 / 模式"
          : "";
      return {
        key: `${foreignKey.table}:${foreignKey.name}`,
        foreignKey,
        values,
        enabled,
        reason,
      };
    });
});

const incomingLinks = computed<InboundLink[]>(() => {
  const menu = cellMenu.value;
  if (!menu || !isTable.value) return [];
  const row = displayRows.value[menu.rowIndex];
  if (!row) return [];
  return foreignKeys.value
    .filter((foreignKey) => foreignKey.referencedTable === props.table)
    .map((foreignKey) => {
      const values = foreignKey.referencedColumns.map(
        (name) => row.values[columnIndex(name)],
      );
      const enabled =
        stableLinkValues(values) && tableAvailable(foreignKey.table);
      const reason = !stableLinkValues(values)
        ? "当前行的被引用列包含 NULL 或非标量值"
        : !tableAvailable(foreignKey.table)
          ? "引用表不在当前库 / 模式"
          : "";
      return {
        key: `${foreignKey.table}:${foreignKey.name}`,
        foreignKey,
        values,
        enabled,
        reason,
      };
    });
});

function openOutboundLink(link: OutboundLink) {
  if (!link.enabled) {
    showMessage(link.reason);
    return;
  }
  emit("open-linked-table", {
    table: link.foreignKey.referencedTable,
    filter: linkFilter(
      link.foreignKey.referencedColumns,
      link.values,
    ),
    label: `${props.table}.${link.foreignKey.columns.join(", ")}`,
  });
  closeCellMenu();
}

function openInboundLink(link: InboundLink) {
  if (!link.enabled) {
    showMessage(link.reason);
    return;
  }
  emit("open-linked-table", {
    table: link.foreignKey.table,
    filter: linkFilter(link.foreignKey.columns, link.values),
    label: `${link.foreignKey.table}.${link.foreignKey.columns.join(", ")}`,
  });
  closeCellMenu();
}

function openForeignKeysFromMenu() {
  closeCellMenu();
  void ensureForeignKeys(true);
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

async function copyDisplayCellValue() {
  if (!cellMenu.value) return;
  const value = menuValue();
  const colIndex = cellMenu.value.colIndex;
  try {
    await writeClipboard(displayCellText(value, colIndex));
    showMessage("已复制显示值（原始值仍保留在数据库和导出中）");
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
    await writeClipboard(
      visibleIndices.value.map((index) => formatCell(row[index])).join("\t"),
    );
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

  const projected = visibleIndices.value.map((index) => row[index]);
  let text: string;
  let label: string;
  if (kind === "insert") {
    text = buildRowInsert(
      visibleColumns.value,
      projected,
      driverKind.value,
      tableIdentifier(),
    );
    label = "INSERT";
  } else if (kind === "update") {
    if (updateCopyReason.value) return;
    const sql = buildRowUpdate(
      visibleColumns.value,
      projected,
      driverKind.value,
      tableIdentifier(),
    );
    if (!sql) return;
    text = sql;
    label = "UPDATE";
  } else {
    text = buildRowJson(visibleColumnNames.value, projected);
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
  cleanupColumnDrag();
  flushColumnOrderSave();
  window.clearTimeout(messageTimer);
  window.clearTimeout(findTimer);
  window.removeEventListener("click", closeCellMenu);
  window.removeEventListener("keydown", onGlobalKey);
  stopResize();
});

async function load(nextOffset = offset.value): Promise<boolean> {
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
    const pruned = pruneFormats(
      formatEntries.value,
      new Set(profileKeys.value),
    );
    if (pruned.length !== formatEntries.value.length) {
      formatEntries.value = pruned;
      persistTableFormats();
    }
    if (pendingColumnView) {
      const view = pendingColumnView;
      pendingColumnView = null;
      applyProfileColumns(view);
    } else {
      restoreFrozenColumns();
      normalizeHiddenColumns();
      columnOrder.value = normalizeColumnOrder(
        columnOrder.value,
        profileKeys.value,
      );
    }
    rows.value = pageData.rows;
    total.value = pageData.total;
    offset.value = pageData.offset;
    pageInput.value = String(
      Math.floor(pageData.offset / limit.value) + 1,
    );
    selectedRows.value = new Set();
    selectionAnchor = null;
    detailOpen.value = false;
    resetHistory();
    void ensureForeignKeys();
    scrollToTop();
    return true;
  } catch (reason) {
    rows.value = [];
    total.value = 0;
    error.value = String(reason);
    emit("connection-error", error.value);
    return false;
  } finally {
    loading.value = false;
  }
}

watch(
  [
    () => props.connectionId,
    () => props.table,
    () => props.database,
    () => props.schema,
    () => props.tableKind,
  ],
  () => {
    // Defer past setup: initializeScope reads computeds declared later.
    void nextTick(() => initializeScope());
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

const viewScope = computed(() =>
  tableViewScopeKey(
    driverKind.value,
    props.connectionId,
    props.database,
    props.schema,
    props.table,
    props.tableKind ?? "table",
  ),
);

function keyByColumnName(): Map<string, string> {
  const map = new Map<string, string>();
  profileKeys.value.forEach((key, index) => {
    const name = columns.value[index]?.name;
    if (name && !map.has(name)) map.set(name, key);
  });
  return map;
}

function currentViewConfig() {
  const widths: Record<string, number> = {};
  profileKeys.value.forEach((key, index) => {
    const value =
      columnWidths[key] ?? columnWidths[columns.value[index]?.name ?? ""];
    if (value) widths[key] = Math.round(value);
  });
  const keyByName = keyByColumnName();
  return {
    filter: appliedFilter.value.trim(),
    sorts: sorts.value.map((sort) => ({ ...sort })),
    limit: limit.value,
    widths,
    hidden: hiddenNames.value
      .map((name) => keyByName.get(name))
      .filter((key): key is string => Boolean(key)),
    frozen: frozenNames.value
      .map((name) => keyByName.get(name))
      .filter((key): key is string => Boolean(key)),
    order: normalizeColumnOrder(columnOrder.value, profileKeys.value),
  };
}

function currentViewSignature(): string {
  return tableViewSignature(currentViewConfig());
}

const viewDirty = computed(
  () =>
    Boolean(activeViewId.value) &&
    Boolean(viewBaseline.value) &&
    currentViewSignature() !== viewBaseline.value,
);

function readViewState() {
  const state = loadTableViewState(viewScope.value);
  viewProfiles.value = state.profiles;
  defaultViewId.value = state.defaultId;
  activeViewId.value = null;
  viewBaseline.value = null;
}

function persistViewState(): boolean {
  const saved = saveTableViewState(viewScope.value, {
    profiles: viewProfiles.value,
    activeId: activeViewId.value,
    defaultId: defaultViewId.value,
  });
  if (!saved) {
    showMessage("视图配置保存失败：本地存储不可用或已满");
  }
  return saved;
}

function sortViewProfiles() {
  viewProfiles.value = [...viewProfiles.value].sort(
    (left, right) => right.updatedAt - left.updatedAt,
  );
}

function applyProfileSettings(
  profile: TableViewProfile,
  validateSorts: boolean,
): number {
  const names = new Set(columns.value.map((column) => column.name));
  const before = profile.sorts.length;
  sorts.value = validateSorts
    ? profile.sorts.filter((sort) => names.has(sort.column))
    : profile.sorts.map((sort) => ({ ...sort }));
  const missingSorts = before - sorts.value.length;
  appliedFilter.value = profile.filter;
  filterDraft.value = profile.filter;
  filterOpen.value = Boolean(profile.filter);
  limit.value = TABLE_VIEW_PAGE_SIZES.includes(profile.limit)
    ? profile.limit
    : 100;
  pageInput.value = "1";
  savePageSize(limit.value);
  return missingSorts;
}

function applyProfileColumns(profile: TableViewProfile): number {
  const keyIndices = new Map(
    profileKeys.value.map((key, index) => [key, index]),
  );
  let dropped = 0;
  for (const key of Object.keys(columnWidths)) {
    delete columnWidths[key];
  }
  for (const [key, width] of Object.entries(profile.widths)) {
    const index = keyIndices.get(key);
    if (index === undefined) {
      dropped += 1;
      continue;
    }
    columnWidths[key] = Math.max(
      60,
      Math.min(600, Math.round(width)),
    );
  }
  const hidden: string[] = [];
  for (const key of profile.hidden) {
    const index = keyIndices.get(key);
    if (index === undefined) {
      dropped += 1;
      continue;
    }
    const name = columns.value[index]?.name;
    if (name) hidden.push(name);
  }
  hiddenNames.value = hidden;
  normalizeHiddenColumns();
  const frozen: string[] = [];
  for (const key of profile.frozen) {
    const index = keyIndices.get(key);
    if (index === undefined) {
      dropped += 1;
      continue;
    }
    const name = columns.value[index]?.name;
    if (name) frozen.push(name);
  }
  applyFrozenNames(frozen);
  persistFrozenColumns();
  columnOrder.value = normalizeColumnOrder(
    profile.order?.length ? profile.order : columnOrder.value,
    profileKeys.value,
  );
  return dropped;
}

interface ViewSnapshot {
  filter: string;
  sorts: TableSort[];
  limit: number;
  widths: Record<string, number>;
  hidden: string[];
  frozen: string[];
  order: string[];
  offset: number;
  activeId: string | null;
  baseline: string | null;
}

function captureViewSnapshot(): ViewSnapshot {
  return {
    filter: appliedFilter.value,
    sorts: sorts.value.map((sort) => ({ ...sort })),
    limit: limit.value,
    widths: { ...columnWidths },
    hidden: [...hiddenNames.value],
    frozen: [...frozenNames.value],
    order: [...columnOrder.value],
    offset: offset.value,
    activeId: activeViewId.value,
    baseline: viewBaseline.value,
  };
}

function restoreViewSnapshot(snapshot: ViewSnapshot) {
  appliedFilter.value = snapshot.filter;
  filterDraft.value = snapshot.filter;
  filterOpen.value = Boolean(snapshot.filter);
  sorts.value = snapshot.sorts.map((sort) => ({ ...sort }));
  limit.value = snapshot.limit;
  savePageSize(snapshot.limit);
  for (const key of Object.keys(columnWidths)) {
    delete columnWidths[key];
  }
  Object.assign(columnWidths, snapshot.widths);
  hiddenNames.value = [...snapshot.hidden];
  columnOrder.value = [...snapshot.order];
  applyFrozenNames(snapshot.frozen);
  persistFrozenColumns();
  activeViewId.value = snapshot.activeId;
  viewBaseline.value = snapshot.baseline;
  pendingColumnView = null;
}

function applyView(id: string) {
  const profile = viewProfiles.value.find((item) => item.id === id);
  if (!profile || !columns.value.length) return;
  autoHideMode.value = "off";
  autoExceptions.clear();
  ensureClean("应用表视图", async () => {
    const snapshot = captureViewSnapshot();
    const missingSorts = applyProfileSettings(profile, true);
    pendingColumnView = profile;
    const ok = await load(0);
    if (!ok) {
      const failure = error.value || "读取失败";
      restoreViewSnapshot(snapshot);
      await load(snapshot.offset);
      showMessage(`应用视图失败：${failure}；已恢复原状态`);
      return;
    }
    activeViewId.value = profile.id;
    viewBaseline.value = currentViewSignature();
    const notes: string[] = [];
    if (missingSorts) notes.push(`忽略 ${missingSorts} 个失效排序`);
    showMessage(
      `已应用视图「${profile.name}」${notes.length ? `（${notes.join("，")}）` : ""}`,
    );
  });
}

function saveView(name: string, makeDefault: boolean) {
  if (viewProfiles.value.length >= MAX_PROFILES_PER_TABLE) {
    showMessage(`每张表最多 ${MAX_PROFILES_PER_TABLE} 个视图`);
    return;
  }
  const profile = createTableViewProfile(name, currentViewConfig());
  viewProfiles.value = [profile, ...viewProfiles.value];
  sortViewProfiles();
  activeViewId.value = profile.id;
  if (makeDefault) defaultViewId.value = profile.id;
  viewBaseline.value = currentViewSignature();
  if (persistViewState()) {
    showMessage(`已保存视图「${profile.name}」`);
  }
}

function updateView() {
  const id = activeViewId.value;
  if (!id) return;
  const current = viewProfiles.value.find((profile) => profile.id === id);
  if (!current) return;
  const config = currentViewConfig();
  viewProfiles.value = viewProfiles.value.map((profile) =>
    profile.id === id
      ? {
          ...profile,
          ...config,
          name: profile.name,
          updatedAt: Date.now(),
        }
      : profile,
  );
  sortViewProfiles();
  viewBaseline.value = currentViewSignature();
  if (persistViewState()) {
    showMessage(`已更新视图「${current.name}」`);
  }
}

function renameView(id: string, name: string) {
  viewProfiles.value = viewProfiles.value.map((profile) =>
    profile.id === id
      ? { ...profile, name: name.trim(), updatedAt: Date.now() }
      : profile,
  );
  sortViewProfiles();
  persistViewState();
}

function deleteView(id: string) {
  const removed = viewProfiles.value.find((profile) => profile.id === id);
  viewProfiles.value = viewProfiles.value.filter(
    (profile) => profile.id !== id,
  );
  if (activeViewId.value === id) {
    activeViewId.value = null;
    viewBaseline.value = null;
  }
  if (defaultViewId.value === id) defaultViewId.value = null;
  if (persistViewState() && removed) {
    showMessage(`已删除视图「${removed.name}」`);
  }
}

function setDefaultView(id: string | null) {
  defaultViewId.value = id;
  persistViewState();
  const profile = viewProfiles.value.find((item) => item.id === id);
  showMessage(
    profile
      ? `已将「${profile.name}」设为默认视图`
      : "已取消默认视图",
  );
}

async function initializeScope() {
  flushColumnOrderSave();
  cleanupColumnDrag();
  clearMessage();
  formatEntries.value = loadTableFormats(tableFormatScopeKey());
  formatDialog.value = null;
  autoHideMode.value = "off";
  autoExceptions.clear();
  columnPanelOpen.value = false;
  sorts.value = [];
  const initialFilter = props.initialFilter?.trim() ?? "";
  filterOpen.value = Boolean(initialFilter);
  filterDraft.value = initialFilter;
  appliedFilter.value = initialFilter;
  for (const key of Object.keys(columnWidths)) {
    delete columnWidths[key];
  }
  hiddenNames.value = [];
  columnOrder.value = loadColumnOrder(tableOrderScope.value);
  cellMenu.value = null;
  headerMenu.value = null;
  columns.value = [];
  rows.value = [];
  total.value = 0;
  clearPendingState();
  pendingColumnView = null;
  readViewState();
  const saved = readSavedPageSize();
  limit.value = saved;
  pageInput.value = "1";

  const defaultProfile = !initialFilter
    ? viewProfiles.value.find((profile) => profile.id === defaultViewId.value)
    : null;
  if (!defaultProfile) {
    await load(0);
    return;
  }

  applyProfileSettings(defaultProfile, false);
  pendingColumnView = defaultProfile;
  activeViewId.value = defaultProfile.id;
  const ok = await load(0);
  if (ok) {
    viewBaseline.value = currentViewSignature();
    return;
  }

  const failure = error.value || "读取失败";
  sorts.value = [];
  appliedFilter.value = initialFilter;
  filterDraft.value = initialFilter;
  filterOpen.value = false;
  limit.value = saved;
  savePageSize(saved);
  hiddenNames.value = [];
  columnOrder.value = loadColumnOrder(tableOrderScope.value);
  for (const key of Object.keys(columnWidths)) {
    delete columnWidths[key];
  }
  applyFrozenNames([]);
  persistFrozenColumns();
  activeViewId.value = null;
  viewBaseline.value = null;
  pendingColumnView = null;
  await load(0);
  showMessage(`默认视图应用失败：${failure}；已恢复默认条件`);
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
  recordHistory();
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
    recordHistory();
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
  recordHistory();
  pendingDeletes.value = new Set([
    ...pendingDeletes.value,
    row.origin,
  ]);
  // Keep updates for this row: a delete can be undone with restoreRow(), and
  // buildPendingChanges() already skips updates for deleted rows.
  editing.value = null;
  showMessage("已标记删除，提交后生效");
}

function restoreRow(rowIndex: number) {
  const row = displayRows.value[rowIndex];
  if (!row || row.origin === null) return;
  if (!pendingDeletes.value.has(row.origin)) return;
  recordHistory();
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
  let changed = false;
  for (const index of selectedRows.value) {
    const row = displayRows.value[index];
    if (!row || row.state === "delete") continue;
    changed = true;
    if (row.origin === null) {
      if (row.insertId !== undefined) insertsToRemove.add(row.insertId);
    } else {
      deletes.add(row.origin);
    }
  }
  if (!changed) return;
  recordHistory();
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
  recordHistory();
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
  resetHistory();
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
  const hasTarget = [...selectedRows.value].some(
    (rowIndex) =>
      displayRows.value[rowIndex] &&
      rowState(rowIndex) !== "delete",
  );
  if (!hasTarget) {
    batchSetOpen.value = false;
    return;
  }
  recordHistory();
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
  recordHistory();
  pendingInserts.value = [...pendingInserts.value, ...clones];
  showMessage(`已克隆 ${clones.length} 行到待提交新增`);
}

async function copySelectedRows(as: "tsv" | "json") {
  const selected = [...selectedRows.value]
    .map((index) => displayRows.value[index])
    .filter(
      (row): row is DisplayRow => Boolean(row) && row.state !== "delete",
    );
  if (!selected.length) return;
  try {
    if (as === "tsv") {
      const header = visibleColumnNames.value.join("\t");
      const lines = selected.map((row) =>
        visibleIndices.value
          .map((index) => formatCell(row.values[index]))
          .join("\t"),
      );
      await writeClipboard([header, ...lines].join("\n"));
      showMessage(`已复制 ${selected.length} 行为 TSV`);
    } else {
      const payload = selected.map((row) =>
        Object.fromEntries(
          visibleColumnEntries.value.map((entry) => [
            entry.column.name,
            row.values[entry.index] ?? null,
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
  const lines = [visibleColumnNames.value.map(csvEscape).join(",")];
  for (const row of pageVisibleRows.value) {
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
      columns: visibleColumnNames.value,
      rows: pageVisibleRows.value,
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
      visibleColumnNames.value,
      pageVisibleRows.value,
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
  <section ref="rootRef" class="data-view">
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
        <TableViewMenu
          :key="viewScope"
          :profiles="viewProfiles"
          :active-id="activeViewId"
          :default-id="defaultViewId"
          :dirty="viewDirty"
          :disabled="loading || saving || !columns.length"
          :scope-label="databaseLabel || ''"
          @apply="applyView"
          @save="saveView"
          @update="updateView"
          @rename="renameView"
          @remove="deleteView"
          @set-default="setDefaultView"
        />
        <template v-if="pendingCount || canUndo || canRedo">
          <button
            class="icon-button"
            title="撤销上一步（⌘/Ctrl+Z）"
            aria-label="撤销上一步"
            :disabled="!canUndo || saving"
            @click="undo"
          >
            <Icon name="undo" :size="14" />
          </button>
          <button
            class="icon-button"
            title="重做（⌘/Ctrl+Shift+Z）"
            aria-label="重做"
            :disabled="!canRedo || saving"
            @click="redo"
          >
            <Icon name="redo" :size="14" />
          </button>
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
          :disabled="!displayRows.length"
          title="查看当前行详情"
          @click="openDetailFromToolbar"
        >
          <Icon name="view" :size="14" />
          行详情
        </button>
        <button
          class="button ghost compact"
          :disabled="loading || !rows.length"
          title="按列剖析当前页（非全表，不含待提交修改）"
          @click="profileOpen = true"
        >
          <Icon name="list" :size="14" />
          剖析当前页
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
          :disabled="!columns.length"
          title="按当前页 / 选中行 / 全部匹配导出"
          @click="tableExportOpen = true"
        >
          <Icon name="download" :size="14" />
          导出…
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
          :disabled="Boolean(pasteDisabledReason)"
          :title="pasteDisabledReason || '从剪贴板粘贴多行到待提交'"
          @click="openPasteDialog"
        >
          <Icon name="paste" :size="14" />
          粘贴新增…
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
        :class="{ active: findOpen }"
        title="查找当前页（⌘/Ctrl+F）"
        @click="focusSearch"
      >
        <Icon name="search" :size="14" />
        查找
      </button>
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
      <button
        v-if="hiddenColumnCount"
        class="link-button hidden-columns-badge"
        :title="`已隐藏 ${hiddenColumnCount} 列，点击恢复全部列`"
        @click="showAllColumns"
      >
        已隐藏 {{ hiddenColumnCount }} 列 · 显示全部
      </button>
      <span
        v-if="linkedFilterLabel && appliedFilter"
        class="filter-meta linked-filter-meta"
        :title="`来自 ${linkedFilterLabel}`"
      >
        来自 {{ linkedFilterLabel }}
      </span>
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

    <GridFindBar
      v-if="findOpen"
      ref="findBar"
      :query="findQuery"
      :case-sensitive="findCaseSensitive"
      :match-count="findMatches.length"
      :row-count="findRowCount"
      :current="findIndex"
      :scope-hint="findScopeHint"
      placeholder="查找当前页…"
      @update:query="findQuery = $event"
      @update:case-sensitive="findCaseSensitive = $event"
      @next="nextMatch"
      @prev="previousMatch"
      @close="closeSearch"
    />

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
      <div v-if="!loading && columns.length" class="result-grid-column-tools">
      <button
        class="button ghost compact"
        :title="`手动隐藏 ${hiddenNames.length} 列 · 自动隐藏 ${autoHiddenIndexSet.size} 列`"
        @click="columnPanelOpen = true"
      >
        <Icon name="list" :size="12" />
        列
        <span v-if="hiddenColumnCount" class="result-filter-badge">
          {{ hiddenColumnCount }}
        </span>
      </button>
    </div>
    <ColumnVisibilityPanel
      :open="columnPanelOpen"
      :columns="visibilityColumns"
      :manual-hidden="manualHiddenKeys"
      :auto-hidden="
        visibilityColumns
          .filter((column) => autoHiddenIndexSet.has(column.index))
          .map((column) => column.key)
      "
      :auto-mode="autoHideMode"
      :has-data="displayRows.length > 0"
      :row-count="displayRows.length"
      row-label="基于当前页，含待提交修改"
      @close="columnPanelOpen = false"
      @toggle="toggleVisibilityColumn"
      @show-all="showAllColumns"
      @set-mode="setAutoHideMode"
    />
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
            v-for="cellEntry in visibleColumnEntries"
            :key="cellEntry.index"
            :style="{ width: columnWidthAt(cellEntry.index) + 'px' }"
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
              v-for="cellEntry in visibleColumnEntries"
              :key="cellEntry.index"
              :data-col-index="cellEntry.index"
              class="sortable"
              :class="{
                'frozen-head': frozenIndices.includes(cellEntry.index),
                'frozen-boundary': cellEntry.index === lastFrozenIndex,
                'col-dragging':
                  columnDragState?.index === cellEntry.index,
                'col-drop-before':
                  columnDragState?.target === cellEntry.index &&
                  !columnDragState.after,
                'col-drop-after':
                  columnDragState?.target === cellEntry.index &&
                  columnDragState.after,
              }"
              :style="
                frozenLeft[cellEntry.index] !== undefined
                  ? { left: `${frozenLeft[cellEntry.index]}px` }
                  : undefined
              "
              tabindex="0"
              :aria-sort="
                sortDirectionFor(cellEntry.column.name) === 'asc'
                  ? 'ascending'
                  : sortDirectionFor(cellEntry.column.name) === 'desc'
                    ? 'descending'
                    : 'none'
              "
              @mousedown="startColumnDrag($event, cellEntry.index)"
              @click="handleHeaderClick(cellEntry.column.name, $event.shiftKey)"
              @keydown.enter.prevent="
                toggleSort(cellEntry.column.name, $event.shiftKey)
              "
              @keydown.space.prevent="
                toggleSort(cellEntry.column.name, $event.shiftKey)
              "
              @contextmenu.prevent="openHeaderMenu($event, cellEntry.index)"
            >
              <span class="column-head">
                <span>{{ cellEntry.column.name }}</span>
                <span v-if="cellEntry.column.primaryKey" class="pk-badge">PK</span>
                <span
                  v-if="columnFormatRule(cellEntry.index)"
                  class="column-format-badge"
                  role="img"
                  aria-label="已应用显示格式"
                  :title="columnFormatSummary(cellEntry.index)"
                >fx</span>
                <span
                  v-if="cellEntry.index === lastFrozenIndex"
                  class="frozen-pin"
                  role="img"
                  :aria-label="`已冻结 ${frozenColumns.length} 列`"
                  :title="`已冻结 ${frozenColumns.length} 列`"
                >
                  <Icon name="pin" :size="11" />
                </span>
                <Icon
                  v-if="sortDirectionFor(cellEntry.column.name)"
                  :name="
                    sortDirectionFor(cellEntry.column.name) === 'asc'
                      ? 'chevron-up'
                      : 'chevron-down'
                  "
                  :size="12"
                  class="sort-icon"
                />
              </span>
              <span class="column-type">{{ cellEntry.column.dataType }}</span>
              <span
                class="col-resizer"
                title="拖拽调整列宽，双击恢复默认"
                @mousedown="startResize($event, cellEntry.index)"
                @dblclick.stop="resetColumnWidth(cellEntry.index)"
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
              :colspan="visibleColumnEntries.length + 2"
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
              v-for="cellEntry in visibleColumnEntries"
              :key="cellEntry.index"
              :class="{
                null: isNull(entry.row.values[cellEntry.index]),
                num: typeof entry.row.values[cellEntry.index] === 'number',
                editable: canEdit,
                modified: isCellModified(entry.index, cellEntry.index),
                'grid-match-active':
                  currentMatch?.rowIndex === entry.index &&
                  currentMatch?.columnIndex === cellEntry.index,
                'frozen-cell': frozenIndices.includes(cellEntry.index),
                'frozen-boundary': cellEntry.index === lastFrozenIndex,
                'format-cell-warning': Boolean(
                  formatWarning(
                    entry.row.values[cellEntry.index],
                    cellEntry.index,
                  ),
                ),
              }"
              :style="
                frozenLeft[cellEntry.index] !== undefined
                  ? { left: `${frozenLeft[cellEntry.index]}px` }
                  : undefined
              "
              :title="displayCellTitle(entry.row.values[cellEntry.index], cellEntry.index)"
              tabindex="-1"
              :data-cell="`${entry.index}:${cellEntry.index}`"
              @dblclick="startEdit(entry.index, cellEntry.index)"
              @contextmenu.prevent="
                openCellMenu($event, entry.index, cellEntry.index)
              "
            >
              <input
                v-if="
                  editing &&
                  editing.rowIndex === entry.index &&
                  editing.colIndex === cellEntry.index
                "
                v-model="editing.value"
                v-focus
                class="data-edit-input"
                @keydown.enter.prevent="commitEdit"
                @keydown.esc.prevent="editing = null"
                @blur="commitEdit"
              />
              <template v-else-if="highlightFor(entry.index, cellEntry.index)">
                <span
                  v-for="(segment, segmentIndex) in highlightFor(
                    entry.index,
                    cellEntry.index,
                  ) ?? []"
                  :key="segmentIndex"
                  :class="{ 'grid-match': segment.match }"
                >{{ segment.text }}</span>
              </template>
              <template v-else>{{
                displayCellText(entry.row.values[cellEntry.index], cellEntry.index)
              }}</template>
            </td>
          </tr>
          <tr
            v-if="virtualEnabled && virtualPaddingBottom > 0"
            class="virtual-spacer"
            aria-hidden="true"
          >
            <td
              :colspan="visibleColumnEntries.length + 2"
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

    <ColumnFormatDialog
      :open="Boolean(formatDialog)"
      :column-name="
        formatDialog ? (columns[formatDialog.colIndex]?.name ?? '') : ''
      "
      :column-type="
        formatDialog ? (columns[formatDialog.colIndex]?.dataType ?? null) : null
      "
      :samples="formatSamples"
      :initial="
        formatDialog ? columnFormatRule(formatDialog.colIndex) : null
      "
      @cancel="formatDialog = null"
      @apply="applyColumnFormat"
    />

    <FilterBuilderDialog
      :open="filterBuilderOpen"
      :columns="columns"
      :driver="driverKind"
      @close="filterBuilderOpen = false"
      @apply="applyBuilderFilter"
    />

    <div
      v-if="headerMenu"
      class="context-menu"
      role="menu"
      :style="{ left: headerMenu.x + 'px', top: headerMenu.y + 'px' }"
      @click.stop
    >
      <button
        class="context-menu-item"
        :disabled="!canMoveHeader('first')"
        aria-label="移到最前"
        @click="moveHeaderColumn('first')"
      >
        <Icon name="chevron-up" :size="14" />
        移到最前
      </button>
      <button
        class="context-menu-item"
        :disabled="!canMoveHeader('left')"
        aria-label="左移一列"
        @click="moveHeaderColumn('left')"
      >
        <Icon name="chevron-left" :size="14" />
        左移
      </button>
      <button
        class="context-menu-item"
        :disabled="!canMoveHeader('right')"
        aria-label="右移一列"
        @click="moveHeaderColumn('right')"
      >
        <Icon name="chevron-right" :size="14" />
        右移
      </button>
      <button
        class="context-menu-item"
        :disabled="!canMoveHeader('last')"
        aria-label="移到最后"
        @click="moveHeaderColumn('last')"
      >
        <Icon name="chevron-down" :size="14" />
        移到最后
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        :disabled="!canFreezeColumn(headerMenu.colIndex)"
        :title="
          canFreezeColumn(headerMenu.colIndex)
            ? undefined
            : visibleColumns.length <= 1
              ? '至少保留一列可滚动'
              : '当前无需横向滚动'
        "
        @click="freezeColumn(headerMenu.colIndex)"
      >
        <Icon name="pin" :size="14" />
        {{ frozenMenuLabel(headerMenu.colIndex) }}
      </button>
      <button
        v-if="frozenIndices.length"
        class="context-menu-item"
        @click="unfreezeColumns"
      >
        <Icon name="close" :size="14" />
        取消冻结
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="openColumnFormat(headerMenu.colIndex)">
        <Icon name="edit" :size="14" />
        显示格式…
      </button>
      <button
        v-if="columnFormatRule(headerMenu.colIndex)"
        class="context-menu-item"
        @click="clearColumnFormatEntry(headerMenu.colIndex)"
      >
        <Icon name="close" :size="14" />
        清除此列格式
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        :disabled="!canHideColumn(headerMenu.colIndex)"
        :title="
          canHideColumn(headerMenu.colIndex)
            ? undefined
            : '至少保留一列可见'
        "
        @click="hideColumn(headerMenu.colIndex)"
      >
        <Icon name="close" :size="14" />
        隐藏此列
      </button>
      <button
        v-if="hiddenColumnCount"
        class="context-menu-item"
        @click="showAllColumns"
      >
        <Icon name="view" :size="14" />
        显示所有列（{{ hiddenColumnCount }}）
      </button>
    </div>

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
      <button class="context-menu-item" @click="openDetailFromMenu">
        <Icon name="list" :size="14" />
        查看行详情…
      </button>
      <button
        class="context-menu-item"
        :disabled="Boolean(pasteDisabledReason)"
        :title="pasteDisabledReason || '从剪贴板粘贴多行'"
        @click="closeCellMenu(); openPasteDialog()"
      >
        <Icon name="paste" :size="14" />
        粘贴新增行…
      </button>
      <template
        v-if="
          isTable &&
          (foreignKeysLoading ||
            foreignKeysError ||
            outgoingLinks.length ||
            incomingLinks.length)
        "
      >
        <div class="context-separator"></div>
        <span class="context-menu-label">关联</span>
        <button
          v-if="foreignKeysLoading"
          class="context-menu-item"
          disabled
        >
          正在加载关联信息…
        </button>
        <button
          v-else-if="foreignKeysError"
          class="context-menu-item"
          @click="openForeignKeysFromMenu"
        >
          <Icon name="warning" :size="14" />
          关联信息不可用，点击重试
        </button>
        <template v-else>
          <button
            v-for="link in outgoingLinks"
            :key="`out-${link.key}`"
            class="context-menu-item"
            :disabled="!link.enabled"
            :title="link.reason || `打开 ${link.foreignKey.referencedTable}`"
            @click="openOutboundLink(link)"
          >
            <Icon name="external" :size="14" />
            打开引用行 → {{ link.foreignKey.referencedTable }}（{{
              link.foreignKey.referencedColumns.join(", ")
            }}）
          </button>
          <button
            v-for="link in incomingLinks.slice(0, 30)"
            :key="`in-${link.key}`"
            class="context-menu-item"
            :disabled="!link.enabled"
            :title="link.reason || `查看 ${link.foreignKey.table}`"
            @click="openInboundLink(link)"
          >
            <Icon name="list" :size="14" />
            查看引用记录 → {{ link.foreignKey.table }}.{{
              link.foreignKey.columns.join(", ")
            }}
          </button>
          <button
            v-if="incomingLinks.length > 30"
            class="context-menu-item"
            disabled
          >
            引用过多（{{ incomingLinks.length }} 项），请从结构页查看
          </button>
        </template>
      </template>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyCellValue">
        <Icon name="copy" :size="14" />
        复制单元格
      </button>
      <button
        v-if="cellMenu && columnFormatRule(cellMenu.colIndex)"
        class="context-menu-item"
        title="只复制格式化后的显示文本；导出、编辑和整行 JSON 仍使用原始值"
        @click="copyDisplayCellValue"
      >
        <Icon name="copy" :size="14" />
        复制显示值
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

    <ResultProfile
      v-if="profileOpen"
      dialog
      :data="tableProfileData"
      :table-page="tableProfilePage"
      @close="profileOpen = false"
    />

    <TableDataExportDialog
      :open="tableExportOpen"
      :connection-id="connectionId"
      :table="table"
      :driver="driverKind"
      :columns="columns.map((column) => column.name)"
      :visible-columns="visibleColumnNames"
      :page-rows="pageExportRows"
      :selected-rows="selectedExportRows"
      :total="total"
      :filter="appliedFilter"
      :sorts="sorts"
      :has-pending-changes="hasPendingChanges"
      :scope-label="databaseLabel || ''"
      @close="tableExportOpen = false"
      @exported="
        tableExportOpen = false;
        showMessage(`已导出 ${$event[1]} 行到 ${$event[0]}`);
      "
    />

    <PasteRowsDialog
      :open="pasteOpen"
      :table="table"
      :columns="columns"
      :disabled-reason="pasteDisabledReason"
      :pending-inserts="pendingInserts.length"
      @close="pasteOpen = false"
      @confirm="applyPastedRows"
    />

    <RowDetailPanel
      :open="detailOpen"
      :title="table"
      :row-label="detailRowLabel"
      :fields="detailFields"
      :editable="detailEditable"
      :read-only-reason="detailReadOnlyReason"
      :row-state="detailRow?.state"
      :can-previous="detailRowIndex > 0"
      :can-next="detailRowIndex < displayRows.length - 1"
      @close="closeRowDetail"
      @previous="moveRowDetail(-1)"
      @next="moveRowDetail(1)"
      @edit="applyDetailEdit"
      @set-null="setDetailNull"
      @view-value="viewDetailValue"
      @copy-value="copyDetailValue"
      @copy-row-json="copyDetailRowJson"
    />

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
