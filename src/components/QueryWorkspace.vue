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
import ResizeHandle from "./ResizeHandle.vue";
import ExplainPlanViewer from "./ExplainPlanViewer.vue";
import ResultChart from "./ResultChart.vue";
import ResultExportDialog from "./ResultExportDialog.vue";
import PinnedResultBar from "./PinnedResultBar.vue";
import ResultCompareDialog from "./ResultCompareDialog.vue";
import ResultGrid from "./ResultGrid.vue";
import ResultProfile from "./ResultProfile.vue";
import ResultTranspose from "./ResultTranspose.vue";
import SqlCheckDialog from "./SqlCheckDialog.vue";
import SqlEditor from "./SqlEditor.vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import { checkSql } from "../lib/sql-check";
import { extractSqlParameters } from "../lib/sql-parameters";
import { hasSqlContent, splitSqlStatements } from "../lib/sql-statements";
import {
  MAX_PINNED_RESULTS,
  type PinnedQueryResult,
} from "../lib/pinned-results";
import type { CompareSource } from "../lib/result-compare";
import type { SqlNavigationPayload } from "../lib/sql-navigation";
import type { ColumnFormatEntry } from "../lib/column-format";
import type { ResultExportSnapshot } from "../lib/result-export";
import type { ChartData } from "../lib/result-chart";
import type {
  Driver,
  ExplainResult,
  QueryResult,
  ScopeTableColumns,
  SqlCheckIssue,
  SqlCheckResult,
} from "../lib/types";

const props = defineProps<{
  sql: string;
  tabId: string;
  tabTitle: string;
  driver: Driver;
  database?: string | null;
  schema?: string | null;
  completionTables: ScopeTableColumns[];
  result: QueryResult | null;
  /// Results from a multi-statement run in the order the statements executed.
  resultSets: QueryResult[];
  resultSetIndex: number;
  batchError: string | null;
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
  runMode: "statement" | "tab";
  explainResult: ExplainResult | null;
  explainError: string | null;
  explainLoading: boolean;
  showExplain: boolean;
  pinnedResults: PinnedQueryResult[];
  activePinId: string | null;
  parameterCount?: number;
  scriptContinueOnError?: boolean;
  scriptWrapInTransaction?: boolean;
}>();

const emit = defineEmits<{
  "update:sql": [value: string];
  selection: [value: string];
  run: [sql?: string];
  "run-statement": [];
  "run-tab": [];
  stop: [];
  explain: [];
  "open-snippets": [];
  "open-format-settings": [];
  "open-parameters": [];
  "open-datasets": [];
  "open-ai": [];
  "update:script-continue-on-error": [value: boolean];
  "update:script-wrap-in-transaction": [value: boolean];
  "begin-transaction": [];
  "commit-transaction": [];
  "rollback-transaction": [];
  "update:editor-ratio": [value: number];
  "toggle-result": [];
  "show-explain": [value: boolean];
  "select-result-set": [index: number];
  navigate: [payload: SqlNavigationPayload];
  "pin-result": [];
  "unpin-result": [id: string];
  "rename-pin": [payload: { id: string; name: string }];
  "select-pin": [id: string | null];
}>();

const liveResultGrid = ref<InstanceType<typeof ResultGrid> | null>(null);
const pinnedResultGrid = ref<InstanceType<typeof ResultGrid> | null>(null);
const columnFormats = ref<ColumnFormatEntry[]>([]);

watch(
  () => [props.tabId, props.result],
  () => {
    columnFormats.value = [];
  },
);
const sqlEditor = ref<InstanceType<typeof SqlEditor> | null>(null);
const liveHidden = ref(0);
const pinnedHidden = ref(0);
const liveFilter = reactive({
  count: 0,
  open: false,
  summary: null as string | null,
});
const pinnedFilter = reactive({
  count: 0,
  open: false,
  summary: null as string | null,
});
const activePin = computed(
  () =>
    props.pinnedResults.find((pin) => pin.id === props.activePinId) ?? null,
);
const sqlParameterCount = computed(
  () => extractSqlParameters(props.sql, props.driver).definitions.length,
);
const scriptStatementCount = computed(
  () =>
    splitSqlStatements(props.sql).filter((range) => hasSqlContent(range.text))
      .length,
);
const activePinResult = computed<QueryResult | null>(
  () => activePin.value?.result ?? null,
);
const activeResult = computed<QueryResult | null>(
  () => activePinResult.value ?? props.result,
);
const activeGrid = computed(() =>
  activePin.value ? pinnedResultGrid.value : liveResultGrid.value,
);
const hiddenColumns = computed(() =>
  activePin.value ? pinnedHidden.value : liveHidden.value,
);
const filterCount = computed(() =>
  activePin.value ? pinnedFilter.count : liveFilter.count,
);
const filterOpen = computed(() =>
  activePin.value ? pinnedFilter.open : liveFilter.open,
);
const filterSummary = computed(() =>
  activePin.value ? pinnedFilter.summary : liveFilter.summary,
);
const chartMode = ref(false);
const transposeMode = ref(false);
const profileMode = ref(false);
const liveChartData = ref<ChartData | null>(null);
const pinnedChartData = ref<ChartData | null>(null);
const chartData = computed<ChartData | null>(() =>
  activePin.value ? pinnedChartData.value : liveChartData.value,
);
const chartNonce = ref(0);
const runMenuOpen = ref(false);
const checkMenuOpen = ref(false);
const checkOpen = ref(false);
const checking = ref(false);
const checkResult = ref<SqlCheckResult | null>(null);
const checkError = ref("");
const checkScope = ref<"statement" | "tab">("statement");
const lastCheckRange = ref<{ from: number; to: number } | null>(null);
const scopeIndex = ref(0);
const scopeTotal = ref(0);
const selectionText = ref("");
const runModeLabel = computed(() =>
  props.runMode === "tab" ? "整个标签" : "当前语句",
);
const showScopeBadge = computed(
  () => scopeTotal.value > 1 && !selectionText.value.trim(),
);
const emptyChartData: ChartData = {
  columns: [],
  visibleColumns: [],
  rows: [],
  truncated: false,
};
const chartDataView = computed(() => chartData.value ?? emptyChartData);
const transposeDisabledReason = computed(() => {
  if (!activePin.value && props.running) return "查询执行中";
  if (!activeResult.value) return "没有可转置的查询结果";
  if (!activeResult.value.isQuery) return "当前语句没有结果集";
  if (!activeResult.value.rows.length) return "结果为空";
  if (!chartData.value?.visibleColumns.length) return "没有可见列";
  return "";
});
const chartDisabledReason = computed(() => {
  if (!activePin.value && props.running) return "查询执行中";
  if (!activeResult.value) return "没有可绘制的查询结果";
  if (!activeResult.value.isQuery) return "当前语句没有结果集";
  if (!activeResult.value.rows.length) return "结果为空";
  if (!chartData.value?.visibleColumns.length) return "没有可见列";
  return "";
});
const profileDisabledReason = computed(() => {
  if (!activePin.value && props.running) return "查询执行中";
  if (!activePin.value && props.error) return "查询失败，没有可剖析的结果";
  if (!activeResult.value) return "没有可剖析的查询结果";
  if (!activeResult.value.isQuery) return "当前语句没有结果集";
  if (!activeResult.value.rows.length) return "结果为空";
  if (!chartData.value?.visibleColumns.length) return "没有可见列";
  return "";
});
const checkScopeLabel = computed(() => {
  if (checkResult.value?.scope === "selection") return "当前选区";
  if (checkResult.value?.scope === "tab") return "整个标签";
  if (checkResult.value?.scope === "statement") return "当前语句";
  return checkScope.value === "tab" ? "整个标签" : "当前语句";
});
const filterDisabledReason = computed(() => {
  if (activePin.value) {
    if (!activePin.value.result.isQuery) return "固定结果没有结果集";
    if (!activePin.value.result.rows.length) return "固定结果为空";
    return "";
  }
  if (props.running) return "查询执行中";
  if (props.error) return "查询失败，没有可筛选的结果";
  if (!props.result) return "没有可筛选的结果";
  if (!props.result.isQuery) return "当前语句没有结果集";
  if (!props.result.rows.length) return "结果为空";
  return "";
});
const pinDisabledReason = computed(() => {
  if (props.activePinId) return "请先返回实时结果再固定";
  if (props.running) return "查询执行中";
  if (props.error) return "查询失败";
  if (props.batchError) return "部分语句执行失败";
  if (!props.result) return "当前没有查询结果";
  if (!props.result.isQuery) return "当前语句没有结果集";
  if (!props.result.rows.length) return "结果为空";
  if (props.pinnedResults.length >= MAX_PINNED_RESULTS) {
    return `每个查询标签最多固定 ${MAX_PINNED_RESULTS} 个结果，请先取消一个`;
  }
  return "";
});

const compareOpen = ref(false);
const compareBaselineId = ref("");
const compareTargetId = ref("");
const comparePinnedOnly = ref(false);

const liveCompareReason = computed(() => {
  if (props.running) return "查询执行中";
  if (props.error) return "查询失败";
  if (props.batchError) return "部分语句执行失败";
  if (!props.result) return "当前没有查询结果";
  if (!props.result.isQuery) return "当前语句没有结果集";
  if (!props.result.rows.length) return "结果为空";
  return "";
});

const compareSources = computed<CompareSource[]>(() => {
  const sources: CompareSource[] = props.pinnedResults.map((pin) => ({
    id: pin.id,
    name: pin.name,
    sql: pin.sql,
    kind: "pinned",
    pinnedAt: pin.pinnedAt,
    result: pin.result,
  }));
  if (!liveCompareReason.value && props.result) {
    sources.push({
      id: "live",
      name: "当前结果",
      sql: props.sql,
      kind: "live",
      result: props.result,
    });
  }
  return sources;
});

function openCompare(
  baseline: string,
  target: string,
  pinnedOnly = false,
) {
  compareBaselineId.value = baseline;
  compareTargetId.value = target;
  comparePinnedOnly.value = pinnedOnly;
  compareOpen.value = true;
}

function compareWithLive(pinId: string) {
  if (liveCompareReason.value) {
    handleEditorNotice(liveCompareReason.value, "error");
    return;
  }
  openCompare(pinId, "live");
}

function compareWithOther(pinId: string) {
  const other = props.pinnedResults.find((pin) => pin.id !== pinId);
  if (!other) {
    handleEditorNotice("需要至少两个固定结果", "error");
    return;
  }
  openCompare(pinId, other.id, true);
}

function compareLatest() {
  const pins = [...props.pinnedResults].sort(
    (left, right) => right.pinnedAt - left.pinnedAt,
  );
  const latest = pins[0];
  if (!latest) return;
  const target = liveCompareReason.value
    ? pins.find((pin) => pin.id !== latest.id)?.id
    : "live";
  if (!target) {
    handleEditorNotice("需要至少两个固定结果或一个可用的当前结果", "error");
    return;
  }
  openCompare(latest.id, target);
}

function exportPinned(id: string) {
  emit("select-pin", id);
  void nextTick(() => {
    void nextTick(() => activeGrid.value?.requestExport());
  });
}

async function copyPinnedSql(id: string) {
  const pin = props.pinnedResults.find((item) => item.id === id);
  if (!pin) return;
  try {
    await writeClipboard(pin.sql);
    handleEditorNotice("已复制固定结果中的 SQL", "info");
  } catch (reason) {
    handleEditorNotice(`复制失败：${reason}`, "error");
  }
}

watch(liveCompareReason, (reason) => {
  if (!compareOpen.value || !reason) return;
  if (
    compareBaselineId.value === "live" ||
    compareTargetId.value === "live"
  ) {
    compareOpen.value = false;
    handleEditorNotice(reason, "error");
  }
});

watch(
  () => props.pinnedResults,
  (pins) => {
    if (!compareOpen.value) return;
    const ids = new Set(pins.map((pin) => pin.id));
    if (
      (compareBaselineId.value !== "live" &&
        !ids.has(compareBaselineId.value)) ||
      (compareTargetId.value !== "live" &&
        !ids.has(compareTargetId.value))
    ) {
      compareOpen.value = false;
      handleEditorNotice("固定结果已取消", "error");
    }
  },
);

const exportOpen = ref(false);
const exportSnapshot = ref<ResultExportSnapshot | null>(null);
const formatMenuOpen = ref(false);
const formatBusy = ref(false);
const toast = ref("");
let toastTimer = 0;

const exportDisabledReason = computed(() => {
  if (activePin.value) {
    if (!activePin.value.result.isQuery) return "固定结果没有结果集";
    if (!activePin.value.result.rows.length) return "固定结果为空";
    return "";
  }
  if (props.running) return "查询执行中，暂不能导出";
  if (props.error) return "查询失败，没有可导出的结果";
  if (!props.result || !props.result.isQuery) return "当前语句没有结果集";
  if (!props.result.rows.length) return "结果为空";
  return "";
});

function requestExport() {
  if (exportDisabledReason.value) return;
  activeGrid.value?.requestExport();
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

function toggleRunMenu() {
  if (props.running) return;
  runMenuOpen.value = !runMenuOpen.value;
}

function handleSelection(value: string) {
  selectionText.value = value;
  emit("selection", value);
}

function handleScope(payload: { index: number; total: number }) {
  scopeIndex.value = payload.index;
  scopeTotal.value = payload.total;
}

function closeEditorMenus() {
  formatMenuOpen.value = false;
  runMenuOpen.value = false;
  checkMenuOpen.value = false;
}

function executeCheck(target: {
  text: string;
  document: string;
  from: number;
  to: number;
  scope: "statement" | "tab" | "selection";
}) {
  checkError.value = "";
  checking.value = true;
  try {
    lastCheckRange.value = { from: target.from, to: target.to };
    checkResult.value = checkSql({
      text: target.text,
      document: target.document,
      offset: target.from,
      scope: target.scope,
      driver: props.driver,
      tables: props.completionTables,
      scopeInfo: {
        database: props.database ?? null,
        schema: props.schema ?? null,
      },
    });
    checkOpen.value = true;
  } catch (reason) {
    checkResult.value = null;
    checkError.value = `检查失败：${reason}`;
    checkOpen.value = true;
  } finally {
    checking.value = false;
  }
}

function runCheck(
  scope: "statement" | "tab" = checkScope.value,
  ignoreSelection = false,
) {
  if (checking.value) return;
  checkScope.value = scope;
  checkMenuOpen.value = false;
  const target = sqlEditor.value?.checkTarget(scope, ignoreSelection);
  if (!target || !target.text.trim()) {
    handleEditorNotice("当前没有可检查的 SQL", "error");
    checkOpen.value = false;
    checkResult.value = null;
    checkError.value = "";
    lastCheckRange.value = null;
    return;
  }
  executeCheck(target);
}

function closeCheck() {
  checkOpen.value = false;
  checkResult.value = null;
  checkError.value = "";
  lastCheckRange.value = null;
}

function recheck() {
  if (checkResult.value?.scope === "selection" && lastCheckRange.value) {
    const target = sqlEditor.value?.rangeTarget(
      lastCheckRange.value.from,
      lastCheckRange.value.to,
    );
    if (target) {
      executeCheck(target);
      return;
    }
  }
  // Jumping to an issue selects it in the editor; rechecking a statement / tab
  // must not mistake that programmatic selection for a user selection.
  runCheck(checkScope.value, true);
}

function jumpToCheckIssue(issue: SqlCheckIssue) {
  sqlEditor.value?.revealRange(issue.from, issue.to);
}

function setResultView(
  view: "grid" | "chart" | "profile" | "explain",
) {
  if (view === "chart") {
    if (chartDisabledReason.value) return;
    chartMode.value = true;
    transposeMode.value = false;
    profileMode.value = false;
    emit("show-explain", false);
    return;
  }
  if (view === "profile") {
    if (profileDisabledReason.value) return;
    profileMode.value = true;
    chartMode.value = false;
    transposeMode.value = false;
    emit("show-explain", false);
    return;
  }
  chartMode.value = false;
  profileMode.value = false;
  if (view === "grid") transposeMode.value = false;
  emit("show-explain", view === "explain");
}

function toggleTranspose() {
  if (transposeDisabledReason.value) return;
  transposeMode.value = !transposeMode.value;
  chartMode.value = false;
  profileMode.value = false;
  emit("show-explain", false);
  if (transposeMode.value) {
    const data = activeGrid.value?.getChartData();
    if (activePin.value) pinnedChartData.value = data ?? null;
    else liveChartData.value = data ?? null;
    activeGrid.value?.clearSelection();
  }
}

function adjustFilterFromChart() {
  chartMode.value = false;
  void nextTick(() => activeGrid.value?.openFilter());
}

function handleChartData(
  data: ChartData,
  source: "live" | "pinned",
) {
  if (source === "pinned") pinnedChartData.value = data;
  else liveChartData.value = data;
}

function syncActiveChartData() {
  void nextTick(() => {
    const data = activeGrid.value?.getChartData() ?? null;
    if (activePin.value) pinnedChartData.value = data;
    else liveChartData.value = data;
  });
}

watch(
  () => props.activePinId,
  () => {
    chartMode.value = false;
    transposeMode.value = false;
    chartNonce.value += 1;
    if (
      !activePin.value &&
      (!props.result?.isQuery || !props.result.rows.length)
    ) {
      profileMode.value = false;
    }
    syncActiveChartData();
  },
);

watch(
  () => [props.result, props.resultSetIndex],
  () => {
    if (activePin.value) return;
    transposeMode.value = false;
    chartNonce.value += 1;
    if (!props.result?.isQuery || !props.result.rows.length) {
      chartMode.value = false;
      profileMode.value = false;
    }
  },
);

watch(
  () => props.running,
  (running) => {
    if (running && !activePin.value) profileMode.value = false;
  },
);

watch(
  () => props.tabId,
  () => {
    // The workspace is shared by every tab; return to the grid when the
    // active tab changes instead of showing the next tab's result as a chart.
    chartMode.value = false;
    transposeMode.value = false;
    profileMode.value = false;
    liveChartData.value = null;
    pinnedChartData.value = null;
    chartNonce.value += 1;
    compareOpen.value = false;
    checkOpen.value = false;
    checkResult.value = null;
    checkError.value = "";
    checkMenuOpen.value = false;
    lastCheckRange.value = null;
  },
);

function insertSnippet(text: string) {
  sqlEditor.value?.insertSnippet(text);
}

function handleExportRequest(snapshot: ResultExportSnapshot) {
  exportSnapshot.value = snapshot;
  exportOpen.value = true;
}

watch(
  () => props.result,
  () => {
    exportOpen.value = false;
  },
);

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
  runCheck,
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
  window.addEventListener("click", closeEditorMenus);
});

onBeforeUnmount(() => {
  window.clearTimeout(toastTimer);
  window.removeEventListener("click", closeEditorMenus);
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
          <div v-if="!running" class="run-menu-host" @click.stop>
            <button
              class="button primary compact"
              :disabled="!canRun"
              :title="
                runDisabledReason ||
                `运行${runModeLabel}（${runShortcutLabel}）`
              "
              aria-label="运行 SQL"
              @click="emit('run')"
            >
              <Icon name="play" :size="13" />
              运行
            </button>
            <button
              class="button ghost compact run-scope-toggle"
              :disabled="!canRun"
              :title="`运行范围：${runModeLabel}`"
              aria-label="选择运行范围"
              :aria-expanded="runMenuOpen"
              @click="toggleRunMenu"
            >
              <Icon name="chevron-down" :size="12" />
            </button>
            <span
              v-if="scriptStatementCount > 1"
              class="run-script-badge"
              title="整段 SQL 包含多条语句"
            >
              脚本模式（{{ scriptStatementCount }} 条）
            </span>
            <span
              v-if="showScopeBadge"
              class="run-scope-badge"
              aria-live="polite"
              :aria-label="`第 ${scopeIndex} / ${scopeTotal} 条语句`"
            >
              第 {{ scopeIndex }} / {{ scopeTotal }} 条
            </span>
            <button
              v-if="sqlParameterCount"
              class="button ghost compact sql-parameter-badge"
              :title="`检测到 ${sqlParameterCount} 个 SQL 参数，点击重新填写`"
              aria-label="编辑 SQL 参数"
              @click="emit('open-parameters')"
            >
              参数：{{ sqlParameterCount }}
            </button>
            <div v-if="runMenuOpen" class="context-menu run-scope-menu">
              <button
                class="context-menu-item"
                @click="emit('run-statement'); runMenuOpen = false"
              >
                <span class="layout-check-slot">
                  <Icon
                    v-if="runMode === 'statement'"
                    name="check"
                    :size="13"
                  />
                </span>
                <span>运行当前语句</span>
              </button>
              <button
                class="context-menu-item"
                @click="emit('run-tab'); runMenuOpen = false"
              >
                <span class="layout-check-slot">
                  <Icon v-if="runMode === 'tab'" name="check" :size="13" />
                </span>
                <span>运行整个标签</span>
              </button>
              <div class="context-separator"></div>
              <button
                class="context-menu-item"
                @click="
                  emit(
                    'update:script-continue-on-error',
                    !scriptContinueOnError,
                  )
                "
              >
                <span class="layout-check-slot">
                  <Icon v-if="scriptContinueOnError" name="check" :size="13" />
                </span>
                <span>多语句出错继续</span>
              </button>
              <button
                class="context-menu-item"
                title="所有语句在同一事务中执行；失败整体回滚（MySQL DDL 可能隐式提交）"
                @click="
                  emit(
                    'update:script-wrap-in-transaction',
                    !scriptWrapInTransaction,
                  )
                "
              >
                <span class="layout-check-slot">
                  <Icon v-if="scriptWrapInTransaction" name="check" :size="13" />
                </span>
                <span>脚本事务包裹</span>
              </button>
            </div>
          </div>
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
          <div class="check-menu-host" @click.stop>
            <button
              class="button ghost compact"
              :disabled="checking"
              title="检查当前 SQL 的表 / 列 / 词法与危险语句，不执行 SQL"
              @click="runCheck('statement')"
            >
              <Icon name="check" :size="13" />
              {{ checking ? "检查中…" : "检查" }}
            </button>
            <button
              class="button ghost compact check-scope-toggle"
              :disabled="checking"
              title="选择检查范围"
              aria-label="选择检查范围"
              :aria-expanded="checkMenuOpen"
              @click="checkMenuOpen = !checkMenuOpen"
            >
              <Icon name="chevron-down" :size="10" />
            </button>
            <div v-if="checkMenuOpen" class="context-menu check-menu">
              <button
                class="context-menu-item"
                @click="runCheck('statement')"
              >
                <span class="layout-check-slot">
                  <Icon
                    v-if="checkScope === 'statement'"
                    name="check"
                    :size="13"
                  />
                </span>
                <span>检查当前语句</span>
              </button>
              <button class="context-menu-item" @click="runCheck('tab')">
                <span class="layout-check-slot">
                  <Icon v-if="checkScope === 'tab'" name="check" :size="13" />
                </span>
                <span>检查整个标签</span>
              </button>
              <p class="check-menu-hint">有选区时始终检查选区</p>
            </div>
          </div>
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
        :database="database ?? null"
        :schema="schema ?? null"
        :completion-tables="completionTables"
        @update:model-value="emit('update:sql', $event)"
        @selection="handleSelection"
        @scope="handleScope"
        @notice="handleEditorNotice"
        @busy="formatBusy = $event"
        @run="emit('run', $event)"
        @navigate="emit('navigate', $event)"
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
        <span v-if="parameterCount" class="result-parameter-badge">
          已应用 {{ parameterCount }} 个参数
        </span>
        <div class="result-hints">
          <div
            v-if="activeResult || explainResult || explainLoading || explainError"
            class="segmented compact-segmented"
          >
            <button
              class="segment"
              :class="{
                active:
                  !showExplain && !chartMode && !transposeMode && !profileMode,
              }"
              @click="setResultView('grid')"
            >
              结果
            </button>
            <button
              class="segment"
              :class="{ active: transposeMode }"
              :disabled="Boolean(transposeDisabledReason)"
              :title="transposeDisabledReason || '按字段查看当前结果（只读）'"
              @click="toggleTranspose"
            >
              转置
            </button>
            <button
              class="segment"
              :class="{ active: chartMode }"
              :disabled="Boolean(chartDisabledReason)"
              :title="chartDisabledReason || '按当前结果与筛选绘制图表'"
              @click="setResultView('chart')"
            >
              图表
            </button>
            <button
              class="segment"
              :class="{ active: profileMode }"
              :disabled="Boolean(profileDisabledReason)"
              :title="
                profileDisabledReason ||
                '按列查看 NULL、Distinct、类型、分布与质量（本地计算）'
              "
              @click="setResultView('profile')"
            >
              剖析
            </button>
            <button
              v-if="explainResult || explainLoading || explainError"
              class="segment"
              :class="{ active: showExplain }"
              @click="setResultView('explain')"
            >
              执行计划
            </button>
          </div>
          <button
            class="button ghost compact result-pin-button"
            :disabled="Boolean(pinDisabledReason) || running"
            :title="pinDisabledReason || '固定当前结果（不包含本地筛选 / 排序）'"
            @click="emit('pin-result')"
          >
            <Icon name="pin" :size="13" />
            固定
          </button>
          <button
            v-if="!showExplain && !chartMode && !transposeMode && !profileMode && (!error || activePin)"
            class="button ghost compact result-filter-button"
            :class="{ active: filterOpen }"
            :disabled="Boolean(filterDisabledReason)"
            :title="filterDisabledReason || '筛选当前已加载结果'"
            @click="activeGrid?.toggleFilter()"
          >
            <Icon name="filter" :size="13" />
            筛选
            <span v-if="filterCount" class="result-filter-badge">
              {{ filterCount }}
            </span>
          </button>
          <button
            v-if="!showExplain && !chartMode && !transposeMode && !profileMode && (!error || activePin)"
            class="button ghost compact"
            title="在已加载结果中查找（⌘/Ctrl+F）"
            @click="activeGrid?.focusSearch()"
          >
            <Icon name="search" :size="13" />
            查找
          </button>
          <span
            v-if="activeResult && (!error || activePin) && !showExplain && !profileMode"
            class="hint"
          >
            {{
              activeResult.isQuery
                ? `${activeResult.rows.length} 行 · ${activeResult.elapsedMs} ms`
                : `影响 ${activeResult.rowsAffected} 行 · ${activeResult.elapsedMs} ms`
            }}
          </span>
          <button
            v-if="hiddenColumns && (!error || activePin) && !showExplain && !chartMode && !transposeMode && !profileMode"
            class="link-button result-hidden-link"
            @click="activeGrid?.showAllColumns()"
          >
            已隐藏 {{ hiddenColumns }} 列 · 显示全部
          </button>
          <button
            class="button ghost compact"
            title="AI SQL 助手（需要先在设置中配置模型）"
            @click="emit('open-ai')"
          >
            <Icon name="bolt" :size="13" />
            AI
          </button>
          <button
            class="button ghost compact"
            title="数据集与 HTML 报表"
            @click="emit('open-datasets')"
          >
            <Icon name="list" :size="13" />
            数据集
          </button>
          <button
            v-if="!showExplain && !chartMode && !transposeMode && !profileMode"
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

      <PinnedResultBar
        :pins="pinnedResults"
        :active-id="activePinId"
        :live-compare-reason="liveCompareReason"
        @select="emit('select-pin', $event)"
        @unpin="emit('unpin-result', $event)"
        @rename="emit('rename-pin', $event)"
        @compare-current="compareWithLive"
        @compare-other="compareWithOther"
        @compare-latest="compareLatest"
        @export="exportPinned"
        @copy-sql="copyPinnedSql"
      />

      <div
        v-if="resultSets.length > 1 && !showExplain && !profileMode && !activePin"
        class="result-set-tabs"
        role="tablist"
        aria-label="查询结果集"
      >
        <button
          v-for="(item, index) in resultSets"
          :key="index"
          class="result-set-tab"
          :class="{ active: index === resultSetIndex }"
          role="tab"
          :aria-selected="index === resultSetIndex"
          @click="emit('select-result-set', index)"
        >
          <span>结果 {{ index + 1 }}</span>
          <span class="result-set-meta">
            {{
              item.isQuery
                ? `${item.rows.length} 行`
                : `影响 ${item.rowsAffected} 行`
            }}
          </span>
        </button>
      </div>

      <ExplainPlanViewer
        v-if="showExplain"
        :result="explainResult"
        :error="explainError"
        :loading="explainLoading"
      />
      <div v-else-if="error && !activePin" class="query-error">
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
      <template v-else>
        <div
          v-if="activePin"
          class="pinned-view-banner"
          role="status"
        >
          <Icon name="pin" :size="13" />
          <span>
            固定结果（原始结果，不含本地筛选 / 排序）·
            <strong>{{ activePin.name }}</strong> · 固定于
            {{ new Date(activePin.pinnedAt).toLocaleTimeString("zh-CN") }}
            · {{ activePin.result.rows.length }} 行 · 不代表数据库当前状态
            <template v-if="activePin.result.truncated">
              · 结果已截断，只固定已加载的
              {{ activePin.result.rows.length }} 行
            </template>
          </span>
          <button
            class="link-button"
            @click="emit('select-pin', null)"
          >
            返回实时结果
          </button>
        </div>
        <div
          v-if="batchError && !chartMode && !profileMode && !activePin"
          class="query-error batch-query-error"
        >
          <div class="query-error-head">
            <span class="query-error-title">
              <Icon name="warning" :size="14" />
              部分语句失败
            </span>
            <button class="button ghost compact" @click="copyError(batchError)">
              复制错误
            </button>
          </div>
          <pre class="query-error-body">{{ batchError }}</pre>
        </div>
        <div v-if="profileMode" class="result-profile-slot">
          <ResultProfile
            :key="activePin ? `pin-${activePin.id}` : 'live'"
            :data="chartDataView"
            :filter-count="filterCount"
            :filter-summary="filterSummary"
            :pinned-name="activePin?.name ?? null"
          />
        </div>
        <ResultTranspose
          v-if="transposeMode && !chartMode"
          :data="chartDataView"
          :title="activePin ? `${tabTitle} · ${activePin.name}` : tabTitle"
          :formats="columnFormats"
        />
        <ResultChart
          v-show="chartMode"
          :data="chartDataView"
          :dataset-key="String(chartNonce)"
          :tab-title="tabTitle"
          :filter-summary="filterSummary"
          :filter-count="filterCount"
          @adjust-filter="adjustFilterFromChart"
        />
        <ResultGrid
          ref="liveResultGrid"
          v-show="!chartMode && !transposeMode && !profileMode && !activePin"
          :result="result"
          :running="running"
          :formats="columnFormats"
          @update:formats="columnFormats = $event"
          @hidden-change="liveHidden = $event"
          @export-request="handleExportRequest"
          @filter-change="
            liveFilter.count = $event.count;
            liveFilter.open = $event.open;
            liveFilter.summary = $event.summary;
          "
          @chart-data="handleChartData($event, 'live')"
        />
        <ResultGrid
          v-if="activePin"
          ref="pinnedResultGrid"
          :key="activePin.id"
          v-show="!chartMode && !transposeMode && !profileMode"
          :result="activePin.result"
          :running="false"
          :formats="columnFormats"
          @update:formats="columnFormats = $event"
          @hidden-change="pinnedHidden = $event"
          @export-request="handleExportRequest"
          @filter-change="
            pinnedFilter.count = $event.count;
            pinnedFilter.open = $event.open;
            pinnedFilter.summary = $event.summary;
          "
          @chart-data="handleChartData($event, 'pinned')"
        />
      </template>
    </div>

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>

    <SqlCheckDialog
      :open="checkOpen"
      :result="checkResult"
      :checking="checking"
      :error="checkError"
      :scope-label="checkScopeLabel"
      @close="closeCheck"
      @recheck="recheck"
      @jump="jumpToCheckIssue"
    />

    <ResultCompareDialog
      :open="compareOpen"
      :sources="compareSources"
      :initial-baseline-id="compareBaselineId"
      :initial-target-id="compareTargetId"
      :pinned-only="comparePinnedOnly"
      @close="compareOpen = false"
    />

    <ResultExportDialog
      :open="exportOpen"
      :snapshot="exportSnapshot"
      :driver="driver"
      :filename-prefix="
        activePin ? `mydb-pin-${activePin.name}` : undefined
      "
      @close="closeExport"
      @exported="handleExported"
    />
  </section>
</template>
