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
import CellValueDialog from "./CellValueDialog.vue";
import ColumnFormatDialog from "./ColumnFormatDialog.vue";
import ColumnVisibilityPanel from "./ColumnVisibilityPanel.vue";
import GridFindBar from "./GridFindBar.vue";
import Icon from "./Icon.vue";
import ResultFilterBar from "./ResultFilterBar.vue";
import RowDetailPanel from "./RowDetailPanel.vue";
import { writeClipboard as copyToClipboard } from "../lib/clipboard";
import {
  formatColumnKeys,
  formatDisplayValue,
  formatRuleSummary,
  type ColumnFormatEntry,
  type ColumnFormatRule,
} from "../lib/column-format";
import { reorderByName } from "../lib/column-order";
import {
  autoHiddenKeys,
  findEmptyColumnKeys,
  keepOneAutoVisible,
  type AutoHideMode,
  type ColumnVisibilityColumn,
} from "../lib/column-visibility";
import { frozenOffsets } from "../lib/frozen-columns";
import type { ChartData } from "../lib/result-chart";
import {
  isCellSelected,
  normalizeRange,
  selectedVisibleColumns,
  type GridCell,
  type GridSelection,
} from "../lib/grid-selection";
import {
  formatStatValue,
  summarizeCells,
  type GridStats,
} from "../lib/grid-stats";
import {
  RESULT_FILTER_LIMIT,
  describeFilter,
  filterResultRows,
  isFilterConditionActive,
  type ResultFilterCondition,
} from "../lib/result-filter";
import {
  countMatchRows,
  findGridMatches,
  formatGridCell,
  highlightSegments,
  type GridMatch,
} from "../lib/grid-search";
import { columnNameIssue, type ResultExportSnapshot } from "../lib/result-export";
import { useVirtualRows } from "../lib/virtual-rows";
import { buildRowJson } from "../lib/row-copy";
import type { QueryResult, RowDetailField } from "../lib/types";

interface CellMenu {
  x: number;
  y: number;
  rowIndex: number;
  colIndex: number;
}

interface HeaderMenu {
  x: number;
  y: number;
  colIndex: number;
}

interface ResizeState {
  index: number;
  startX: number;
  startWidth: number;
}

const props = defineProps<{
  result: QueryResult | null;
  running: boolean;
  formats?: ColumnFormatEntry[];
}>();

const emit = defineEmits<{
  "hidden-change": [count: number];
  "export-request": [snapshot: ResultExportSnapshot];
  "filter-change": [
    payload: {
      count: number;
      active: boolean;
      open: boolean;
      summary: string | null;
    },
  ];
  "chart-data": [data: ChartData];
  "update:formats": [entries: ColumnFormatEntry[]];
}>();

const columnWidths = reactive<Record<number, number>>({});
const defaultWidths = ref<number[]>([]);
const cellMenu = ref<CellMenu | null>(null);
const headerMenu = ref<HeaderMenu | null>(null);
const formatDialog = ref<{ colIndex: number } | null>(null);
const columnPanelOpen = ref(false);
const autoHideMode = ref<AutoHideMode>("off");
const autoExceptions = reactive(new Set<number>());
const hiddenColumns = reactive(new Set<number>());
const columnOrder = ref<number[]>([]);
const lastColumnNames = ref<string[]>([]);
const columnDrag = ref<{
  index: number;
  startX: number;
  startY: number;
  frozen: boolean;
  moved: boolean;
  target: number | null;
  after: boolean;
} | null>(null);
let suppressHeaderClick = false;
const toast = ref("");
const sortIndex = ref<number | null>(null);
const sortDirection = ref<"asc" | "desc">("asc");
const findOpen = ref(false);
const findQuery = ref("");
const findTerm = ref("");
const findCaseSensitive = ref(false);
const findIndex = ref(0);
const findBar = ref<InstanceType<typeof GridFindBar> | null>(null);
let findTimer = 0;
let restoreFocus: HTMLElement | null = null;
const valueViewer = ref<{
  rowIndex: number;
  colIndex: number;
} | null>(null);
const detailOpen = ref(false);
const detailRowIndex = ref(0);
const frozenIndices = ref<number[]>([]);
const frozenNames = ref<string[]>([]);
const filterOpen = ref(false);
const filterCaseSensitive = ref(false);
const filterConditions = ref<ResultFilterCondition[]>([]);
const selection = ref<GridSelection | null>(null);
const activeCell = ref<GridCell | null>(null);
const draggingSelection = ref(false);
let filterSequence = 0;
const scrollRef = ref<HTMLElement | null>(null);
let resizeState: ResizeState | null = null;
let toastTimer = 0;

const visibilityColumns = computed<ColumnVisibilityColumn[]>(() => {
  const columns = props.result?.columns ?? [];
  const keys = formatColumnKeys(columns.map((name) => ({ name })));
  return columns.map((name, index) => ({
    key: keys[index] ?? String(index),
    name,
    index,
  }));
});

const manualHiddenKeys = computed(() =>
  visibilityColumns.value
    .filter((column) => hiddenColumns.has(column.index))
    .map((column) => column.key),
);

const emptyColumnKeys = computed(() =>
  findEmptyColumnKeys(
    props.result?.rows ?? [],
    visibilityColumns.value,
    { mode: autoHideMode.value },
  ),
);

const autoHiddenIndices = computed(() => {
  if (autoHideMode.value === "off") return new Set<number>();
  const exceptions = new Set(
    visibilityColumns.value
      .filter((column) => autoExceptions.has(column.index))
      .map((column) => column.key),
  );
  const keys = autoHiddenKeys(
    visibilityColumns.value,
    emptyColumnKeys.value,
    exceptions,
  );
  const manual = new Set(manualHiddenKeys.value);
  keepOneAutoVisible(visibilityColumns.value, manual, keys);
  return new Set(
    visibilityColumns.value
      .filter((column) => keys.has(column.key))
      .map((column) => column.index),
  );
});

const visibleColumns = computed(() => {
  const columns = props.result?.columns ?? [];
  const ordered: Array<{ name: string; index: number }> = [];
  const seen = new Set<number>();
  for (const index of columnOrder.value) {
    if (index < 0 || index >= columns.length || seen.has(index)) continue;
    seen.add(index);
    if (!hiddenColumns.has(index) && !autoHiddenIndices.value.has(index)) {
      ordered.push({ name: columns[index], index });
    }
  }
  for (let index = 0; index < columns.length; index += 1) {
    if (seen.has(index) || hiddenColumns.has(index) || autoHiddenIndices.value.has(index)) continue;
    ordered.push({ name: columns[index], index });
  }
  const frozen = ordered.filter((column) =>
    frozenIndices.value.includes(column.index),
  );
  const rest = ordered.filter(
    (column) => !frozenIndices.value.includes(column.index),
  );
  return [...frozen, ...rest];
});

const frozenColumns = computed(() =>
  visibleColumns.value.filter((column) =>
    frozenIndices.value.includes(column.index),
  ),
);
const lastFrozenIndex = computed(
  () => frozenColumns.value[frozenColumns.value.length - 1]?.index ?? null,
);
const frozenLeft = computed(() =>
  frozenOffsets(
    (props.result?.columns ?? []).map((_, index) => columnWidth(index)),
    frozenColumns.value.map((column) => column.index),
    46,
  ),
);

function updateFrozenNamesFromOrder() {
  frozenNames.value = visibleColumns.value
    .filter((column) => frozenIndices.value.includes(column.index))
    .map((column) => column.name);
}

function columnBlock(index: number): Array<{ name: string; index: number }> {
  const frozen = frozenIndices.value.includes(index);
  return visibleColumns.value.filter(
    (column) => frozenIndices.value.includes(column.index) === frozen,
  );
}

function reorderColumn(from: number, to: number, after: boolean) {
  if (from === to) return;
  const frozenFrom = frozenIndices.value.includes(from);
  const frozenTo = frozenIndices.value.includes(to);
  if (frozenFrom !== frozenTo) {
    flashToast("请先取消冻结或先冻结该列，再跨冻结块移动");
    return;
  }
  const order = [...columnOrder.value];
  const fromPosition = order.indexOf(from);
  if (fromPosition < 0) return;
  order.splice(fromPosition, 1);
  const target = order.indexOf(to);
  if (target < 0) return;
  order.splice(target + (after ? 1 : 0), 0, from);
  columnOrder.value = order;
  updateFrozenNamesFromOrder();
}

function startColumnDrag(event: MouseEvent, index: number) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest(".col-resizer")) return;
  columnDrag.value = {
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
  const drag = columnDrag.value;
  if (!drag) return;
  if (
    !drag.moved &&
    Math.abs(event.clientX - drag.startX) < 4 &&
    Math.abs(event.clientY - drag.startY) < 4
  ) {
    return;
  }
  drag.moved = true;
  event.preventDefault();
  const element = document.elementFromPoint(event.clientX, event.clientY);
  const header = element?.closest<HTMLElement>("th[data-col-index]");
  if (!header) {
    columnDrag.value = { ...drag, target: null };
    return;
  }
  const targetIndex = Number(header.dataset.colIndex);
  if (!Number.isInteger(targetIndex) || targetIndex === drag.index) {
    columnDrag.value = { ...drag, target: null };
    return;
  }
  const rect = header.getBoundingClientRect();
  columnDrag.value = {
    ...drag,
    target: targetIndex,
    after: event.clientX > rect.left + rect.width / 2,
  };
}

function finishColumnDrag() {
  const drag = columnDrag.value;
  cleanupColumnDrag();
  if (!drag) return;
  if (drag.moved) {
    suppressHeaderClick = true;
    window.setTimeout(() => {
      suppressHeaderClick = false;
    }, 0);
  }
  if (!drag.moved || drag.target === null) return;
  reorderColumn(drag.index, drag.target, drag.after);
}

function cleanupColumnDrag() {
  columnDrag.value = null;
  window.removeEventListener("mousemove", moveColumnDrag);
  window.removeEventListener("mouseup", finishColumnDrag);
  window.removeEventListener("keydown", cancelColumnDragOnEscape, true);
}

function cancelColumnDrag() {
  if (!columnDrag.value) return;
  cleanupColumnDrag();
  suppressHeaderClick = true;
  window.setTimeout(() => {
    suppressHeaderClick = false;
  }, 0);
}

function cancelColumnDragOnEscape(event: KeyboardEvent) {
  if (event.key !== "Escape" || !columnDrag.value) return;
  event.preventDefault();
  event.stopPropagation();
  cancelColumnDrag();
}

function handleHeaderClick(index: number) {
  if (suppressHeaderClick) {
    suppressHeaderClick = false;
    return;
  }
  toggleSort(index);
}

function canMoveHeader(
  direction: "left" | "right" | "first" | "last",
): boolean {
  const menu = headerMenu.value;
  if (!menu) return false;
  const block = columnBlock(menu.colIndex);
  const position = block.findIndex((column) => column.index === menu.colIndex);
  if (position < 0 || block.length < 2) return false;
  if (direction === "left" || direction === "first") return position > 0;
  return position < block.length - 1;
}

function moveHeaderColumn(
  direction: "left" | "right" | "first" | "last",
) {
  const menu = headerMenu.value;
  if (!menu || !canMoveHeader(direction)) return;
  const block = columnBlock(menu.colIndex);
  const position = block.findIndex((column) => column.index === menu.colIndex);
  let target: { index: number } | undefined;
  let after = false;
  if (direction === "left") {
    target = block[position - 1];
    after = false;
  } else if (direction === "right") {
    target = block[position + 1];
    after = true;
  } else if (direction === "first") {
    target = block[0];
    after = false;
  } else {
    target = block[block.length - 1];
    after = true;
  }
  closeMenus();
  if (target) reorderColumn(menu.colIndex, target.index, after);
}

function frozenMenuLabel(index: number): string {
  const position = visibleColumns.value.findIndex(
    (column) => column.index === index,
  );
  const names = visibleColumns.value
    .slice(0, position >= 0 ? position + 1 : 0)
    .map((column) => column.name);
  if (!names.length) return "冻结到此列";
  const preview =
    names.length <= 3
      ? names.join("、")
      : `${names.slice(0, 2).join("、")}… 等 ${names.length} 列`;
  return `冻结到此列（${preview}）`;
}

function canFreezeColumn(index: number): boolean {
  const position = visibleColumns.value.findIndex(
    (column) => column.index === index,
  );
  if (visibleColumns.value.length <= 1 || position < 0) return false;
  if (position >= visibleColumns.value.length - 1) return false;
  const element = scrollRef.value;
  if (element && element.scrollWidth <= element.clientWidth + 1) {
    return false;
  }
  return true;
}

function freezeColumn(index: number) {
  if (!canFreezeColumn(index)) {
    flashToast(
      visibleColumns.value.length <= 1
        ? "至少保留一列可滚动"
        : "当前无需横向滚动",
    );
    return;
  }
  const position = visibleColumns.value.findIndex(
    (column) => column.index === index,
  );
  if (position < 0) return;
  const frozen = visibleColumns.value.slice(0, position + 1);
  frozenIndices.value = frozen.map((column) => column.index);
  frozenNames.value = frozen.map((column) => column.name);
  closeMenus();
}

function unfreezeColumns() {
  frozenIndices.value = [];
  frozenNames.value = [];
  closeMenus();
}

const filterBarColumns = computed(() =>
  (props.result?.columns ?? []).map((name, index) => ({
    name,
    hidden: hiddenColumns.has(index),
  })),
);

function addFilterCondition(columnIndex = 0, value = ""): boolean {
  if (filterConditions.value.length >= RESULT_FILTER_LIMIT) return false;
  filterSequence += 1;
  filterConditions.value = [
    ...filterConditions.value,
    {
      id: filterSequence,
      columnIndex,
      operator: "contains",
      value,
    },
  ];
  return true;
}

function updateFilterCondition(
  index: number,
  patch: Partial<ResultFilterCondition>,
) {
  filterConditions.value = filterConditions.value.map((condition, position) =>
    position === index ? { ...condition, ...patch } : condition,
  );
}

function removeFilterCondition(index: number) {
  filterConditions.value = filterConditions.value.filter(
    (_, position) => position !== index,
  );
}

function clearFilter() {
  filterConditions.value = [];
}

function toggleFilter() {
  if (!props.result || props.running) return;
  filterOpen.value = !filterOpen.value;
}

function openFilter() {
  if (!props.result || props.running) return;
  filterOpen.value = true;
}

function filterByColumn(index: number) {
  filterOpen.value = true;
  addFilterCondition(index);
  closeMenus();
}

function filterByMenuValue() {
  const menu = cellMenu.value;
  if (!menu) return;
  const value = sortedRows.value[menu.rowIndex]?.[menu.colIndex];
  filterOpen.value = true;
  if (value === null || value === undefined) {
    if (!addFilterCondition(menu.colIndex, "")) {
      flashToast("最多 5 个条件");
    } else {
      updateFilterCondition(filterConditions.value.length - 1, {
        operator: "is-null",
      });
    }
  } else if (!addFilterCondition(menu.colIndex, formatCell(value))) {
    flashToast("最多 5 个条件");
  }
  closeMenus();
}

watch(
  () => visibleColumns.value.map((column) => column.index).join("|"),
  () => {
    const visible = new Set(
      visibleColumns.value.map((column) => column.index),
    );
    const next = frozenIndices.value.filter((index) => visible.has(index));
    if (next.length === frozenIndices.value.length) return;
    frozenIndices.value = next;
    frozenNames.value = visibleColumns.value
      .filter((column) => next.includes(column.index))
      .map((column) => column.name);
    if (!next.length) frozenNames.value = [];
  },
);

const jsonDisabledReason = computed(() => {
  if (props.running) return "查询执行中";
  if (!props.result) return "没有结果";
  return columnNameIssue(visibleColumns.value.map((column) => column.name));
});

const activeFilterCount = computed(
  () =>
    filterConditions.value.filter(isFilterConditionActive).length,
);
const filterSummary = computed(() =>
  describeFilter(
    filterConditions.value,
    props.result?.columns ?? [],
  ),
);
const filteredRows = computed(() =>
  filterResultRows(
    props.result?.rows ?? [],
    filterConditions.value,
    filterCaseSensitive.value,
  ),
);

watch(
  [activeFilterCount, filterOpen],
  ([count, open]) => {
    emit("filter-change", {
      count,
      active: count > 0,
      open,
      summary: filterSummary.value,
    });
  },
  { immediate: true },
);

watch(
  filterConditions,
  () => {
    selection.value = null;
    activeCell.value = null;
  },
  { deep: true },
);

watch(
  () => visibleColumns.value.map((column) => column.index).join("|"),
  () => {
    if (!selection.value) return;
    if (
      !selectedVisibleColumns(
        selection.value,
        visibleColumns.value.map((column) => column.index),
      ).length
    ) {
      selection.value = null;
      activeCell.value = null;
    }
  },
);

const sortedRows = computed<unknown[][]>(() => {
  const rows = filteredRows.value;
  const index = sortIndex.value;
  if (index === null) return rows;

  const direction = sortDirection.value === "asc" ? 1 : -1;
  return [...rows].sort((left, right) => {
    const a = left[index];
    const b = right[index];
    const aNull = a === null || a === undefined;
    const bNull = b === null || b === undefined;
    if (aNull && bNull) return 0;
    if (aNull) return 1;
    if (bNull) return -1;
    if (typeof a === "number" && typeof b === "number") {
      return (a - b) * direction;
    }
    if (typeof a === "boolean" && typeof b === "boolean") {
      return (Number(a) - Number(b)) * direction;
    }
    return (
      String(a).localeCompare(String(b), "zh-CN", { numeric: true }) *
      direction
    );
  });
});

const selectionRange = computed(() =>
  selection.value ? normalizeRange(selection.value) : null,
);
const selectionVisibleColumns = computed(() =>
  selectedVisibleColumns(
    selection.value,
    visibleColumns.value.map((column) => column.index),
  ),
);
const selectionRowRange = computed<[number, number] | null>(() => {
  const range = selectionRange.value;
  if (!range || !sortedRows.value.length) return null;
  const top = Math.max(0, range.top);
  const bottom = Math.min(sortedRows.value.length - 1, range.bottom);
  if (bottom < top) return null;
  return [top, bottom];
});
const selectionStats = computed<GridStats | null>(() => {
  const rows = selectionRowRange.value;
  const columns = selectionVisibleColumns.value;
  if (!rows || !columns.length) return null;
  const values: unknown[] = [];
  for (let row = rows[0]; row <= rows[1]; row += 1) {
    for (const column of columns) {
      values.push(sortedRows.value[row]?.[column]);
    }
  }
  return summarizeCells(values);
});
const selectionSummaryVisible = computed(
  () => Boolean(selection.value && selectionStats.value),
);
const selectionRowCount = computed(() => {
  const rows = selectionRowRange.value;
  return rows ? rows[1] - rows[0] + 1 : 0;
});
const selectionColumnCount = computed(
  () => selectionVisibleColumns.value.length,
);
const selectionTitles = computed(() => {
  const stats = selectionStats.value;
  if (!stats) return "";
  return `求和 ${stats.sum} / 平均 ${stats.average} / 最小 ${stats.min} / 最大 ${stats.max}`;
});

function isGridSelected(row: number, column: number): boolean {
  return isCellSelected(selection.value, row, column);
}

function isActiveCell(row: number, column: number): boolean {
  return activeCell.value?.row === row && activeCell.value?.column === column;
}

function buildChartData(): ChartData {
  return {
    columns: props.result?.columns ?? [],
    visibleColumns: visibleColumns.value.map((column) => ({
      name: column.name,
      index: column.index,
    })),
    rows: sortedRows.value,
    truncated: Boolean(props.result?.truncated),
  };
}

watch(
  [sortedRows, () => visibleColumns.value.map((column) => column.index).join("|")],
  () => emit("chart-data", buildChartData()),
  { immediate: true },
);

function clearSelection() {
  selection.value = null;
  activeCell.value = null;
  draggingSelection.value = false;
  window.removeEventListener("mouseup", stopSelectionDrag);
}

function focusGrid() {
  scrollRef.value?.focus({ preventScroll: true });
}

function setSingleSelection(row: number, column: number) {
  const cell = { row, column };
  selection.value = { anchor: cell, focus: cell };
  activeCell.value = cell;
}

function onCellMouseDown(event: MouseEvent, row: number, column: number) {
  if (event.button !== 0) return;
  event.preventDefault();
  focusGrid();
  if (event.shiftKey && selection.value) {
    selection.value = {
      anchor: { ...selection.value.anchor },
      focus: { row, column },
    };
  } else {
    setSingleSelection(row, column);
  }
  activeCell.value = { row, column };
  draggingSelection.value = true;
  window.addEventListener("mouseup", stopSelectionDrag);
}

function onCellMouseEnter(row: number, column: number) {
  if (!draggingSelection.value || !selection.value) return;
  selection.value = {
    anchor: selection.value.anchor,
    focus: { row, column },
  };
  activeCell.value = { row, column };
}

function stopSelectionDrag() {
  draggingSelection.value = false;
  window.removeEventListener("mouseup", stopSelectionDrag);
}

function visibleColumnPosition(column: number): number {
  return visibleColumns.value.findIndex((item) => item.index === column);
}

function moveActiveCell(
  rowDelta: number,
  columnDelta: number,
  extend: boolean,
) {
  if (!sortedRows.value.length || !visibleColumns.value.length) return;
  const current =
    activeCell.value && {
      row: Math.min(sortedRows.value.length - 1, Math.max(0, activeCell.value.row)),
      column: activeCell.value.column,
    };
  let position = current ? visibleColumnPosition(current.column) : -1;
  if (position < 0) position = 0;
  const nextPosition = Math.max(
    0,
    Math.min(visibleColumns.value.length - 1, position + columnDelta),
  );
  const next = {
    row: Math.max(
      0,
      Math.min(
        sortedRows.value.length - 1,
        (current?.row ?? 0) + rowDelta,
      ),
    ),
    column: visibleColumns.value[nextPosition].index,
  };
  activeCell.value = next;
  if (extend) {
    const anchor = selection.value?.anchor ?? current ?? next;
    selection.value = { anchor, focus: next };
  } else {
    selection.value = { anchor: next, focus: next };
  }
  focusGrid();
  void nextTick(() => scrollToRow(next.row, { center: true }));
}

function onGridKeydown(event: KeyboardEvent) {
  const target = event.target instanceof Element ? event.target : null;
  if (target?.closest('input, textarea, select, [contenteditable="true"]')) {
    return;
  }
  const modifier = event.metaKey || event.ctrlKey;
  const key = event.key.toLowerCase();
  if (modifier && key === "c" && selection.value) {
    event.preventDefault();
    copySelection(false);
    return;
  }
  if (modifier && key === "a") {
    if (!sortedRows.value.length || !visibleColumns.value.length) return;
    event.preventDefault();
    const first = visibleColumns.value[0].index;
    const last = visibleColumns.value[visibleColumns.value.length - 1].index;
    selection.value = {
      anchor: { row: 0, column: first },
      focus: { row: sortedRows.value.length - 1, column: last },
    };
    activeCell.value = { row: 0, column: first };
    return;
  }
  if (event.key === "Escape") {
    if (selection.value) {
      event.preventDefault();
      clearSelection();
    }
    return;
  }
  const extend = event.shiftKey;
  if (event.key === "ArrowUp") {
    event.preventDefault();
    moveActiveCell(-1, 0, extend);
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    moveActiveCell(1, 0, extend);
  } else if (event.key === "ArrowLeft") {
    event.preventDefault();
    moveActiveCell(0, -1, extend);
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    moveActiveCell(0, 1, extend);
  } else if (event.key === "Home" && visibleColumns.value.length) {
    event.preventDefault();
    moveActiveCell(0, -visibleColumns.value.length, extend);
  } else if (event.key === "End" && visibleColumns.value.length) {
    event.preventDefault();
    moveActiveCell(0, visibleColumns.value.length, extend);
  } else if (event.key === "PageUp" || event.key === "PageDown") {
    event.preventDefault();
    const step = Math.max(
      1,
      Math.floor(((scrollRef.value?.clientHeight ?? 300) - 40) / 30),
    );
    moveActiveCell(event.key === "PageUp" ? -step : step, 0, extend);
  }
}

function selectColumn(column: number) {
  if (!sortedRows.value.length) return;
  selection.value = {
    anchor: { row: 0, column },
    focus: { row: sortedRows.value.length - 1, column },
  };
  activeCell.value = { row: 0, column };
  closeMenus();
}

function copySelection(withHeader: boolean) {
  const rows = selectionRowRange.value;
  const columns = selectionVisibleColumns.value;
  if (!rows || !columns.length) return;
  const cellCount = (rows[1] - rows[0] + 1) * columns.length;
  if (cellCount > 50_000) {
    flashToast("选区过大，请缩小范围");
    return;
  }
  const columnNames = columns.map(
    (column) => props.result?.columns[column] ?? "",
  );
  const lines: string[] = [];
  if (withHeader) lines.push(columnNames.map(tsvValue).join("\t"));
  for (let row = rows[0]; row <= rows[1]; row += 1) {
    lines.push(
      columns
        .map((column) => tsvValue(sortedRows.value[row]?.[column]))
        .join("\t"),
    );
  }
  const text = lines.join("\n");
  if (text.length > 10 * 1024 * 1024) {
    flashToast("选区过大，请缩小范围");
    return;
  }
  void writeClipboard(
    text,
    `已复制 ${rows[1] - rows[0] + 1} × ${columns.length} 个单元格`,
  );
}

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
  rowCount: computed(() => sortedRows.value.length),
  rowHeight: 30,
  overscan: 10,
});

const virtualRows = computed(() => {
  const list = sortedRows.value.slice(virtualStart.value, virtualEnd.value);
  return list.map((row, offset) => ({
    row,
    index: virtualStart.value + offset,
  }));
});

const formatMap = computed(() => {
  const map = new Map<string, ColumnFormatRule>();
  for (const entry of props.formats ?? []) {
    map.set(entry.columnKey, entry.rule);
  }
  return map;
});

const formatColumnKeysForResult = computed(() =>
  formatColumnKeys((props.result?.columns ?? []).map((name) => ({ name }))),
);

function columnFormatKey(index: number): string {
  return formatColumnKeysForResult.value[index] ?? "";
}

function ruleForColumn(index: number): ColumnFormatRule | null {
  return formatMap.value.get(columnFormatKey(index)) ?? null;
}

function formatSummaryForColumn(index: number): string {
  const rule = ruleForColumn(index);
  return rule ? formatRuleSummary(rule) : "";
}

function displayCell(value: unknown, index: number): { text: string; warning: string | null } {
  const rule = ruleForColumn(index);
  if (!rule) return { text: formatCell(value), warning: null };
  return formatDisplayValue(value, rule);
}

function formatCellForColumn(value: unknown, index: number): string {
  return displayCell(value, index).text;
}

function formatWarningForColumn(value: unknown, index: number): string | null {
  return displayCell(value, index).warning;
}

function cellTitle(value: unknown, index: number): string {
  const formatted = displayCell(value, index);
  return formatted.warning ? `${formatted.text} · ${formatted.warning}` : formatted.text;
}

function openColumnFormat(index: number) {
  headerMenu.value = null;
  formatDialog.value = { colIndex: index };
}

function clearColumnFormat(index: number) {
  const key = columnFormatKey(index);
  emit(
    "update:formats",
    (props.formats ?? []).filter((entry) => entry.columnKey !== key),
  );
}

function applyColumnFormat(rule: ColumnFormatRule | null) {
  const dialog = formatDialog.value;
  if (!dialog) return;
  const key = columnFormatKey(dialog.colIndex);
  const next = (props.formats ?? []).filter((entry) => entry.columnKey !== key);
  if (rule) next.push({ columnKey: key, rule });
  emit("update:formats", next);
  formatDialog.value = null;
}

const formatSamples = computed(() => {
  const dialog = formatDialog.value;
  if (!dialog || !props.result) return [];
  return sortedRows.value
    .map((row) => row[dialog.colIndex])
    .filter((value) => value !== null && value !== undefined)
    .slice(0, 10);
});

const findMatches = computed<GridMatch[]>(() =>
  findGridMatches(
    sortedRows.value,
    visibleColumns.value,
    findTerm.value,
    {
      caseSensitive: findCaseSensitive.value,
      display: (value, columnIndex) => formatCellForColumn(value, columnIndex),
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
const findScopeHint = computed(() => {
  const loaded = sortedRows.value.length;
  return props.result?.truncated
    ? `仅搜索已加载的 ${loaded} 行（结果已截断），不改变查询与导出范围`
    : `仅搜索已加载的 ${loaded} 行，不改变查询与导出范围`;
});
const findHighlights = computed(() => {
  const highlights = new Map<
    string,
    ReturnType<typeof highlightSegments>
  >();
  if (!findTerm.value.trim()) return highlights;
  for (const entry of virtualRows.value) {
    for (const column of visibleColumns.value) {
      const key = `${entry.index}:${column.index}`;
      if (!matchCellKeys.value.has(key)) continue;
      highlights.set(
        key,
        highlightSegments(
          formatCellForColumn(entry.row[column.index], column.index),
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

const formatCell = formatGridCell;

function isNull(value: unknown): boolean {
  return value === null || value === undefined;
}

function copyValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  return formatCell(value);
}

function tsvValue(value: unknown): string {
  const text = copyValue(value);
  if (/[\t\n\r"]/.test(text)) {
    return `"${text.replaceAll('"', '""')}"`;
  }
  return text;
}

function displayWidth(text: string): number {
  let width = 0;
  for (const character of text) {
    width += character.charCodeAt(0) > 255 ? 12 : 7;
  }
  return width;
}

function estimateColumnWidth(result: QueryResult, index: number): number {
  const name = result.columns[index] ?? "";
  let width = Math.max(88, displayWidth(name) + 30);
  const sample = Math.min(result.rows.length, 80);

  for (let rowIndex = 0; rowIndex < sample; rowIndex += 1) {
    const length = displayWidth(formatCell(result.rows[rowIndex]?.[index]));
    if (length > 320) {
      width = Math.max(width, 340);
      break;
    }
    width = Math.max(width, length + 22);
  }

  return Math.min(360, width);
}

function columnWidth(index: number): number {
  return columnWidths[index] ?? defaultWidths.value[index] ?? 140;
}

function startResize(event: MouseEvent, index: number) {
  event.preventDefault();
  event.stopPropagation();
  resizeState = {
    index,
    startX: event.clientX,
    startWidth: columnWidth(index),
  };
  window.addEventListener("mousemove", moveResize);
  window.addEventListener("mouseup", stopResize);
}

function moveResize(event: MouseEvent) {
  if (!resizeState) return;
  columnWidths[resizeState.index] = Math.max(
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

function resetColumnWidth(index: number) {
  delete columnWidths[index];
}

function toggleSort(index: number) {
  clearSelection();
  if (sortIndex.value !== index) {
    sortIndex.value = index;
    sortDirection.value = "asc";
  } else if (sortDirection.value === "asc") {
    sortDirection.value = "desc";
  } else {
    sortIndex.value = null;
    sortDirection.value = "asc";
  }
  scrollToTop();
}

function openCellMenu(
  event: MouseEvent,
  rowIndex: number,
  colIndex: number,
) {
  event.preventDefault();
  headerMenu.value = null;
  if (!isCellSelected(selection.value, rowIndex, colIndex)) {
    setSingleSelection(rowIndex, colIndex);
  } else {
    activeCell.value = { row: rowIndex, column: colIndex };
  }
  focusGrid();
  cellMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 230)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 380)),
    rowIndex,
    colIndex,
  };
}

const viewerValue = computed<unknown>(() => {
  const viewer = valueViewer.value;
  if (!viewer) return null;
  return sortedRows.value[viewer.rowIndex]?.[viewer.colIndex];
});

const viewerColumn = computed(() => {
  const viewer = valueViewer.value;
  if (!viewer) return "";
  return props.result?.columns[viewer.colIndex] ?? "";
});

const detailRow = computed(
  () => sortedRows.value[detailRowIndex.value] ?? null,
);
const detailRowLabel = computed(
  () => `第 ${detailRowIndex.value + 1} / ${sortedRows.value.length} 行`,
);
const detailFields = computed<RowDetailField[]>(() => {
  const row = detailRow.value;
  if (!row) return [];
  return (props.result?.columns ?? []).map((name, index) => ({
    name,
    value: row[index],
    hidden: hiddenColumns.has(index),
    editable: false,
  }));
});

function openRowDetail(rowIndex: number) {
  if (!sortedRows.value[rowIndex]) return;
  closeMenus();
  detailRowIndex.value = rowIndex;
  detailOpen.value = true;
}

function openDetailFromMenu() {
  const menu = cellMenu.value;
  if (!menu) return;
  const index = menu.rowIndex;
  closeMenus();
  openRowDetail(index);
}

function closeRowDetail() {
  detailOpen.value = false;
}

function moveRowDetail(delta: number) {
  const next = detailRowIndex.value + delta;
  if (next < 0 || next >= sortedRows.value.length) return;
  detailRowIndex.value = next;
}

function viewDetailValue(index: number) {
  openValueViewer(detailRowIndex.value, index);
}

function copyDetailValue(index: number) {
  const field = detailFields.value[index];
  if (!field) return;
  void writeClipboard(copyValue(field.value), `已复制 ${field.name}`);
}

function copyDetailRowJson() {
  const row = detailRow.value;
  const columns = props.result?.columns ?? [];
  if (!row) return;
  void writeClipboard(
    buildRowJson(
      columns,
      columns.map((_, index) => row[index] ?? null),
    ),
    "已复制整行 JSON（全部列）",
  );
}

function openValueViewer(rowIndex: number, colIndex: number) {
  closeMenus();
  valueViewer.value = { rowIndex, colIndex };
}

function openValueFromMenu() {
  const menu = cellMenu.value;
  if (!menu) return;
  openValueViewer(menu.rowIndex, menu.colIndex);
}

function onCellDoubleClick(rowIndex: number, colIndex: number) {
  const value = sortedRows.value[rowIndex]?.[colIndex];
  if (typeof value === "number" || typeof value === "boolean") return;
  openValueViewer(rowIndex, colIndex);
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

function openHeaderMenu(event: MouseEvent, colIndex: number) {
  event.preventDefault();
  cellMenu.value = null;
  headerMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 360)),
    colIndex,
  };
}

function closeMenus() {
  cellMenu.value = null;
  headerMenu.value = null;
}

function hideColumn() {
  const menu = headerMenu.value;
  if (!menu) return;
  if (visibleColumns.value.length > 1) {
    hiddenColumns.add(menu.colIndex);
  }
  closeMenus();
}

function showOnlyColumn() {
  const menu = headerMenu.value;
  if (!menu) return;
  hiddenColumns.clear();
  props.result?.columns.forEach((_, index) => {
    if (index !== menu.colIndex) hiddenColumns.add(index);
  });
  closeMenus();
}

function showAllColumns() {
  hiddenColumns.clear();
  autoHideMode.value = "off";
  autoExceptions.clear();
  closeMenus();
}

const hiddenColumnTotal = computed(
  () => new Set([...hiddenColumns, ...autoHiddenIndices.value]).size,
);

function toggleVisibilityColumn(key: string, visible: boolean) {
  const column = visibilityColumns.value.find((item) => item.key === key);
  if (!column) return;
  if (visible) {
    hiddenColumns.delete(column.index);
    autoExceptions.add(column.index);
  } else if (
    autoHiddenIndices.value.has(column.index) ||
    autoExceptions.has(column.index)
  ) {
    autoExceptions.delete(column.index);
  } else {
    hiddenColumns.add(column.index);
  }
}

function setAutoHideMode(mode: AutoHideMode) {
  autoHideMode.value = mode;
  autoExceptions.clear();
}

function resetHeaderWidth() {
  if (headerMenu.value) {
    delete columnWidths[headerMenu.value.colIndex];
  }
  closeMenus();
}

function copyHeaderName() {
  const menu = headerMenu.value;
  const name = menu ? props.result?.columns[menu.colIndex] : undefined;
  if (name) {
    void writeClipboard(name, "已复制列名");
  } else {
    closeMenus();
  }
}

function onGlobalClick() {
  closeMenus();
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  const element = target instanceof Element ? target : null;
  if (!element) return false;
  return Boolean(
    element.closest(
      'input, textarea, select, [contenteditable="true"], .cm-editor',
    ),
  );
}

function onGlobalKey(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (findOpen.value) {
      closeSearch();
      return;
    }
    if (detailOpen.value && !valueViewer.value) {
      detailOpen.value = false;
      return;
    }
    closeMenus();
    return;
  }
  if (
    !(event.metaKey || event.ctrlKey) ||
    event.key.toLowerCase() !== "f"
  ) {
    return;
  }
  const panel = scrollRef.value?.closest(".result-panel");
  const target = event.target;
  if (!panel || !(target instanceof Node) || !panel.contains(target)) {
    return;
  }
  const element = target instanceof Element ? target : null;
  if (element?.closest(".grid-findbar")) {
    event.preventDefault();
    void focusSearch();
    return;
  }
  if (element?.closest('.modal-backdrop, [role="dialog"]')) return;
  if (isTextEntryTarget(target)) return;
  event.preventDefault();
  void focusSearch();
}

function flashToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 1200);
}

async function writeClipboard(text: string, message: string) {
  try {
    await copyToClipboard(text);
    flashToast(message);
  } catch {
    flashToast("复制失败");
  }
  closeMenus();
}

function menuCellValue(): unknown {
  const menu = cellMenu.value;
  if (!menu) return null;
  return sortedRows.value[menu.rowIndex]?.[menu.colIndex];
}

function copyCell() {
  void writeClipboard(copyValue(menuCellValue()), "已复制单元格");
}

function copyDisplayCell() {
  const menu = cellMenu.value;
  if (!menu) return;
  const value = sortedRows.value[menu.rowIndex]?.[menu.colIndex];
  void writeClipboard(
    displayCell(value, menu.colIndex).text,
    "已复制显示值（原始值仍保留在数据库和导出中）",
  );
}

function copyRow() {
  const menu = cellMenu.value;
  const row = menu ? sortedRows.value[menu.rowIndex] : undefined;
  if (!row) return;
  const text = visibleColumns.value
    .map((column) => tsvValue(row[column.index]))
    .join("\t");
  void writeClipboard(text, "已复制整行");
}

function copyColumnName() {
  const menu = cellMenu.value;
  const name = menu ? props.result?.columns[menu.colIndex] : undefined;
  if (!name) return;
  void writeClipboard(name, "已复制列名");
}

function copyAsJson() {
  if (jsonDisabledReason.value) return;
  const menu = cellMenu.value;
  const row = menu ? sortedRows.value[menu.rowIndex] : undefined;
  if (!row) {
    closeMenus();
    return;
  }
  const columns = visibleColumns.value;
  const text = buildRowJson(
    columns.map((column) => column.name),
    columns.map((column) => row[column.index] ?? null),
  );
  const message = hiddenColumns.size
    ? `已复制 JSON（仅可见列，已隐藏 ${hiddenColumns.size} 列）`
    : "已复制 JSON";
  void writeClipboard(text, message);
}

function buildExportSnapshot(): ResultExportSnapshot | null {
  const result = props.result;
  if (!result || !result.rows.length) return null;
  const columns = visibleColumns.value;
  if (!columns.length) return null;
  const activeSort = sortIndex.value;
  return {
    columns: columns.map((column) => column.name),
    rows: sortedRows.value.map((row) =>
      columns.map((column) => row[column.index] ?? null),
    ),
    sort:
      activeSort === null
        ? null
        : {
            column: result.columns[activeSort] ?? "",
            direction: sortDirection.value,
            columnHidden: hiddenColumns.has(activeSort),
          },
    hiddenCount: hiddenColumns.size,
    sourceRowCount: result.rows.length,
    truncated: result.truncated,
    filteredOut: Math.max(0, result.rows.length - filteredRows.value.length),
    filterSummary: filterSummary.value,
  };
}

function requestExport() {
  if (props.running) return;
  const snapshot = buildExportSnapshot();
  if (!snapshot) return;
  closeMenus();
  emit("export-request", snapshot);
}

function copyAll() {
  const snapshot = buildExportSnapshot();
  if (!snapshot) return;
  const header = snapshot.columns.map(tsvValue).join("\t");
  const body = snapshot.rows.map((row) =>
    row.map((value) => tsvValue(value)).join("\t"),
  );
  void writeClipboard([header, ...body].join("\n"), "已复制全部结果");
}

watch(
  () => props.result,
  (result) => {
    const nextNames = result?.columns ?? [];
    columnOrder.value = reorderByName(
      columnOrder.value,
      lastColumnNames.value,
      nextNames,
    );
    lastColumnNames.value = [...nextNames];
    cancelColumnDrag();
    const previousFrozen = [...frozenNames.value];
    stopResize();
    closeMenus();
    hiddenColumns.clear();
    autoHideMode.value = "off";
    autoExceptions.clear();
    columnPanelOpen.value = false;
    toast.value = "";
    sortIndex.value = null;
    sortDirection.value = "asc";
    findQuery.value = "";
    findTerm.value = "";
    findIndex.value = 0;
    window.clearTimeout(findTimer);
    valueViewer.value = null;
    detailOpen.value = false;
    detailRowIndex.value = 0;
    filterOpen.value = false;
    filterConditions.value = [];
    filterCaseSensitive.value = false;
    selection.value = null;
    activeCell.value = null;
    for (const key of Object.keys(columnWidths)) {
      delete columnWidths[Number(key)];
    }
    defaultWidths.value = result
      ? result.columns.map((_, index) => estimateColumnWidth(result, index))
      : [];
    if (
      result &&
      previousFrozen.length &&
      previousFrozen.length < result.columns.length &&
      previousFrozen.every(
        (name, index) => result.columns[index] === name,
      )
    ) {
      frozenIndices.value = previousFrozen.map((_, index) => index);
      frozenNames.value = previousFrozen;
    } else {
      frozenIndices.value = [];
      frozenNames.value = [];
    }
    scrollToTop();
  },
  { immediate: true },
);

watch(
  () => hiddenColumnTotal.value,
  (count) => emit("hidden-change", count),
  { immediate: true },
);

defineExpose({
  showAllColumns,
  requestExport,
  focusSearch,
  toggleFilter,
  openFilter,
  clearSelection,
  getChartData: buildChartData,
});

onMounted(() => {
  window.addEventListener("click", onGlobalClick);
  window.addEventListener("keydown", onGlobalKey);
});

onBeforeUnmount(() => {
  cleanupColumnDrag();
  stopResize();
  window.clearTimeout(findTimer);
  window.clearTimeout(toastTimer);
  window.removeEventListener("click", onGlobalClick);
  window.removeEventListener("keydown", onGlobalKey);
});
</script>

<template>
  <div class="result-grid-shell">
    <ResultFilterBar
      v-if="filterOpen"
      :conditions="filterConditions"
      :columns="filterBarColumns"
      :case-sensitive="filterCaseSensitive"
      :filtered-count="filteredRows.length"
      :total-count="result?.rows.length ?? 0"
      :truncated="Boolean(result?.truncated)"
      @update:condition="updateFilterCondition"
      @update:case-sensitive="filterCaseSensitive = $event"
      @add="addFilterCondition()"
      @remove="removeFilterCondition"
      @clear="clearFilter"
      @close="filterOpen = false"
    />
    <GridFindBar
      v-if="findOpen"
      ref="findBar"
      :query="findQuery"
      :case-sensitive="findCaseSensitive"
      :match-count="findMatches.length"
      :row-count="findRowCount"
      :current="findIndex"
      :scope-hint="findScopeHint"
      placeholder="查找当前结果…"
      @update:query="findQuery = $event"
      @update:case-sensitive="findCaseSensitive = $event"
      @next="nextMatch"
      @prev="previousMatch"
      @close="closeSearch"
    />
    <div v-if="result" class="result-grid-column-tools">
      <button
        class="button ghost compact"
        :title="`手动隐藏 ${hiddenColumns.size} 列 · 自动隐藏 ${autoHiddenIndices.size} 列`"
        @click="columnPanelOpen = true"
      >
        <Icon name="list" :size="12" />
        列
        <span v-if="hiddenColumnTotal" class="result-filter-badge">
          {{ hiddenColumnTotal }}
        </span>
      </button>
    </div>
    <ColumnVisibilityPanel
      :open="columnPanelOpen"
      :columns="visibilityColumns"
      :manual-hidden="manualHiddenKeys"
      :auto-hidden="
        visibilityColumns
          .filter((column) => autoHiddenIndices.has(column.index))
          .map((column) => column.key)
      "
      :auto-mode="autoHideMode"
      :has-data="Boolean(result?.rows.length)"
      :row-count="result?.rows.length ?? 0"
      row-label="基于已加载结果，不含本地筛选"
      @close="columnPanelOpen = false"
      @toggle="toggleVisibilityColumn"
      @show-all="showAllColumns"
      @set-mode="setAutoHideMode"
    />
    <div
      ref="scrollRef"
      class="result-grid-wrap"
      tabindex="0"
      @keydown="onGridKeydown"
    >
    <div v-if="running" class="panel-empty">执行中…</div>
    <div v-else-if="!result" class="panel-empty">
      运行 SQL 后在这里查看结果
    </div>
    <div v-else-if="!result.rows.length" class="panel-empty">
      <template v-if="result.isQuery">查询成功，但没有返回行</template>
      <template v-else>
        语句执行成功，影响 {{ result.rowsAffected }} 行
      </template>
      <div v-if="result.truncated">结果已截断</div>
    </div>
    <table
      v-else
      class="result-grid resizable-grid"
      :class="{ virtualized: virtualEnabled }"
    >
      <colgroup>
        <col style="width: 46px" />
        <col
          v-for="column in visibleColumns"
          :key="column.index"
          :style="{ width: columnWidth(column.index) + 'px' }"
        />
      </colgroup>
      <thead>
        <tr>
          <th class="row-index">#</th>
          <th
            v-for="column in visibleColumns"
            :key="column.index"
            :data-col-index="column.index"
            class="sortable"
            :class="{
              'frozen-head': frozenIndices.includes(column.index),
              'frozen-boundary': column.index === lastFrozenIndex,
              'col-dragging':
                columnDrag?.index === column.index && columnDrag.moved,
              'col-drop-before':
                columnDrag?.target === column.index && !columnDrag.after,
              'col-drop-after':
                columnDrag?.target === column.index && columnDrag.after,
            }"
            :style="
              frozenLeft[column.index] !== undefined
                ? { left: `${frozenLeft[column.index]}px` }
                : undefined
            "
            tabindex="0"
            :title="column.name"
            :aria-sort="
              sortIndex === column.index
                ? sortDirection === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'
            "
            @mousedown="startColumnDrag($event, column.index)"
            @click="handleHeaderClick(column.index)"
            @keydown.enter.prevent="toggleSort(column.index)"
            @keydown.space.prevent="toggleSort(column.index)"
            @contextmenu.prevent="openHeaderMenu($event, column.index)"
          >
            <span class="result-head-label">
              <span class="result-head-text">{{ column.name }}</span>
              <span
                v-if="ruleForColumn(column.index)"
                class="column-format-badge"
                role="img"
                aria-label="已应用显示格式"
                :title="formatSummaryForColumn(column.index)"
              >fx</span>
              <span
                v-if="column.index === lastFrozenIndex"
                class="frozen-pin"
                role="img"
                :aria-label="`已冻结 ${frozenColumns.length} 列`"
                :title="`已冻结 ${frozenColumns.length} 列`"
              >
                <Icon name="pin" :size="11" />
              </span>
              <Icon
                v-if="sortIndex === column.index"
                :name="sortDirection === 'asc' ? 'chevron-up' : 'chevron-down'"
                :size="12"
                class="sort-icon"
              />
            </span>
            <span
              class="col-resizer"
              title="拖拽调整列宽，双击恢复自动宽度"
              @mousedown="startResize($event, column.index)"
              @dblclick.stop="resetColumnWidth(column.index)"
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
            :colspan="visibleColumns.length + 1"
            :style="{ height: `${virtualPaddingTop}px` }"
          ></td>
        </tr>
        <tr
          v-for="entry in virtualRows"
          :key="entry.index"
          class="data-row"
        >
          <td class="row-index">{{ entry.index + 1 }}</td>
          <td
            v-for="column in visibleColumns"
            :key="column.index"
            :class="{
              null: isNull(entry.row[column.index]),
              num: typeof entry.row[column.index] === 'number',
              'menu-target':
                cellMenu?.rowIndex === entry.index &&
                cellMenu?.colIndex === column.index,
              'grid-match-active':
                currentMatch?.rowIndex === entry.index &&
                currentMatch?.columnIndex === column.index,
              'frozen-cell': frozenIndices.includes(column.index),
              'frozen-boundary': column.index === lastFrozenIndex,
              'grid-selected': isGridSelected(entry.index, column.index),
              'grid-active-cell': isActiveCell(entry.index, column.index),
              'format-cell-warning': Boolean(
                formatWarningForColumn(entry.row[column.index], column.index),
              ),
            }"
            :style="
              frozenLeft[column.index] !== undefined
                ? { left: `${frozenLeft[column.index]}px` }
                : undefined
            "
            :title="cellTitle(entry.row[column.index], column.index)"
            tabindex="-1"
            :data-cell="`${entry.index}:${column.index}`"
            @mousedown="onCellMouseDown($event, entry.index, column.index)"
            @mouseenter="onCellMouseEnter(entry.index, column.index)"
            @dblclick="onCellDoubleClick(entry.index, column.index)"
            @contextmenu.prevent="
              openCellMenu($event, entry.index, column.index)
            "
          >
            <template v-if="highlightFor(entry.index, column.index)">
              <span
                v-for="(segment, segmentIndex) in highlightFor(
                  entry.index,
                  column.index,
                ) ?? []"
                :key="segmentIndex"
                :class="{ 'grid-match': segment.match }"
              >{{ segment.text }}</span>
            </template>
            <template v-else>
              {{ formatCellForColumn(entry.row[column.index], column.index) }}
            </template>
          </td>
        </tr>
        <tr
          v-if="virtualEnabled && virtualPaddingBottom > 0"
          class="virtual-spacer"
          aria-hidden="true"
        >
          <td
            :colspan="visibleColumns.length + 1"
            :style="{ height: `${virtualPaddingBottom}px` }"
          ></td>
        </tr>
      </tbody>
    </table>
    <div v-if="result?.truncated" class="panel-empty">
      结果超过限制，仅显示前 {{ result?.rows.length ?? 0 }} 行
    </div>

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>

    <div
      v-if="cellMenu"
      class="context-menu"
      :style="{ left: cellMenu.x + 'px', top: cellMenu.y + 'px' }"
      @click.stop
    >
      <button class="context-menu-item" @click="openValueFromMenu">
        <Icon name="view" :size="14" />
        查看值…
      </button>
      <button
        v-if="selectionSummaryVisible"
        class="context-menu-item"
        @click="copySelection(false)"
      >
        <Icon name="copy" :size="14" />
        复制选区
      </button>
      <button
        v-if="selectionSummaryVisible"
        class="context-menu-item"
        @click="copySelection(true)"
      >
        <Icon name="copy" :size="14" />
        复制选区（含列名）
      </button>
      <button class="context-menu-item" @click="openDetailFromMenu">
        <Icon name="list" :size="14" />
        查看行详情…
      </button>
      <button
        class="context-menu-item"
        :disabled="filterConditions.length >= RESULT_FILTER_LIMIT"
        @click="filterByMenuValue"
      >
        <Icon name="filter" :size="14" />
        按此值筛选
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyCell">
        <Icon name="copy" :size="14" />
        复制单元格
      </button>
      <button
        v-if="ruleForColumn(cellMenu.colIndex)"
        class="context-menu-item"
        title="只复制格式化后的显示文本；导出与整行 JSON 仍用原始值"
        @click="copyDisplayCell"
      >
        <Icon name="copy" :size="14" />
        复制显示值
      </button>
      <button class="context-menu-item" @click="copyRow">
        <Icon name="copy" :size="14" />
        复制整行
      </button>
      <button class="context-menu-item" @click="copyColumnName">
        <Icon name="copy" :size="14" />
        复制列名
      </button>
      <button
        class="context-menu-item"
        :disabled="Boolean(jsonDisabledReason)"
        :title="jsonDisabledReason || '复制为 JSON'"
        @click="copyAsJson"
      >
        <Icon name="copy" :size="14" />
        复制为 JSON
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyAll">
        <Icon name="copy" :size="14" />
        复制全部结果
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        :disabled="running || !result?.rows.length"
        @click="requestExport"
      >
        <Icon name="download" :size="14" />
        导出结果…
      </button>
    </div>

    <div
      v-if="headerMenu"
      class="context-menu"
      :style="{ left: headerMenu.x + 'px', top: headerMenu.y + 'px' }"
      @click.stop
    >
      <button class="context-menu-item" @click="copyHeaderName">
        <Icon name="copy" :size="14" />
        复制列名
      </button>
      <button
        class="context-menu-item"
        :disabled="!sortedRows.length"
        @click="selectColumn(headerMenu.colIndex)"
      >
        <Icon name="table" :size="14" />
        统计此列
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        @click="openColumnFormat(headerMenu.colIndex)"
      >
        <Icon name="edit" :size="14" />
        显示格式…
      </button>
      <button
        v-if="ruleForColumn(headerMenu.colIndex)"
        class="context-menu-item"
        @click="clearColumnFormat(headerMenu.colIndex)"
      >
        <Icon name="close" :size="14" />
        清除此列格式
      </button>
      <div class="context-separator"></div>
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
      <button
        class="context-menu-item"
        :disabled="filterConditions.length >= RESULT_FILTER_LIMIT"
        :title="
          filterConditions.length >= RESULT_FILTER_LIMIT
            ? '最多 5 个条件'
            : '按此列筛选…'
        "
        @click="filterByColumn(headerMenu.colIndex)"
      >
        <Icon name="filter" :size="14" />
        按此列筛选…
      </button>
      <button class="context-menu-item" @click="showOnlyColumn">
        <Icon name="view" :size="14" />
        只看此列
      </button>
      <button
        class="context-menu-item"
        :disabled="visibleColumns.length <= 1"
        @click="hideColumn"
      >
        <Icon name="close" :size="14" />
        隐藏此列
      </button>
      <button
        class="context-menu-item"
        :disabled="!hiddenColumns.size"
        @click="showAllColumns"
      >
        <Icon name="refresh" :size="14" />
        显示所有列
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="resetHeaderWidth">
        <Icon name="structure" :size="14" />
        恢复自动列宽
      </button>
      <div class="context-separator"></div>
      <button
        class="context-menu-item"
        :disabled="running || !result?.rows.length"
        @click="requestExport"
      >
        <Icon name="download" :size="14" />
        导出结果…
      </button>
    </div>

    <ColumnFormatDialog
      :open="Boolean(formatDialog)"
      :column-name="formatDialog ? (result?.columns[formatDialog.colIndex] ?? '') : ''"
      :samples="formatSamples"
      :initial="formatDialog ? ruleForColumn(formatDialog.colIndex) : null"
      @cancel="formatDialog = null"
      @apply="applyColumnFormat"
    />

    <CellValueDialog
      :open="Boolean(valueViewer)"
      :column="viewerColumn"
      :row-label="valueViewer ? `第 ${valueViewer.rowIndex + 1} 行` : ''"
      :value="viewerValue"
      @close="closeValueViewer"
    />
    </div>

    <div
      v-if="selectionSummaryVisible && selectionStats"
      class="grid-selection-summary"
      aria-live="polite"
      :title="selectionTitles"
    >
      <span class="selection-main">
        {{ selectionRowCount }} 行 × {{ selectionColumnCount }} 列 ·
        {{ selectionStats.cells }} 个单元格
      </span>
      <span>非空 {{ selectionStats.nonNull }}</span>
      <span>NULL {{ selectionStats.nulls }}</span>
      <span>去重 {{ selectionStats.distinct }}</span>
      <template v-if="selectionStats.numericCount">
        <span class="selection-numeric">
          数值 {{ selectionStats.numericCount }} · 求和
          {{ formatStatValue(selectionStats.sum) }} · 平均
          {{ formatStatValue(selectionStats.average) }} · 最小
          {{ formatStatValue(selectionStats.min) }} · 最大
          {{ formatStatValue(selectionStats.max) }}
        </span>
      </template>
      <span class="spacer"></span>
      <button class="button ghost compact" @click="copySelection(false)">
        复制选区
      </button>
      <button
        class="icon-button"
        title="清空选区（Esc）"
        aria-label="清空选区"
        @click="clearSelection"
      >
        <Icon name="close" :size="12" />
      </button>
    </div>

    <RowDetailPanel
      :open="detailOpen"
      title="查询结果"
      :row-label="detailRowLabel"
      :fields="detailFields"
      :editable="false"
      read-only-reason="查询结果只读"
      :can-previous="detailRowIndex > 0"
      :can-next="detailRowIndex < sortedRows.length - 1"
      @close="closeRowDetail"
      @previous="moveRowDetail(-1)"
      @next="moveRowDetail(1)"
      @view-value="viewDetailValue"
      @copy-value="copyDetailValue"
      @copy-row-json="copyDetailRowJson"
    />
  </div>
</template>
