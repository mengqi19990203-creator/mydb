<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import ResizeHandle from "./ResizeHandle.vue";
import ResultExportDialog from "./ResultExportDialog.vue";
import ResultGrid from "./ResultGrid.vue";
import SqlEditor from "./SqlEditor.vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import type { ResultExportSnapshot } from "../lib/result-export";
import type {
  Driver,
  QueryResult,
  ScopeTableColumns,
} from "../lib/types";

const props = defineProps<{
  sql: string;
  tabId: string;
  driver: Driver;
  completionTables: ScopeTableColumns[];
  result: QueryResult | null;
  running: boolean;
  error: string | null;
  inTransaction: boolean;
  transactionBusy: boolean;
  editorRatio: number;
  resultCollapsed: boolean;
  canRun: boolean;
  runDisabledReason: string;
  runShortcutLabel: string;
}>();

const emit = defineEmits<{
  "update:sql": [value: string];
  selection: [value: string];
  run: [sql?: string];
  "begin-transaction": [];
  "commit-transaction": [];
  "rollback-transaction": [];
  "update:editor-ratio": [value: number];
  "toggle-result": [];
}>();

const resultGrid = ref<InstanceType<typeof ResultGrid> | null>(null);
const sqlEditor = ref<InstanceType<typeof SqlEditor> | null>(null);
const hiddenColumns = ref(0);
const exportOpen = ref(false);
const exportSnapshot = ref<ResultExportSnapshot | null>(null);
const toast = ref("");
let toastTimer = 0;

const exportDisabledReason = computed(() => {
  if (props.running) return "查询执行中，暂不能导出";
  if (props.error) return "查询失败，没有可导出的结果";
  if (!props.result || !props.result.isQuery) return "当前语句没有结果集";
  if (!props.result.rows.length) return "结果为空";
  return "";
});

function requestExport() {
  if (exportDisabledReason.value) return;
  resultGrid.value?.requestExport();
}

function handleExportRequest(snapshot: ResultExportSnapshot) {
  exportSnapshot.value = snapshot;
  exportOpen.value = true;
}

function closeExport() {
  exportOpen.value = false;
}

function handleExported(path: string, rows: number) {
  exportOpen.value = false;
  toast.value = `已导出 ${rows} 行到 ${path}`;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 3000);
}

defineExpose({
  requestExport,
  runEditorCommand: (
    command:
      | "search"
      | "find-next"
      | "find-previous"
      | "goto-line"
      | "toggle-comment",
  ) => sqlEditor.value?.runCommand(command),
});

onBeforeUnmount(() => {
  window.clearTimeout(toastTimer);
});

async function copyError(error: string) {
  try {
    await writeClipboard(error);
  } catch {
    // Clipboard permission is optional; the error stays visible either way.
  }
}
</script>

<template>
  <section
    class="query-workspace"
    :class="{ 'result-collapsed': resultCollapsed }"
  >
    <div class="editor-panel">
      <div class="panel-header">
        <span>SQL 编辑器</span>
        <div class="editor-actions">
          <button
            class="button primary compact"
            :disabled="!canRun || running"
            :title="runDisabledReason || `运行 SQL（${runShortcutLabel}）`"
            aria-label="运行 SQL"
            @click="emit('run')"
          >
            <span v-if="running" class="spinner"></span>
            <Icon v-else name="play" :size="13" />
            {{ running ? "执行中…" : "运行" }}
          </button>
          <span class="hint">⌘ / Ctrl + Enter 运行 · 选中时仅执行选中部分</span>
          <span v-if="inTransaction" class="transaction-badge">
            事务中 · 未提交
          </span>
          <button
            v-if="!inTransaction"
            class="button ghost compact"
            :disabled="transactionBusy || running"
            @click="emit('begin-transaction')"
          >
            开始事务
          </button>
          <template v-else>
            <button
              class="button ghost compact"
              :disabled="transactionBusy || running"
              @click="emit('commit-transaction')"
            >
              提交
            </button>
            <button
              class="button ghost compact"
              :disabled="transactionBusy || running"
              @click="emit('rollback-transaction')"
            >
              回滚
            </button>
          </template>
        </div>
      </div>
      <SqlEditor
        ref="sqlEditor"
        :model-value="sql"
        :tab-id="tabId"
        :driver="driver"
        :completion-tables="completionTables"
        @update:model-value="emit('update:sql', $event)"
        @selection="emit('selection', $event)"
        @run="emit('run', $event)"
      />
    </div>
    <ResizeHandle
      v-if="!resultCollapsed"
      orientation="horizontal"
      mode="ratio"
      :value="editorRatio"
      :min="0.2"
      :max="0.8"
      label="调整编辑器与结果高度"
      @update:value="emit('update:editor-ratio', $event)"
      @reset="emit('update:editor-ratio', 0.42)"
    />
    <div class="result-panel">
      <div class="panel-header">
        <span>结果</span>
        <div class="result-hints">
          <span v-if="result && !error" class="hint">
            {{ result.rows.length }} 行 · {{ result.elapsedMs }} ms
          </span>
          <button
            v-if="hiddenColumns && !error"
            class="link-button result-hidden-link"
            @click="resultGrid?.showAllColumns()"
          >
            已隐藏 {{ hiddenColumns }} 列 · 显示全部
          </button>
          <button
            class="button ghost compact"
            :disabled="Boolean(exportDisabledReason)"
            :title="exportDisabledReason || '导出当前结果'"
            @click="requestExport"
          >
            <Icon name="download" :size="13" />
            导出…
          </button>
          <button
            class="icon-button"
            :title="
              resultCollapsed
                ? '展开结果面板（⌘J）'
                : '折叠结果面板（⌘J）'
            "
            :aria-label="resultCollapsed ? '展开结果面板' : '折叠结果面板'"
            @click="emit('toggle-result')"
          >
            <Icon
              :name="resultCollapsed ? 'chevron-up' : 'chevron-down'"
              :size="14"
            />
          </button>
        </div>
      </div>

      <div v-if="error" class="query-error">
        <div class="query-error-head">
          <span class="query-error-title">
            <Icon name="warning" :size="14" />
            执行失败
          </span>
          <button class="button ghost compact" @click="copyError(error)">
            复制错误
          </button>
        </div>
        <pre class="query-error-body">{{ error }}</pre>
      </div>
      <ResultGrid
        v-else
        ref="resultGrid"
        :result="result"
        :running="running"
        @hidden-change="hiddenColumns = $event"
        @export-request="handleExportRequest"
      />
    </div>

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>

    <ResultExportDialog
      :open="exportOpen"
      :snapshot="exportSnapshot"
      :driver="driver"
      @close="closeExport"
      @exported="handleExported"
    />
  </section>
</template>
