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
import Icon from "./Icon.vue";
import { writeClipboard as copyToClipboard } from "../lib/clipboard";
import { columnNameIssue, type ResultExportSnapshot } from "../lib/result-export";
import { useVirtualRows } from "../lib/virtual-rows";
import { buildRowJson } from "../lib/row-copy";
import type { QueryResult } from "../lib/types";

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
}>();

const emit = defineEmits<{
  "hidden-change": [count: number];
  "export-request": [snapshot: ResultExportSnapshot];
}>();

const columnWidths = reactive<Record<number, number>>({});
const defaultWidths = ref<number[]>([]);
const cellMenu = ref<CellMenu | null>(null);
const headerMenu = ref<HeaderMenu | null>(null);
const hiddenColumns = reactive(new Set<number>());
const toast = ref("");
const sortIndex = ref<number | null>(null);
const sortDirection = ref<"asc" | "desc">("asc");
const valueViewer = ref<{
  rowIndex: number;
  colIndex: number;
} | null>(null);
const scrollRef = ref<HTMLElement | null>(null);
let resizeState: ResizeState | null = null;
let toastTimer = 0;

const visibleColumns = computed(() =>
  (props.result?.columns ?? [])
    .map((name, index) => ({ name, index }))
    .filter((column) => !hiddenColumns.has(column.index)),
);

const jsonDisabledReason = computed(() => {
  if (props.running) return "查询执行中";
  if (!props.result) return "没有结果";
  return columnNameIssue(visibleColumns.value.map((column) => column.name));
});

const sortedRows = computed<unknown[][]>(() => {
  const rows = props.result?.rows ?? [];
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

const {
  enabled: virtualEnabled,
  start: virtualStart,
  end: virtualEnd,
  paddingTop: virtualPaddingTop,
  paddingBottom: virtualPaddingBottom,
  scrollToTop,
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
  cellMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 260)),
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
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 280)),
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
  closeMenus();
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

function onGlobalKey(event: KeyboardEvent) {
  if (event.key === "Escape") closeMenus();
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
    stopResize();
    closeMenus();
    hiddenColumns.clear();
    toast.value = "";
    sortIndex.value = null;
    sortDirection.value = "asc";
    valueViewer.value = null;
    for (const key of Object.keys(columnWidths)) {
      delete columnWidths[Number(key)];
    }
    defaultWidths.value = result
      ? result.columns.map((_, index) => estimateColumnWidth(result, index))
      : [];
    scrollToTop();
  },
  { immediate: true },
);

watch(
  () => hiddenColumns.size,
  (count) => emit("hidden-change", count),
  { immediate: true },
);

defineExpose({ showAllColumns, requestExport });

onMounted(() => {
  window.addEventListener("click", onGlobalClick);
  window.addEventListener("keydown", onGlobalKey);
});

onBeforeUnmount(() => {
  stopResize();
  window.clearTimeout(toastTimer);
  window.removeEventListener("click", onGlobalClick);
  window.removeEventListener("keydown", onGlobalKey);
});
</script>

<template>
  <div ref="scrollRef" class="result-grid-wrap">
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
            class="sortable"
            tabindex="0"
            :title="column.name"
            :aria-sort="
              sortIndex === column.index
                ? sortDirection === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'
            "
            @click="toggleSort(column.index)"
            @keydown.enter.prevent="toggleSort(column.index)"
            @keydown.space.prevent="toggleSort(column.index)"
            @contextmenu.prevent="openHeaderMenu($event, column.index)"
          >
            <span class="result-head-label">
              <span class="result-head-text">{{ column.name }}</span>
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
            }"
            :title="formatCell(entry.row[column.index])"
            tabindex="-1"
            :data-cell="`${entry.index}:${column.index}`"
            @dblclick="onCellDoubleClick(entry.index, column.index)"
            @contextmenu.prevent="
              openCellMenu($event, entry.index, column.index)
            "
          >
            {{ formatCell(entry.row[column.index]) }}
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
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyCell">
        <Icon name="copy" :size="14" />
        复制单元格
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
      <div class="context-separator"></div>
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

    <CellValueDialog
      :open="Boolean(valueViewer)"
      :column="viewerColumn"
      :row-label="valueViewer ? `第 ${valueViewer.rowIndex + 1} 行` : ''"
      :value="viewerValue"
      @close="closeValueViewer"
    />
  </div>
</template>
