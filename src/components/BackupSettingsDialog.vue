<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type {
  BackupJob,
  BackupRun,
  ConnectionConfig,
  DatabaseExportProgress,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connections: ConnectionConfig[];
  defaultConnectionId: string | null;
}>();

const emit = defineEmits<{
  close: [];
  restore: [path: string, connectionId: string];
}>();

const tab = ref<"jobs" | "history">("jobs");
const jobs = ref<BackupJob[]>([]);
const runs = ref<BackupRun[]>([]);
const loading = ref(false);
const error = ref("");
const editing = ref<BackupJob | null>(null);
const saving = ref(false);
const editorError = ref("");
const deleteTarget = ref<BackupJob | null>(null);
const runningJobId = ref("");
const operationId = ref("");
const cancelling = ref(false);
const progress = ref<DatabaseExportProgress | null>(null);
let unlisten: UnlistenFn | undefined;

const supportedConnections = computed(() =>
  props.connections.filter((connection) => connection.driver !== "redis"),
);

const missedJobs = computed(() => jobs.value.filter((job) => job.missed));
const busy = computed(() => Boolean(runningJobId.value));

function newJob(): BackupJob {
  const connection =
    supportedConnections.value.find(
      (item) => item.id === props.defaultConnectionId,
    ) ?? supportedConnections.value[0];
  return {
    id: "",
    name: "每日逻辑备份",
    connectionId: connection?.id ?? "",
    connectionName: connection?.name ?? "",
    connectionAvailable: Boolean(connection),
    database: connection?.database || null,
    schema: null,
    includeStructure: true,
    includeData: true,
    includeDrops: false,
    tableMode: "all",
    tablePattern: "",
    outputDir: "",
    filenameTemplate:
      "mydb-backup-{connection}-{database}-{timestamp}.sql",
    enabled: true,
    schedule: {
      kind: "daily",
      intervalHours: 1,
      hour: 2,
      minute: 0,
      weekday: 0,
    },
    retention: 5,
    nextRun: null,
    lastRun: null,
    lastStatus: null,
    lastError: null,
    missed: false,
  };
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", { hour12: false });
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function frequency(job: BackupJob): string {
  const { schedule } = job;
  const time = `${String(schedule.hour).padStart(2, "0")}:${String(
    schedule.minute,
  ).padStart(2, "0")}`;
  if (schedule.kind === "hourly") {
    return schedule.intervalHours === 1
      ? "每小时"
      : `每 ${schedule.intervalHours} 小时`;
  }
  if (schedule.kind === "weekly") {
    const days = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    return `${days[schedule.weekday] ?? "周一"} ${time}`;
  }
  return `每天 ${time}`;
}

function statusLabel(status: string | null): string {
  switch (status) {
    case "success":
      return "成功";
    case "failed":
      return "失败";
    case "cancelled":
      return "已取消";
    case "running":
      return "运行中";
    default:
      return "未运行";
  }
}

function scopeLabel(job: BackupJob): string {
  const parts = [job.database, job.schema].filter(Boolean);
  return parts.length ? parts.join(" · ") : "当前库 / 模式";
}

function templatePreview(job: BackupJob): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  const name = job.filenameTemplate
    .replaceAll("{connection}", job.connectionName || "connection")
    .replaceAll("{database}", job.database || "database")
    .replaceAll("{schema}", job.schema || "schema")
    .replace(
      "{date}",
      `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`,
    )
    .replace(
      "{time}",
      `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`,
    )
    .replace(
      "{timestamp}",
      `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(
        now.getHours(),
      )}${pad(now.getMinutes())}${pad(now.getSeconds())}`,
    );
  return `${job.outputDir || "（选择目录）"}/${name}`;
}

async function loadJobs() {
  jobs.value = await api.listBackupJobs();
}

async function loadRuns() {
  runs.value = await api.listBackupRuns();
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    await Promise.all([loadJobs(), loadRuns()]);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function startCreate() {
  tab.value = "jobs";
  editing.value = newJob();
  editorError.value = "";
}

function startEdit(job: BackupJob) {
  editing.value = structuredClone(job);
  editorError.value = "";
}

async function chooseDirectory() {
  if (!editing.value) return;
  const path = await openDirectory({ directory: true, multiple: false });
  if (typeof path === "string") editing.value.outputDir = path;
}

async function save() {
  if (!editing.value || saving.value) return;
  saving.value = true;
  editorError.value = "";
  try {
    await api.saveBackupJob(editing.value);
    editing.value = null;
    await loadJobs();
  } catch (reason) {
    editorError.value = String(reason);
  } finally {
    saving.value = false;
  }
}

async function toggleEnabled(job: BackupJob) {
  try {
    const updated = await api.saveBackupJob({
      ...structuredClone(job),
      enabled: !job.enabled,
    });
    const index = jobs.value.findIndex((item) => item.id === job.id);
    if (index >= 0) jobs.value[index] = updated;
  } catch (reason) {
    error.value = String(reason);
  }
}

async function removeJob() {
  const target = deleteTarget.value;
  deleteTarget.value = null;
  if (!target) return;
  try {
    await api.deleteBackupJob(target.id);
    await loadJobs();
  } catch (reason) {
    error.value = String(reason);
  }
}

async function run(job: BackupJob) {
  if (busy.value) return;
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `backup-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  runningJobId.value = job.id;
  operationId.value = id;
  cancelling.value = false;
  error.value = "";
  progress.value = null;
  try {
    await api.runBackupJob(job.id, id);
    await Promise.all([loadRuns(), loadJobs()]);
  } catch (reason) {
    error.value = String(reason);
    await Promise.all([loadRuns(), loadJobs()]);
  } finally {
    runningJobId.value = "";
    operationId.value = "";
    cancelling.value = false;
    progress.value = null;
  }
}

async function cancel() {
  if (!operationId.value || cancelling.value) return;
  cancelling.value = true;
  try {
    await api.cancelBackup(operationId.value);
  } catch (reason) {
    error.value = String(reason);
    cancelling.value = false;
  }
}

async function runMissed() {
  for (const job of [...missedJobs.value]) {
    await run(job);
  }
}

async function skipMissed() {
  try {
    jobs.value = await api.skipMissedBackups();
  } catch (reason) {
    error.value = String(reason);
  }
}

async function openDirectoryOf(job: BackupJob) {
  try {
    await openPath(job.outputDir);
  } catch (reason) {
    error.value = String(reason);
  }
}

function restore(run: BackupRun) {
  if (!run.path) return;
  emit("restore", run.path, run.connectionId);
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      tab.value = "jobs";
      editing.value = null;
      deleteTarget.value = null;
      error.value = "";
      void load();
    }
  },
  { immediate: true },
);

onMounted(async () => {
  unlisten = await listen<DatabaseExportProgress>(
    "export:progress",
    (event) => {
      if (!operationId.value) return;
      if (event.payload.operationId !== operationId.value) return;
      progress.value = event.payload;
    },
  );
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal backup-modal"
      role="dialog"
      aria-modal="true"
      aria-label="备份计划"
    >
      <header class="modal-header">
        <div>
          <h2>备份计划</h2>
          <p>仅在应用打开时自动执行逻辑 SQL 备份</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <p class="backup-banner">
        备份计划仅在 mydb 打开时执行；关闭应用、休眠或退出不会运行。不会安装系统服务，也不替代数据库原生 dump 或云备份。
      </p>

      <div class="backup-toolbar">
        <div class="segmented">
          <button
            class="segment"
            :class="{ active: tab === 'jobs' }"
            @click="tab = 'jobs'"
          >
            计划 {{ jobs.length }}
          </button>
          <button
            class="segment"
            :class="{ active: tab === 'history' }"
            @click="tab = 'history'"
          >
            历史 {{ runs.length }}
          </button>
        </div>
        <span class="spacer"></span>
        <button class="button ghost compact" :disabled="loading" @click="load">
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button
          class="button primary compact"
          :disabled="!supportedConnections.length"
          @click="startCreate"
        >
          <Icon name="plus" :size="14" />
          新建计划
        </button>
      </div>

      <div class="backup-body">
        <p v-if="error" class="data-message error">{{ error }}</p>

        <div v-if="missedJobs.length && tab === 'jobs'" class="backup-missed">
          <span>
            有 {{ missedJobs.length }} 个计划在应用关闭期间已错过运行时间，不会自动补跑。
          </span>
          <button
            class="button ghost compact"
            :disabled="busy"
            @click="runMissed"
          >
            立即补跑一次
          </button>
          <button class="button ghost compact" @click="skipMissed">
            跳过
          </button>
        </div>

        <div v-if="loading" class="panel-empty">正在读取备份计划…</div>

        <template v-else-if="tab === 'jobs'">
          <div v-if="!jobs.length" class="panel-empty">
            还没有备份计划。创建后会在应用运行时按计划导出 SQL。
          </div>
          <table v-else class="structure-table backup-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>连接 / 范围</th>
                <th>频率</th>
                <th>下次运行</th>
                <th>上次结果</th>
                <th>启用</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="job in jobs"
                :key="job.id"
                :class="{ 'backup-job-missed': job.missed }"
              >
                <td>
                  <strong>{{ job.name }}</strong>
                  <span v-if="job.missed" class="backup-badge">已错过</span>
                  <span
                    v-if="!job.connectionAvailable"
                    class="backup-badge danger"
                  >
                    连接不可用
                  </span>
                  <div class="backup-sub">
                    {{ job.includeStructure ? "结构" : "" }}
                    <template v-if="job.includeStructure && job.includeData"> + </template>
                    {{ job.includeData ? "数据" : "" }}
                    · 保留 {{ job.retention }} 份
                  </div>
                </td>
                <td>
                  <div>{{ job.connectionName || "—" }}</div>
                  <div class="backup-sub">{{ scopeLabel(job) }}</div>
                </td>
                <td>{{ frequency(job) }}</td>
                <td>{{ formatTime(job.nextRun) }}</td>
                <td>
                  <div>{{ statusLabel(job.lastStatus) }}</div>
                  <div class="backup-sub">{{ formatTime(job.lastRun) }}</div>
                  <div
                    v-if="job.lastError"
                    class="backup-error"
                    :title="job.lastError"
                  >
                    {{ job.lastError }}
                  </div>
                </td>
                <td>
                  <input
                    type="checkbox"
                    :checked="job.enabled"
                    :aria-label="`启用 ${job.name}`"
                    @change="toggleEnabled(job)"
                  />
                </td>
                <td class="backup-actions">
                  <button
                    v-if="runningJobId === job.id"
                    class="button ghost compact danger"
                    :disabled="cancelling"
                    @click="cancel"
                  >
                    {{ cancelling ? "停止中…" : "停止" }}
                  </button>
                  <button
                    v-else
                    class="button ghost compact"
                    :disabled="busy || !job.connectionAvailable"
                    @click="run(job)"
                  >
                    运行
                  </button>
                  <button
                    class="button ghost compact"
                    :disabled="busy"
                    @click="startEdit(job)"
                  >
                    编辑
                  </button>
                  <button
                    class="button ghost compact"
                    :disabled="!job.outputDir"
                    @click="openDirectoryOf(job)"
                  >
                    目录
                  </button>
                  <button
                    class="button ghost compact danger"
                    :disabled="busy"
                    @click="deleteTarget = job"
                  >
                    删除
                  </button>
                </td>
              </tr>
            </tbody>
          </table>

          <p v-if="progress" class="backup-progress">
            {{ progress.phase }} · {{ progress.objectName || "准备中" }}
            · {{ progress.rows }} 行 · {{ formatBytes(progress.bytes) }}
          </p>
        </template>

        <template v-else>
          <div v-if="!runs.length" class="panel-empty">
            还没有备份历史。
          </div>
          <table v-else class="structure-table backup-table">
            <thead>
              <tr>
                <th>计划</th>
                <th>状态</th>
                <th>开始</th>
                <th>耗时</th>
                <th>文件</th>
                <th>行 / 表</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="run in runs" :key="run.id">
                <td>
                  {{ run.jobName }}
                  <div class="backup-sub">{{ run.connectionName }}</div>
                </td>
                <td>
                  <span class="backup-status" :class="run.status">
                    {{ statusLabel(run.status) }}
                  </span>
                  <div v-if="run.error" class="backup-error" :title="run.error">
                    {{ run.error }}
                  </div>
                </td>
                <td>{{ formatTime(run.startedAt) }}</td>
                <td>
                  {{
                    run.finishedAt
                      ? `${Math.max(
                          0,
                          Math.round(
                            (new Date(run.finishedAt).getTime() -
                              new Date(run.startedAt).getTime()) /
                              1000,
                          ),
                        )} s`
                      : "—"
                  }}
                </td>
                <td class="backup-path" :title="run.path">
                  {{ run.path || "—" }}
                  <div v-if="run.bytes" class="backup-sub">
                    {{ formatBytes(run.bytes) }}
                  </div>
                </td>
                <td>{{ run.rows }} / {{ run.tables }}</td>
                <td class="backup-actions">
                  <button
                    class="button ghost compact"
                    :disabled="!run.path"
                    @click="openPath(run.path)"
                  >
                    打开位置
                  </button>
                  <button
                    class="button ghost compact"
                    :disabled="!run.path || run.status !== 'success'"
                    title="打开执行 SQL 文件对话框，不会自动执行"
                    @click="restore(run)"
                  >
                    恢复…
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </template>
      </div>

      <div v-if="editing" class="backup-editor-backdrop">
        <form class="backup-editor" @submit.prevent="save">
          <header class="modal-header">
            <h2>{{ editing.id ? "编辑备份计划" : "新建备份计划" }}</h2>
            <button
              type="button"
              class="icon-button"
              title="关闭"
              @click="editing = null"
            >
              <Icon name="close" />
            </button>
          </header>

          <div class="backup-form-grid">
            <label class="backup-field">
              <span>名称</span>
              <input v-model="editing.name" required maxlength="80" />
            </label>
            <label class="backup-field">
              <span>连接</span>
              <select v-model="editing.connectionId">
                <option
                  v-for="connection in supportedConnections"
                  :key="connection.id"
                  :value="connection.id"
                >
                  {{ connection.name }}（{{ connection.driver }}）
                </option>
              </select>
            </label>
            <label class="backup-field wide">
              <span>输出目录</span>
              <div class="backup-path-input">
                <input v-model="editing.outputDir" placeholder="/absolute/path" />
                <button
                  type="button"
                  class="button ghost compact"
                  @click="chooseDirectory"
                >
                  选择…
                </button>
              </div>
            </label>
            <label class="backup-field wide">
              <span>文件名模板</span>
              <input v-model="editing.filenameTemplate" />
              <small class="backup-hint">
                支持 {connection}、{database}、{schema}、{date}、{time}、{timestamp}；自动清理路径分隔符。
              </small>
            </label>
            <div class="backup-field wide">
              <span>内容</span>
              <div class="backup-checks">
                <label><input v-model="editing.includeStructure" type="checkbox" /> 结构</label>
                <label><input v-model="editing.includeData" type="checkbox" /> 数据</label>
                <label><input v-model="editing.includeDrops" type="checkbox" /> 包含 DROP IF EXISTS</label>
              </div>
            </div>
            <label class="backup-field">
              <span>表范围</span>
              <select v-model="editing.tableMode">
                <option value="all">全部表 / 视图</option>
                <option value="include">包含匹配</option>
                <option value="exclude">排除匹配</option>
              </select>
            </label>
            <label class="backup-field">
              <span>匹配规则</span>
              <input
                v-model="editing.tablePattern"
                :disabled="editing.tableMode === 'all'"
                placeholder="例如 order_* 或 user?"
              />
            </label>
            <label class="backup-field">
              <span>频率</span>
              <select v-model="editing.schedule.kind">
                <option value="hourly">每小时</option>
                <option value="daily">每天</option>
                <option value="weekly">每周</option>
              </select>
            </label>
            <label v-if="editing.schedule.kind === 'hourly'" class="backup-field">
              <span>间隔（小时）</span>
              <input
                v-model.number="editing.schedule.intervalHours"
                type="number"
                min="1"
                max="24"
              />
            </label>
            <template v-else>
              <label v-if="editing.schedule.kind === 'weekly'" class="backup-field">
                <span>星期</span>
                <select v-model.number="editing.schedule.weekday">
                  <option :value="0">周一</option>
                  <option :value="1">周二</option>
                  <option :value="2">周三</option>
                  <option :value="3">周四</option>
                  <option :value="4">周五</option>
                  <option :value="5">周六</option>
                  <option :value="6">周日</option>
                </select>
              </label>
              <label class="backup-field">
                <span>时间</span>
                <div class="backup-time">
                  <input
                    v-model.number="editing.schedule.hour"
                    type="number"
                    min="0"
                    max="23"
                  />
                  <span>:</span>
                  <input
                    v-model.number="editing.schedule.minute"
                    type="number"
                    min="0"
                    max="59"
                  />
                </div>
              </label>
            </template>
            <label class="backup-field">
              <span>保留最近成功备份</span>
              <input
                v-model.number="editing.retention"
                type="number"
                min="1"
                max="30"
              />
            </label>
            <label class="backup-field backup-enabled">
              <span>启用</span>
              <input v-model="editing.enabled" type="checkbox" />
            </label>
            <div class="backup-field wide">
              <span>预览路径</span>
              <code>{{ templatePreview(editing) }}</code>
            </div>
          </div>

          <p class="backup-hint">
            备份只读取当前连接的库 / 模式；输出纯 .sql，不做压缩、增量或物理备份。只读连接也可以备份。
          </p>
          <p v-if="editorError" class="message error">{{ editorError }}</p>

          <div class="confirm-actions">
            <button type="button" class="button ghost" @click="editing = null">
              取消
            </button>
            <button class="button primary" :disabled="saving">
              {{ saving ? "保存中…" : "保存计划" }}
            </button>
          </div>
        </form>
      </div>

      <ConfirmDialog
        :open="!!deleteTarget"
        title="删除备份计划"
        :message="`确定删除「${deleteTarget?.name ?? ''}」？`"
        detail="历史记录与已生成的备份文件不会被删除。"
        confirm-text="删除"
        danger
        @confirm="removeJob"
        @cancel="deleteTarget = null"
      />
    </div>
  </div>
</template>

<style scoped>
.backup-modal {
  width: min(1080px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  position: relative;
}

.backup-banner {
  margin: 0 20px;
  padding: 8px 10px;
  border: 1px solid var(--warning-border, rgba(210, 153, 34, 0.35));
  border-radius: 7px;
  background: rgba(210, 153, 34, 0.1);
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.5;
}

.backup-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.backup-body {
  min-height: 240px;
  max-height: 560px;
  overflow: auto;
  padding: 12px 20px 18px;
}

.backup-missed {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  padding: 7px 9px;
  border-radius: 7px;
  background: var(--accent-soft);
  color: var(--text-secondary);
  font-size: 11px;
}

.backup-missed span {
  flex: 1;
}

.backup-table {
  width: 100%;
}

.backup-table td {
  vertical-align: top;
}

.backup-sub {
  margin-top: 2px;
  color: var(--faint);
  font-size: 10px;
}

.backup-badge {
  display: inline-block;
  margin-left: 5px;
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 9px;
}

.backup-badge.danger {
  background: var(--danger-soft);
  color: var(--danger);
}

.backup-error {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--danger);
  font-size: 10px;
}

.backup-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 190px;
}

.backup-path {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 10px;
}

.backup-status.success {
  color: var(--success);
}

.backup-status.failed {
  color: var(--danger);
}

.backup-status.cancelled {
  color: var(--muted);
}

.backup-progress {
  margin: 10px 0 0;
  color: var(--muted);
  font-size: 11px;
}

.backup-job-missed {
  background: rgba(210, 153, 34, 0.06);
}

.backup-editor-backdrop {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: grid;
  place-items: center;
  padding: 18px;
  border-radius: inherit;
  background: rgba(0, 0, 0, 0.55);
}

.backup-editor {
  width: min(780px, 100%);
  max-height: calc(100vh - 100px);
  overflow: auto;
  padding-bottom: 16px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--panel);
  box-shadow: var(--shadow-overlay);
}

.backup-form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 14px;
  padding: 4px 20px 10px;
}

.backup-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--text-secondary);
  font-size: 11px;
}

.backup-field.wide {
  grid-column: 1 / -1;
}

.backup-field input,
.backup-field select {
  min-height: 30px;
  padding: 5px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 12px;
}

.backup-field code {
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 10px;
  overflow-wrap: anywhere;
}

.backup-path-input {
  display: flex;
  gap: 6px;
}

.backup-path-input input {
  flex: 1;
  min-width: 0;
}

.backup-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
}

.backup-checks label,
.backup-enabled {
  flex-direction: row;
  align-items: center;
  gap: 6px;
}

.backup-time {
  display: flex;
  align-items: center;
  gap: 5px;
}

.backup-time input {
  width: 64px;
}

.backup-hint {
  margin: 6px 20px;
  color: var(--faint);
  font-size: 10px;
  line-height: 1.5;
}

.backup-field .backup-hint {
  margin: 0;
}
</style>