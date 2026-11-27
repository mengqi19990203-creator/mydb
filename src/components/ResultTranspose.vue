<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import { formatGridCell } from "../lib/grid-search";
import {
  formatColumnKeys,
  formatDisplayValue,
  type ColumnFormatEntry,
} from "../lib/column-format";
import type { ChartData } from "../lib/result-chart";

const MAX_TRANSPOSE_ROWS = 500;
const MAX_TRANSPOSE_COLUMNS = 100;

const props = defineProps<{
  data: ChartData;
  title: string;
  formats?: ColumnFormatEntry[];
}>();

interface TransposeMenu {
  x: number;
  y: number;
  rowIndex: number | null;
  columnIndex: number | null;
}

const menu = ref<TransposeMenu | null>(null);
const toast = ref("");
let toastTimer = 0;

const rows = computed(() => props.data.rows.slice(0, MAX_TRANSPOSE_ROWS));
const columns = computed(() =>
  props.data.visibleColumns.slice(0, MAX_TRANSPOSE_COLUMNS),
);
const rowTruncated = computed(
  () => props.data.rows.length > MAX_TRANSPOSE_ROWS,
);
const columnTruncated = computed(
  () => props.data.visibleColumns.length > MAX_TRANSPOSE_COLUMNS,
);

function value(rowIndex: number, columnIndex: number): unknown {
  return rows.value[rowIndex]?.[columnIndex] ?? null;
}

const columnKeys = computed(() =>
  formatColumnKeys(props.data.columns.map((name) => ({ name }))),
);

function ruleForColumn(columnIndex: number) {
  const key = columnKeys.value[columnIndex] ?? "";
  return props.formats?.find((entry) => entry.columnKey === key)?.rule ?? null;
}

function displayValue(rowIndex: number, columnIndex: number): string {
  const valueHere = value(rowIndex, columnIndex);
  const rule = ruleForColumn(columnIndex);
  return rule ? formatDisplayValue(valueHere, rule).text : formatGridCell(valueHere);
}

function showToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 1800);
}

function openCellMenu(
  event: MouseEvent,
  rowIndex: number,
  columnIndex: number,
) {
  event.preventDefault();
  menu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 200)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 120)),
    rowIndex,
    columnIndex,
  };
}

function openRowMenu(event: MouseEvent, columnIndex: number) {
  event.preventDefault();
  menu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 200)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 120)),
    rowIndex: null,
    columnIndex,
  };
}

function closeMenu() {
  menu.value = null;
}

async function copyValue() {
  const current = menu.value;
  closeMenu();
  if (!current || current.rowIndex === null || current.columnIndex === null) {
    return;
  }
  const column = columns.value[current.columnIndex];
  if (!column) return;
  try {
    await writeClipboard(
      formatGridCell(value(current.rowIndex, column.index)),
    );
    showToast("已复制值");
  } catch (reason) {
    showToast(`复制失败：${reason}`);
  }
}

async function copyDisplayValue() {
  const current = menu.value;
  closeMenu();
  if (!current || current.rowIndex === null || current.columnIndex === null) return;
  const column = columns.value[current.columnIndex];
  if (!column) return;
  try {
    await writeClipboard(displayValue(current.rowIndex, column.index));
    showToast("已复制显示值");
  } catch (reason) {
    showToast(`复制失败：${reason}`);
  }
}

async function copyRow() {
  const current = menu.value;
  closeMenu();
  if (!current || current.columnIndex === null) return;
  const column = columns.value[current.columnIndex];
  if (!column) return;
  const text = rows.value
    .map((_, rowIndex) => formatGridCell(value(rowIndex, column.index)))
    .join("\t");
  try {
    await writeClipboard(text);
    showToast("已复制此字段行 TSV");
  } catch (reason) {
    showToast(`复制失败：${reason}`);
  }
}

async function copyAll() {
  const header = [
    "字段",
    ...rows.value.map((_, index) => `行 ${index + 1}`),
  ];
  const lines = [header.join("\t")];
  for (const column of columns.value) {
    lines.push(
      [
        column.name,
        ...rows.value.map((_, rowIndex) =>
          formatGridCell(value(rowIndex, column.index)),
        ),
      ].join("\t"),
    );
  }
  try {
    await writeClipboard(lines.join("\n"));
    showToast(`已复制 ${columns.value.length} × ${rows.value.length} 转置表`);
  } catch (reason) {
    showToast(`复制失败：${reason}`);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && menu.value) {
    event.preventDefault();
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener("click", closeMenu);
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.clearTimeout(toastTimer);
  window.removeEventListener("click", closeMenu);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="result-transpose">
    <div class="transpose-toolbar">
      <span class="hint">
        {{ title }} · {{ columns.length }} 个字段 × {{ rows.length }} 行
        （只读）
      </span>
      <span class="spacer"></span>
      <button class="button ghost compact" @click="copyAll">
        <Icon name="copy" :size="13" />
        复制转置表 TSV
      </button>
    </div>
    <p v-if="rowTruncated || columnTruncated" class="data-message warning">
      仅转置前
      {{ Math.min(rows.length, MAX_TRANSPOSE_ROWS) }} 行 ×
      {{ Math.min(columns.length, MAX_TRANSPOSE_COLUMNS) }} 列；请先筛选或隐藏列。
    </p>
    <div v-if="!columns.length || !rows.length" class="panel-empty">
      没有可转置的可见行或列。
    </div>
    <div v-else class="transpose-scroll">
      <table class="transpose-table">
        <thead>
          <tr>
            <th class="transpose-field-head">字段</th>
            <th
              v-for="(_, index) in rows"
              :key="index"
              class="mono transpose-row-head"
            >
              行 {{ index + 1 }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(column, columnIndex) in columns"
            :key="column.index"
            @contextmenu="openRowMenu($event, columnIndex)"
          >
            <td class="transpose-field-name" :title="column.name">
              {{ column.name }}
            </td>
            <td
              v-for="(_, rowIndex) in rows"
              :key="rowIndex"
              :class="{
                null: value(rowIndex, column.index) === null,
                num: typeof value(rowIndex, column.index) === 'number',
              }"
              :title="displayValue(rowIndex, column.index)"
              @contextmenu="openCellMenu($event, rowIndex, columnIndex)"
            >
              {{ displayValue(rowIndex, column.index) }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      v-if="menu"
      class="context-menu"
      role="menu"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @click.stop
    >
      <button
        v-if="menu.rowIndex !== null && menu.columnIndex !== null"
        class="context-menu-item"
        @click="copyValue"
      >
        <Icon name="copy" :size="14" />
        复制值
      </button>
      <button
        v-if="
          menu.rowIndex !== null &&
          menu.columnIndex !== null &&
          ruleForColumn(columns[menu.columnIndex]?.index ?? -1)
        "
        class="context-menu-item"
        @click="copyDisplayValue"
      >
        <Icon name="copy" :size="14" />
        复制显示值
      </button>
      <button
        v-if="menu.columnIndex !== null"
        class="context-menu-item"
        @click="copyRow"
      >
        <Icon name="copy" :size="14" />
        复制此字段行 TSV
      </button>
    </div>

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>
  </div>
</template>
