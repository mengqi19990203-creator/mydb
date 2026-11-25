<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import ResizeHandle from "./ResizeHandle.vue";
import ExplainPlanViewer from "./ExplainPlanViewer.vue";
import ResultExportDialog from "./ResultExportDialog.vue";
import ResultGrid from "./ResultGrid.vue";
import SqlEditor from "./SqlEditor.vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import type { ResultExportSnapshot } from "../lib/result-export";
import type {
  Driver,
  ExplainResult,
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
  stopping: boolean;
  stopNote: string | null;
  error: string | null;
  inTransaction: boolean;
  transactionBusy: boolean;
  editorRatio: number;
  resultCollapsed: boolean;
  canRun: boolean;
  runDisabledReason: string;
  runShortcutLabel: string;
  explainResult: ExplainResult | null;
  explainError: string | null;
  explainLoading: boolean;
  showExplain: boolean;
}>();

const emit = defineEmits<{
  "update:sql": [value: string];
  selection: [value: string];
  run: [sql?: string];
  stop: [];
  explain: [];
  "open-snippets": [];
  "open-format-settings": [];
  "begin-transaction": [];
  "commit-transaction": [];
  "rollback-transaction": [];
  "update:editor-ratio": [value: number];
  "toggle-result": [];
  "show-explain": [value: boolean];
}>();

const resultGrid = ref<InstanceType<typeof ResultGrid> | null>(null);
const sqlEditor = ref<InstanceType<typeof SqlEditor> | null>(null);
const hiddenColumns = ref(0);
const exportOpen = ref(false);
const exportSnapshot = ref<ResultExportSnapshot | null>(null);
const formatMenuOpen = ref(false);
const formatBusy = ref(false);
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

function runFormatCommand(command: "format-sql" | "compress-sql") {
  formatMenuOpen.value = false;
  sqlEditor.value?.runCommand(command);
}

function openFormatSettings() {
  formatMenuOpen.value = false;
  emit("open-format-settings");
}

function handleEditorNotice(message: string, kind: "info" | "error") {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, kind === "error" ? 4000 : 1800);
}

function closeFormatMenu() {
  formatMenuOpen.value = false;
}

function insertSnippet(text: string) {
  sqlEditor.value?.insertSnippet(text);
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
  insertSnippet,
  currentStatement: () => sqlEditor.value?.currentStatement() ?? "",
  runEditorCommand: (
    command:
      | "search"
      | "find-next"
      | "find-previous"
      | "goto-line"
      | "toggle-comment"
      | "format-sql"
      | "compress-sql",
  ) => sqlEditor.value?.runCommand(command),
});

onMounted(() => {
  window.addEventListener("click", closeFormatMenu);
});

onBeforeUnmount(() => {
  window.clearTimeout(toastTimer);
  window.removeEventListener("click", closeFormatMenu);
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
            v-if="!running"
            class="button primary compact"
            :disabled="!canRun"
            :title="runDisabledReason || `运行 SQL（${runShortcutLabel}）`"
            aria-label="运行 SQL"
            @click="emit('run')"
          >
            <Icon name="play" :size="13" />
            运行
          </button>
          <button
            v-else
            class="button danger compact"
            :disabled="stopping"
            title="停止当前查询（Mod+.）"
            aria-label="停止查询"
            @click="emit('stop')"
          >
            <span v-if="stopping" class="spinner"></span>
            <Icon v-else name="close" :size="13" />
            {{ stopping ? "停止中…" : "停止" }}
          </button>
          <button
            class="button ghost compact"
            :disabled="!canRun || running"
            :title="
              runDisabledReason ||
              '解释当前选中语句或光标所在语句，不修改编辑器内容'
            "
            @click="emit('explain')"
          >
            <Icon name="bolt" :size="13" />
            解释
          </button>
          <div class="format-menu-host" @click.stop>
            <button
              class="button ghost compact"
              :disabled="running || formatBusy"
              :title="
                running
                  ? '查询执行中'
                  : '格式化 / 压缩 SQL（Shift+Mod+F）'
              "
              :aria-expanded="formatMenuOpen"
              @click="formatMenuOpen = !formatMenuOpen"
            >
              <Icon name="code" :size="13" />
              {{ formatBusy ? "格式化中…" : "格式" }}
              <Icon name="chevron-down" :size="10" />
            </button>
            <div v-if="formatMenuOpen" class="context-menu format-menu">
              <button class="context-menu-item" @click="runFormatCommand('format-sql')">
                格式化 SQL
                <span class="context-menu-shortcut">⇧⌘F</span>
              </button>
              <button class="context-menu-item" @click="runFormatCommand('compress-sql')">
                压缩 SQL
              </button>
              <div class="context-separator"></div>
              <button class="context-menu-item" @click="openFormatSettings">
                格式化设置…
              </button>
            </div>
          </div>
          <button
            class="button ghost compact"
            title="SQL 片段库（Shift+Mod+K）"
            @click="emit('open-snippets')"
          >
            <Icon name="list" :size="13" />
            片段
          </button>
          <span class="hint">⌘ / Ctrl + Enter 运行 · 选中时仅执行选中部分</span>
          <span v-if="stopNote" class="hint stop-note">{{ stopNote }}</span>
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
        @notice="handleEditorNotice"
        @busy="formatBusy = $event"
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
          <div
            v-if="explainResult || explainLoading || explainError"
            class="segmented compact-segmented"
          >
            <button
              class="segment"
              :class="{ active: !showExplain }"
              @click="emit('show-explain', false)"
            >
              结果
            </button>
            <button
              class="segment"
              :class="{ active: showExplain }"
              @click="emit('show-explain', true)"
            >
              执行计划
            </button>
          </div>
          <span v-if="result && !error && !showExplain" class="hint">
            {{ result.rows.length }} 行 · {{ result.elapsedMs }} ms
          </span>
          <button
            v-if="hiddenColumns && !error && !showExplain"
            class="link-button result-hidden-link"
            @click="resultGrid?.showAllColumns()"
          >
            已隐藏 {{ hiddenColumns }} 列 · 显示全部
          </button>
          <button
            v-if="!showExplain"
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

      <ExplainPlanViewer
        v-if="showExplain"
        :result="explainResult"
        :error="explainError"
        :loading="explainLoading"
      />
      <div v-else-if="error" class="query-error">
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
