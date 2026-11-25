<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import DangerousSqlDialog from "./DangerousSqlDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  findDangerousSql,
  loadDangerConfirm,
} from "../lib/dangerous-sql";
import type {
  Driver,
  SqlFilePreview,
  SqlScriptProgress,
  SqlScriptSummary,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
  readOnly: boolean;
  initialPath?: string | null;
}>();

const emit = defineEmits<{
  close: [];
  executed: [summary: SqlScriptSummary];
}>();

const path = ref("");
const preview = ref<SqlFilePreview | null>(null);
const loading = ref(false);
const executing = ref(false);
const error = ref("");
const stopOnError = ref(true);
const useTransaction = ref(true);
const progress = ref<SqlScriptProgress | null>(null);
const progressLog = ref<SqlScriptProgress[]>([]);
const summary = ref<SqlScriptSummary | null>(null);
const cancelRequested = ref(false);
const operationId = ref("");
const dangerousOpen = ref(false);
let unlisten: UnlistenFn | undefined;

const dangerousStatements = computed(() =>
  (preview.value?.statements ?? []).filter(
    (statement) => findDangerousSql(statement.preview) !== null,
  ),
);

const fileName = computed(() =>
  path.value ? path.value.split("/").pop() ?? path.value : "",
);

const fileSize = computed(() => {
  const bytes = preview.value?.bytes ?? 0;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
});

const hasTransactionControl = computed(() =>
  Boolean(
    preview.value?.statements.some(
      (statement) => statement.kind === "transaction",
    ),
  ),
);

const progressPercent = computed(() => {
  const current = progress.value;
  if (!current || !current.total) return 0;
  return Math.min(100, Math.round((current.index / current.total) * 100));
});

const canExecute = computed(
  () =>
    Boolean(preview.value) &&
    !loading.value &&
    !executing.value &&
    (preview.value?.violationCount ?? 0) === 0,
);

const kindLabels: Record<string, string> = {
  query: "查询",
  dml: "DML",
  ddl: "DDL",
  transaction: "事务",
  other: "其他",
};

function formatElapsed(value: number): string {
  if (value < 1000) return `${value} ms`;
  return `${(value / 1000).toFixed(2)} s`;
}

function reset() {
  path.value = "";
  preview.value = null;
  loading.value = false;
  executing.value = false;
  error.value = "";
  stopOnError.value = true;
  useTransaction.value = true;
  progress.value = null;
  progressLog.value = [];
  summary.value = null;
  cancelRequested.value = false;
  operationId.value = "";
}

async function chooseFile() {
  if (executing.value) return;
  error.value = "";
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "SQL 文件", extensions: ["sql"] }],
  });
  if (typeof selected !== "string") return;
  await loadPreview(selected);
}

async function loadPreview(selected: string) {
  loading.value = true;
  error.value = "";
  summary.value = null;
  progress.value = null;
  progressLog.value = [];
  useTransaction.value = true;
  stopOnError.value = true;
  try {
    const result = await api.previewSqlFile(props.connectionId, selected);
    path.value = selected;
    preview.value = result;
    if (
      result.statements.some((statement) => statement.kind === "transaction")
    ) {
      useTransaction.value = false;
    }
    if (result.statementCount === 0) {
      error.value = "SQL 文件没有可执行语句";
    }
  } catch (reason) {
    preview.value = null;
    path.value = "";
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function execute(bypassDanger = false) {
  if (!canExecute.value || !preview.value) return;
  if (preview.value.violationCount > 0) {
    error.value = "脚本包含只读连接不允许的写入语句，已阻止执行";
    return;
  }
  if (
    !bypassDanger &&
    loadDangerConfirm() &&
    dangerousStatements.value.length > 0
  ) {
    dangerousOpen.value = true;
    return;
  }
  dangerousOpen.value = false;
  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `script-${Date.now()}`;
  operationId.value = id;
  executing.value = true;
  cancelRequested.value = false;
  error.value = "";
  summary.value = null;
  progressLog.value = [];
  const wantsTransaction = useTransaction.value && !hasTransactionControl.value;
  try {
    const result = await api.executeSqlFile(
      props.connectionId,
      path.value,
      {
        stopOnError: wantsTransaction ? true : stopOnError.value,
        useTransaction: wantsTransaction,
      },
      id,
    );
    summary.value = result;
    emit("executed", result);
  } catch (reason) {
    error.value = cancelRequested.value ? "执行已取消" : String(reason);
  } finally {
    executing.value = false;
    cancelRequested.value = false;
  }
}

async function cancelExecution() {
  if (!executing.value || !operationId.value) return;
  cancelRequested.value = true;
  try {
    await api.cancelSqlScript(operationId.value);
  } catch (reason) {
    error.value = String(reason);
  }
}

function requestClose() {
  if (executing.value) return;
  emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      if (props.initialPath) {
        void loadPreview(props.initialPath);
      }
    }
  },
  { immediate: true },
);

watch(useTransaction, (value) => {
  if (value) stopOnError.value = true;
});

onMounted(async () => {
  unlisten = await listen<SqlScriptProgress>(
    "sql-script:progress",
    (event) => {
      if (event.payload.operationId !== operationId.value) return;
      progress.value = event.payload;
      if (event.payload.status !== "running") {
        progressLog.value = [
          event.payload,
          ...progressLog.value,
        ].slice(0, 30);
      }
    },
  );
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="requestClose">
    <div
      class="modal sql-script-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sql-script-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="sql-script-title">执行 SQL 文件</h2>
          <p>{{ scopeLabel }} · 先预览语句与只读检查，再选择事务和失败策略</p>
        </div>
        <button
          class="icon-button"
          title="关闭"
          :disabled="executing"
          @click="requestClose"
        >
          <Icon name="close" />
        </button>
      </header>

      <div class="sql-script-body">
        <section class="sql-script-file">
          <div class="sql-script-file-row">
            <div class="sql-script-file-name">
              <strong>{{ fileName || "尚未选择文件" }}</strong>
              <span v-if="preview" class="hint">
                {{ fileSize }} · {{ preview.statementCount }} 条语句
              </span>
            </div>
            <button
              class="button ghost compact"
              :disabled="executing || loading"
              @click="chooseFile"
            >
              {{ preview ? "更换文件…" : "选择 SQL 文件…" }}
            </button>
          </div>
          <p v-if="path" class="sql-script-path" :title="path">{{ path }}</p>
          <p v-if="readOnly || preview?.readOnly" class="draft-hint">
            当前连接为只读模式：脚本中只要包含写语句就会整体拒绝，不会执行任何语句。
          </p>
          <p v-if="loading" class="draft-hint">正在读取并分析 SQL 文件…</p>
        </section>

        <section v-if="preview" class="sql-script-options">
          <label class="checkbox-row">
            <input
              v-model="useTransaction"
              type="checkbox"
              :disabled="executing || hasTransactionControl"
            />
            整脚本使用一个事务
          </label>
          <label class="checkbox-row">
            <input
              v-model="stopOnError"
              type="checkbox"
              :disabled="executing || useTransaction || hasTransactionControl"
            />
            遇错停止（关闭后继续执行剩余语句）
          </label>
          <p v-if="useTransaction && !hasTransactionControl" class="draft-hint">
            整脚本事务开启时遇错会停止并回滚；MySQL 的 DDL 会隐式提交，无法全部回滚。
          </p>
          <p v-if="hasTransactionControl" class="draft-hint">
            脚本包含 BEGIN / COMMIT 等事务控制语句，外层事务会自动关闭。
          </p>
          <div v-if="preview.violationCount" class="sql-script-violations">
            <strong>只读连接阻止执行（{{ preview.violationCount }} 条）</strong>
            <p
              v-for="violation in preview.violations"
              :key="violation.index"
            >
              第 {{ violation.index }} 条 · 文件第 {{ violation.line }} 行：{{
                violation.message
              }}
            </p>
            <p v-if="preview.violationCount > preview.violations.length" class="hint">
              仅显示前 {{ preview.violations.length }} 条
            </p>
          </div>
        </section>

        <section v-if="preview" class="sql-script-statements">
          <div class="sql-script-section-head">
            <strong>语句预览</strong>
            <span class="hint">
              显示前 {{ preview.statements.length }} / {{ preview.statementCount }} 条
            </span>
          </div>
          <div class="sql-script-statement-list">
            <div
              v-for="statement in preview.statements"
              :key="statement.index"
              class="sql-script-statement"
            >
              <span class="sql-script-index">{{ statement.index }}</span>
              <span class="sql-script-line">L{{ statement.line }}</span>
              <span class="sql-script-kind">{{
                kindLabels[statement.kind] ?? statement.kind
              }}</span>
              <code :title="statement.preview">{{ statement.preview }}</code>
            </div>
          </div>
        </section>

        <section v-if="executing || summary" class="sql-script-run">
          <div v-if="executing" class="sql-script-progress">
            <div class="sql-script-section-head">
              <span>
                第 {{ progress?.index ?? 0 }} / {{ progress?.total ?? preview?.statementCount ?? 0 }} 条 ·
                {{ progress?.preview ?? "准备执行…" }}
              </span>
              <span>{{ progressPercent }}%</span>
            </div>
            <div class="sql-script-progress-track">
              <div
                class="sql-script-progress-fill"
                :style="{ width: `${progressPercent}%` }"
              ></div>
            </div>
            <div class="sql-script-run-actions">
              <span class="hint">
                {{ progress?.status === "running" ? "执行中" : progress?.status ?? "" }}
                · 客户端会在当前语句结束后停止
              </span>
              <button
                class="button ghost compact"
                :disabled="cancelRequested"
                @click="cancelExecution"
              >
                {{ cancelRequested ? "正在取消…" : "取消执行" }}
              </button>
            </div>
          </div>

          <div v-if="summary" class="sql-script-summary">
            <strong>
              {{ summary.cancelled ? "执行已取消" : summary.failed ? "执行完成，存在失败" : "执行完成" }}
            </strong>
            <p class="hint">
              共 {{ summary.total }} 条 · 成功 {{ summary.succeeded }} · 失败
              {{ summary.failed }} · 未执行 {{ summary.skipped }} · 影响
              {{ summary.rowsAffected }} 行 · {{ formatElapsed(summary.elapsedMs) }}
              <template v-if="summary.rolledBack"> · 已回滚</template>
            </p>
            <div v-if="summary.failures.length" class="sql-script-failures">
              <p v-for="failure in summary.failures" :key="failure.index">
                第 {{ failure.index }} 条 · 文件第 {{ failure.line }} 行：{{
                  failure.message
                }}
              </p>
            </div>
            <div v-if="summary.notes.length" class="sql-script-notes">
              <p v-for="note in summary.notes" :key="note">{{ note }}</p>
            </div>
          </div>

          <div v-if="progressLog.length" class="sql-script-log">
            <div
              v-for="entry in progressLog.slice(0, 8)"
              :key="`${entry.index}:${entry.status}`"
              :class="['sql-script-log-row', entry.status]"
            >
              <span>{{ entry.index }}</span>
              <span>{{ entry.status === "success" ? "成功" : "失败" }}</span>
              <code :title="entry.preview">{{ entry.preview }}</code>
              <span class="hint">{{ entry.elapsedMs }} ms</span>
            </div>
          </div>
        </section>

        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" :disabled="executing" @click="requestClose">
          关闭
        </button>
        <div class="spacer"></div>
        <button
          class="button primary"
          :disabled="!canExecute"
          @click="execute()"
        >
          {{ executing ? "执行中…" : "开始执行" }}
        </button>
      </footer>

      <DangerousSqlDialog
        :open="dangerousOpen"
        :sql="dangerousStatements.map((item) => item.preview).join('\n')"
        :message="`脚本包含 ${dangerousStatements.length} 条危险语句（缺少 WHERE、DROP、TRUNCATE 或 DROP COLUMN），请确认后再执行。`"
        :high-risk="true"
        :connection-name="scopeLabel"
        :scope="fileName || 'SQL 文件'"
        @confirm="execute(true)"
        @cancel="dangerousOpen = false"
      />
    </div>
  </div>
</template>

<style scoped>
.sql-script-modal {
  width: min(940px, calc(100vw - 48px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
}

.sql-script-body {
  min-height: 0;
  overflow-y: auto;
  padding: 0 20px 10px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.sql-script-file,
.sql-script-options,
.sql-script-statements,
.sql-script-run {
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.sql-script-file-row,
.sql-script-section-head,
.sql-script-run-actions,
.sql-script-log-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.sql-script-file-name {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.sql-script-path {
  margin: 6px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 11px;
}

.sql-script-options {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
}

.sql-script-options .draft-hint,
.sql-script-options .sql-script-violations {
  flex-basis: 100%;
  margin: 0;
}

.sql-script-violations,
.sql-script-failures {
  color: var(--danger);
}

.sql-script-violations p,
.sql-script-failures p,
.sql-script-notes p {
  margin: 3px 0;
  font-size: 11px;
}

.sql-script-statement-list {
  max-height: 240px;
  margin-top: 8px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: var(--surface-well);
}

.sql-script-statement {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
}

.sql-script-statement:last-child {
  border-bottom: none;
}

.sql-script-index {
  width: 28px;
  color: var(--muted);
  text-align: right;
}

.sql-script-line {
  width: 42px;
  color: var(--muted);
  font-family: var(--font-mono);
}

.sql-script-kind {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--panel-3);
  color: var(--text-secondary);
  font-size: 9px;
}

.sql-script-statement code,
.sql-script-log-row code {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}

.sql-script-statement code {
  flex: 1;
}

.sql-script-progress-track {
  height: 6px;
  margin: 8px 0;
  overflow: hidden;
  border-radius: 999px;
  background: var(--surface-well);
}

.sql-script-progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 160ms ease;
}

.sql-script-log {
  margin-top: 10px;
  max-height: 150px;
  overflow: auto;
}

.sql-script-log-row {
  justify-content: flex-start;
  padding: 3px 0;
  font-size: 11px;
}

.sql-script-log-row.failed {
  color: var(--danger);
}

.sql-script-log-row code {
  flex: 1;
}

.sql-script-notes {
  margin-top: 6px;
  color: var(--muted);
}
</style>