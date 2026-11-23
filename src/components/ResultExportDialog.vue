<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../lib/api";
import {
  buildExportContent,
  columnNameIssue,
  exportExtension,
  exportFilename,
  type ResultExportFormat,
  type ResultExportSnapshot,
} from "../lib/result-export";
import type { Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  snapshot: ResultExportSnapshot | null;
  driver: Driver;
}>();

const emit = defineEmits<{
  close: [];
  exported: [path: string, rows: number];
}>();

const format = ref<ResultExportFormat>("csv");
const withHeader = ref(true);
const tableName = ref("");
const error = ref("");
const writing = ref(false);

const columnIssue = computed(() =>
  props.snapshot ? columnNameIssue(props.snapshot.columns) : "",
);
const rowsCount = computed(() => props.snapshot?.rows.length ?? 0);
const columnsCount = computed(() => props.snapshot?.columns.length ?? 0);
const sortLabel = computed(() => {
  const sort = props.snapshot?.sort;
  if (!sort) return "";
  const direction = sort.direction === "asc" ? "升序" : "降序";
  return `${sort.column} ${direction}${sort.columnHidden ? "（排序列已隐藏）" : ""}`;
});

const disabledReason = computed(() => {
  if (!props.snapshot) return "没有可导出的结果";
  if (!rowsCount.value) return "结果为空";
  if (writing.value) return "正在写入文件…";
  if (
    (format.value === "json" || format.value === "sql") &&
    columnIssue.value
  ) {
    return columnIssue.value;
  }
  if (format.value === "sql" && !tableName.value.trim()) {
    return "请填写目标表名";
  }
  return "";
});

const canExport = computed(() => !disabledReason.value);

const formatExtensions: Record<ResultExportFormat, string> = {
  csv: "CSV",
  json: "JSON",
  markdown: "Markdown",
  sql: "SQL",
};

function reset() {
  format.value = "csv";
  withHeader.value = true;
  tableName.value = "";
  error.value = "";
  writing.value = false;
}

function close() {
  if (writing.value) return;
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    close();
  }
}

async function submit() {
  if (!props.snapshot || !canExport.value) return;
  error.value = "";

  let content: string;
  try {
    content = buildExportContent(format.value, props.snapshot, {
      withHeader: withHeader.value,
      table: tableName.value.trim(),
      driver: props.driver,
    });
  } catch (reason) {
    error.value = String(reason);
    return;
  }

  let path: string | null;
  try {
    const selected = await saveDialog({
      defaultPath: exportFilename(format.value),
      filters: [
        {
          name: formatExtensions[format.value],
          extensions: [exportExtension(format.value)],
        },
      ],
    });
    path = typeof selected === "string" ? selected : null;
  } catch (reason) {
    error.value = String(reason);
    return;
  }
  if (!path) return;

  writing.value = true;
  try {
    await api.writeTextFile(path, content);
    emit("exported", path, props.snapshot.rows.length);
    emit("close");
  } catch (reason) {
    error.value = String(reason);
  } finally {
    writing.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="close">
    <div
      class="modal confirm-modal prompt-modal export-modal result-export-modal"
      role="dialog"
      aria-modal="true"
      aria-label="导出查询结果"
    >
      <h2>导出查询结果</h2>

      <p class="result-export-summary">
        当前结果：{{ rowsCount }} 行 · {{ columnsCount }} 列
        <span v-if="snapshot?.hiddenCount">
          （已隐藏 {{ snapshot.hiddenCount }} 列）
        </span>
        <span v-if="sortLabel"> · 排序：{{ sortLabel }}</span>
      </p>
      <p v-if="snapshot?.truncated" class="result-export-warning">
        当前结果仅 {{ rowsCount }} 行（已截断），导出和排序都只针对这些行
      </p>

      <div class="export-field">
        <span class="export-label">格式</span>
        <select v-model="format">
          <option value="csv">CSV</option>
          <option value="json" :disabled="Boolean(columnIssue)">
            JSON
          </option>
          <option value="markdown">Markdown</option>
          <option value="sql" :disabled="Boolean(columnIssue)">
            SQL INSERT
          </option>
        </select>
      </div>

      <div v-if="format === 'csv'" class="export-field">
        <span class="export-label">表头</span>
        <label class="export-check">
          <input v-model="withHeader" type="checkbox" />
          <span>包含表头行</span>
        </label>
      </div>

      <div v-if="format === 'sql'" class="export-field">
        <span class="export-label">目标表</span>
        <input
          v-model="tableName"
          type="text"
          placeholder="table 或 schema.table"
          spellcheck="false"
        />
      </div>

      <p v-if="format === 'json'" class="result-export-hint">
        导出为对象数组，保留数字 / 布尔 / null 类型。
      </p>
      <p v-if="format === 'sql'" class="result-export-hint">
        每行生成一条 INSERT 语句，不包含事务；不填表名或列名重复时不可导出。
      </p>
      <p v-if="columnIssue" class="result-export-warning">{{ columnIssue }}</p>

      <p v-if="error" class="message error">{{ error }}</p>

      <div class="confirm-actions">
        <button class="button ghost" :disabled="writing" @click="close">
          取消
        </button>
        <button
          class="button primary"
          :disabled="!canExport"
          :title="disabledReason || '选择保存位置并导出'"
          @click="submit"
        >
          {{ writing ? "写入中…" : "导出…" }}
        </button>
      </div>
    </div>
  </div>
</template>