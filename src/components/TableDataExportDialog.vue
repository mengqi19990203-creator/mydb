<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  rowsToCsv,
  rowsToJson,
  rowsToSql,
  tableExportExtension,
  tableExportTimestamp,
  type TableExportFormat,
  type TableExportRange,
} from "../lib/table-export";
import type { Driver, TableSort } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  table: string;
  driver: Driver;
  columns: string[];
  visibleColumns: string[];
  pageRows: unknown[][];
  selectedRows: unknown[][];
  total: number;
  filter: string;
  sorts: TableSort[];
  hasPendingChanges: boolean;
  scopeLabel: string;
}>();

const emit = defineEmits<{
  close: [];
  exported: [path: string, rows: number, pending: boolean];
}>();

const format = ref<TableExportFormat>("csv");
const range = ref<TableExportRange>("page");
const withHeader = ref(true);
const includeHidden = ref(false);
const snapshotVisibleColumns = ref<string[]>([]);
const targetTable = ref("");
const saving = ref(false);
const error = ref("");
const progressRows = ref(0);
const progressBytes = ref(0);
const operationId = ref("");
const completed = ref<{ path: string; rows: number } | null>(null);
let unlisten: UnlistenFn | undefined;

const hiddenColumnCount = computed(
  () =>
    props.columns.filter(
      (column) => !snapshotVisibleColumns.value.includes(column),
    ).length,
);
const activeColumns = computed(() => {
  if (format.value === "sql" && includeHidden.value) {
    return [...props.columns];
  }
  const visible = snapshotVisibleColumns.value.filter((column) =>
    props.columns.includes(column),
  );
  return visible.length ? visible : [...props.columns];
});
const activeIndices = computed(() =>
  activeColumns.value.map((column) => props.columns.indexOf(column)),
);
function project(rows: unknown[][]): unknown[][] {
  return rows.map((row) =>
    activeIndices.value.map((index) =>
      index >= 0 ? (row[index] ?? null) : null,
    ),
  );
}
const pageRowsView = computed(() => project(props.pageRows));
const selectedRowsView = computed(() => project(props.selectedRows));
const selectedAvailable = computed(() => props.selectedRows.length > 0);
const allReason = computed(() =>
  props.hasPendingChanges
    ? "存在未提交修改，请先提交或放弃，或选择当前页"
    : "",
);
const rowCount = computed(() =>
  range.value === "page"
    ? pageRowsView.value.length
    : range.value === "selected"
      ? selectedRowsView.value.length
      : props.total,
);
const sqlMissingHidden = computed(
  () =>
    format.value === "sql" &&
    hiddenColumnCount.value > 0 &&
    !includeHidden.value,
);
const formatIssue = computed(() => {
  if (range.value === "all") return allReason.value;
  if (!rowCount.value) return "当前范围没有可导出的行";
  if (format.value === "xlsx" && rowCount.value > 200_000) {
    return "XLSX 单文件最多 200,000 行，请改用 CSV";
  }
  if (format.value === "sql" && !targetTable.value.trim()) {
    return "SQL INSERT 需要目标表名";
  }
  return "";
});
const canExport = computed(
  () => !saving.value && !formatIssue.value && rowCount.value > 0,
);
const summary = computed(() => {
  const scopes: Record<TableExportRange, string> = {
    page: "当前页（含待提交显示值）",
    selected: "当前页选中行（含待提交显示值）",
    all: "全部匹配（数据库当前值）",
  };
  return scopes[range.value];
});
const sortSummary = computed(() =>
  props.sorts.length
    ? props.sorts
        .map((sort) => `${sort.column} ${sort.direction.toUpperCase()}`)
        .join("，")
    : "原始顺序",
);

function reset() {
  format.value = "csv";
  range.value = "page";
  withHeader.value = true;
  includeHidden.value = false;
  snapshotVisibleColumns.value = [...props.visibleColumns];
  targetTable.value = props.table;
  saving.value = false;
  error.value = "";
  progressRows.value = 0;
  progressBytes.value = 0;
  operationId.value = "";
  completed.value = null;
}

watch(
  () => props.open,
  (open) => {
    if (open) reset();
  },
  { immediate: true },
);

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function defaultFileName(): string {
  return `${props.table}-${tableExportTimestamp()}.${tableExportExtension(
    format.value,
  )}`;
}

async function exportRows() {
  if (!canExport.value || saving.value) return;
  const chosen = await saveDialog({
    defaultPath: defaultFileName(),
    filters: [
      {
        name:
          format.value === "xlsx"
            ? "Excel 工作簿"
            : format.value.toUpperCase(),
        extensions: [tableExportExtension(format.value)],
      },
    ],
  });
  if (typeof chosen !== "string") return;
  saving.value = true;
  error.value = "";
  completed.value = null;
  progressRows.value = 0;
  progressBytes.value = 0;
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `table-export-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  operationId.value = id;
  try {
    let rows = rowCount.value;
    if (range.value === "all") {
      rows = await api.exportTableRows({
        connectionId: props.connectionId,
        table: props.table,
        format: format.value,
        path: chosen,
        filter: props.filter || null,
        sorts: props.sorts,
        limit: null,
        withHeader: withHeader.value,
        targetTable: targetTable.value.trim() || props.table,
        columns:
          activeColumns.value.length === props.columns.length
            ? null
            : [...activeColumns.value],
        operationId: id,
      });
    } else {
      const data =
        range.value === "selected"
          ? selectedRowsView.value
          : pageRowsView.value;
      if (format.value === "xlsx") {
        const summary = await api.exportQueryResultXlsx(
          activeColumns.value,
          data as unknown[][],
          chosen,
          props.table,
          withHeader.value,
        );
        rows = summary.rows;
      } else {
        const contents =
          format.value === "csv"
            ? rowsToCsv(data, props.columns, withHeader.value)
            : format.value === "json"
              ? rowsToJson(data, props.columns, props.table)
              : rowsToSql(
                  data,
                  props.columns,
                  targetTable.value.trim() || props.table,
                  props.driver,
                );
        await api.writeTextFile(chosen, contents);
        rows = data.length;
      }
      completed.value = { path: chosen, rows };
      emit("exported", chosen, rows, true);
    }
    completed.value = { path: chosen, rows };
    if (range.value === "all") {
      emit("exported", chosen, rows, false);
    }
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
    operationId.value = "";
  }
}

async function cancelExport() {
  if (!operationId.value) return;
  try {
    await api.cancelExport(operationId.value);
  } catch (reason) {
    error.value = String(reason);
  }
}

function close() {
  if (saving.value) return;
  emit("close");
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    unlisten?.();
    unlisten = await listen<{
      operationId?: string;
      rows: number;
      bytes: number;
      phase: string;
    }>("table-export:progress", (event) => {
      if (!operationId.value) return;
      if (event.payload.operationId !== operationId.value) return;
      progressRows.value = event.payload.rows;
      progressBytes.value = event.payload.bytes;
    });
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="close">
    <div class="modal prompt-modal table-export-modal" role="dialog" aria-modal="true" aria-label="导出表数据">
      <header class="data-toolbar table-export-head">
        <div>
          <h2>导出表数据 · {{ table }}</h2>
          <p class="hint">
            {{ scopeLabel }} ·
            {{
              format === "sql" && includeHidden
                ? "按全部列导出"
                : "按当前可见列导出"
            }}<span v-if="hiddenColumnCount && !(format === 'sql' && includeHidden)"
              >（{{ hiddenColumnCount }} 列已隐藏）</span
            >
            · 排序：{{ sortSummary }}
          </p>
        </div>
        <button class="icon-button" title="关闭" :disabled="saving" @click="close">
          <Icon name="close" />
        </button>
      </header>

      <p v-if="hasPendingChanges" class="data-message warning import-message">
        存在未提交修改；当前页 / 选中行导出会包含显示值，全部匹配只能导出数据库当前值。
      </p>
      <p v-if="error" class="data-message error import-message">{{ error }}</p>

      <div class="table-export-body">
        <label class="import-field">
          范围
          <select v-model="range" :disabled="saving">
            <option value="page">当前页（{{ pageRows.length }} 行）</option>
            <option value="selected" :disabled="!selectedAvailable">
              当前页选中行（{{ selectedRows.length }} 行）
            </option>
            <option value="all" :disabled="Boolean(allReason)">
              全部匹配（{{ total }} 行）
            </option>
          </select>
        </label>
        <label class="import-field">
          格式
          <select v-model="format" :disabled="saving">
            <option value="csv">CSV</option>
            <option value="json">JSON</option>
            <option value="xlsx">XLSX</option>
            <option value="sql">SQL INSERT</option>
          </select>
        </label>
        <label v-if="format === 'csv'" class="import-check">
          <input v-model="withHeader" type="checkbox" :disabled="saving" />
          包含表头
        </label>
        <label v-if="format === 'sql'" class="import-field">
          目标表
          <input v-model="targetTable" :disabled="saving" placeholder="schema.table" />
        </label>
        <label v-if="format === 'sql' && hiddenColumnCount" class="import-check">
          <input v-model="includeHidden" type="checkbox" :disabled="saving" />
          包含隐藏列
        </label>
        <span class="hint">
          {{ summary }} · 将导出 {{ rowCount }} 行 ×
          {{ activeColumns.length }} 列
        </span>
        <span v-if="range === 'all' && filter" class="hint" :title="filter">
          WHERE {{ filter }}
        </span>
      </div>

      <p v-if="allReason" class="hint">{{ allReason }}</p>
      <p v-if="sqlMissingHidden" class="data-message warning import-message">
        当前有 {{ hiddenColumnCount }} 个隐藏列，SQL INSERT 只会包含可见列；如需完整字段，请勾选「包含隐藏列」。
      </p>

      <div v-if="saving || progressRows" class="table-export-progress">
        <span>
          已读取 {{ progressRows.toLocaleString("zh-CN") }} 行
          <template v-if="progressBytes">
            · {{ formatBytes(progressBytes) }}
          </template>
        </span>
        <button class="button ghost compact" :disabled="!saving" @click="cancelExport">
          取消
        </button>
      </div>

      <p v-if="completed" class="data-message success">
        已导出 {{ completed.rows.toLocaleString("zh-CN") }} 行到 {{ completed.path }}
      </p>

      <footer class="modal-actions">
        <span v-if="formatIssue" class="hint">{{ formatIssue }}</span>
        <span class="spacer"></span>
        <button class="button ghost" :disabled="saving" @click="close">关闭</button>
        <button
          class="button primary"
          :disabled="!canExport"
          :title="formatIssue || undefined"
          @click="exportRows"
        >
          {{ saving ? "导出中…" : "选择文件并导出" }}
        </button>
      </footer>
    </div>
  </div>
</template>
