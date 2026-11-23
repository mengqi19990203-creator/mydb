<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import Icon from "./Icon.vue";
import { writeClipboard as copyToClipboard } from "../lib/clipboard";
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
}>();

const columnWidths = reactive<Record<number, number>>({});
const defaultWidths = ref<number[]>([]);
const cellMenu = ref<CellMenu | null>(null);
const headerMenu = ref<HeaderMenu | null>(null);
const hiddenColumns = reactive(new Set<number>());
const toast = ref("");
const sortIndex = ref<number | null>(null);
const sortDirection = ref<"asc" | "desc">("asc");
let resizeState: ResizeState | null = null;
let toastTimer = 0;

const visibleColumns = computed(() =>
  (props.result?.columns ?? [])
    .map((name, index) => ({ name, index }))
    .filter((column) => !hiddenColumns.has(column.index)),
);

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
    return;
  }
  if (sortDirection.value === "asc") {
    sortDirection.value = "desc";
    return;
  }
  sortIndex.value = null;
  sortDirection.value = "asc";
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
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 190)),
    rowIndex,
    colIndex,
  };
}

function openHeaderMenu(event: MouseEvent, colIndex: number) {
  event.preventDefault();
  cellMenu.value = null;
  headerMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 240)),
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

function copyAll() {
  if (!props.result) return;
  const columns = visibleColumns.value;
  const header = columns.map((column) => tsvValue(column.name)).join("\t");
  const body = sortedRows.value.map((row) =>
    columns.map((column) => tsvValue(row[column.index])).join("\t"),
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
    for (const key of Object.keys(columnWidths)) {
      delete columnWidths[Number(key)];
    }
    defaultWidths.value = result
      ? result.columns.map((_, index) => estimateColumnWidth(result, index))
      : [];
  },
  { immediate: true },
);

watch(
  () => hiddenColumns.size,
  (count) => emit("hidden-change", count),
  { immediate: true },
);

defineExpose({ showAllColumns });

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
  <div class="result-grid-wrap">
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
    <table v-else class="result-grid resizable-grid">
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
        <tr v-for="(row, rowIndex) in sortedRows" :key="rowIndex">
          <td class="row-index">{{ rowIndex + 1 }}</td>
          <td
            v-for="column in visibleColumns"
            :key="column.index"
            :class="{
              null: isNull(row[column.index]),
              num: typeof row[column.index] === 'number',
              'menu-target':
                cellMenu?.rowIndex === rowIndex &&
                cellMenu?.colIndex === column.index,
            }"
            :title="formatCell(row[column.index])"
            @contextmenu.prevent="
              openCellMenu($event, rowIndex, column.index)
            "
          >
            {{ formatCell(row[column.index]) }}
          </td>
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
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="copyAll">
        <Icon name="copy" :size="14" />
        复制全部结果
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
    </div>
  </div>
</template>
