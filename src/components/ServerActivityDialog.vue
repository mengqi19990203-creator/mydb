<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import ServerUsersPanel from "./ServerUsersPanel.vue";
import * as api from "../lib/api";
import type {
  Driver,
  ServerOverview,
  ServerProcess,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
  readOnly: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const tab = ref<"overview" | "processes" | "users">("overview");
const usersPanel = ref<InstanceType<typeof ServerUsersPanel> | null>(null);
const overview = ref<ServerOverview | null>(null);
const processes = ref<ServerProcess[]>([]);
const loading = ref(false);
const error = ref("");
const autoRefresh = ref<0 | 5 | 10>(0);
const terminateTarget = ref<ServerProcess | null>(null);
const terminating = ref(false);
let timer = 0;

const supported = computed(
  () => props.driver === "mysql" || props.driver === "postgres",
);

function formatUptime(value: string): string {
  const seconds = Number(value);
  if (!Number.isFinite(seconds) || seconds < 0) return value || "—";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = Math.floor(seconds % 60);
  if (days) return `${days} 天 ${hours} 小时`;
  if (hours) return `${hours} 小时 ${minutes} 分`;
  if (minutes) return `${minutes} 分 ${rest} 秒`;
  return `${rest} 秒`;
}

function metricValue(label: string, value: string): string {
  if (/uptime|运行时长/i.test(label)) return formatUptime(value);
  return value || "—";
}

function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatPercent(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(1)}%`;
}

function formatRate(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "采样中";
  return `${value.toFixed(1)} / s`;
}

function formatCount(value: number | null): string {
  if (value === null) return "—";
  return value.toLocaleString("zh-CN");
}

async function load() {
  if (!props.open || !supported.value) return;
  loading.value = true;
  error.value = "";
  try {
    const [overviewData, processData] = await Promise.all([
      api.serverOverview(props.connectionId),
      api.serverProcesses(props.connectionId),
    ]);
    overview.value = overviewData;
    processes.value = processData;
    if (tab.value === "users") {
      void usersPanel.value?.refresh();
    }
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function schedule() {
  window.clearInterval(timer);
  if (autoRefresh.value && props.open) {
    timer = window.setInterval(() => {
      if (!loading.value) void load();
    }, autoRefresh.value * 1000);
  }
}

function askTerminate(process: ServerProcess) {
  if (props.readOnly) return;
  terminateTarget.value = process;
}

async function confirmTerminate() {
  const target = terminateTarget.value;
  terminateTarget.value = null;
  if (!target || terminating.value) return;
  terminating.value = true;
  error.value = "";
  try {
    await api.terminateServerProcess(props.connectionId, target.id);
    await load();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    terminating.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      tab.value = "overview";
      autoRefresh.value = 0;
      void load();
    } else {
      window.clearInterval(timer);
    }
  },
  { immediate: true },
);

watch(autoRefresh, schedule);

onBeforeUnmount(() => {
  window.clearInterval(timer);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal server-activity-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-activity-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="server-activity-title">服务器活动</h2>
          <p>{{ scopeLabel }} · {{ driver === "mysql" ? "MySQL" : "PostgreSQL" }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="server-activity-toolbar">
        <div class="segmented">
          <button
            class="segment"
            :class="{ active: tab === 'overview' }"
            @click="tab = 'overview'"
          >
            概览
          </button>
          <button
            class="segment"
            :class="{ active: tab === 'processes' }"
            @click="tab = 'processes'"
          >
            进程 {{ processes.length }}
          </button>
          <button
            class="segment"
            :class="{ active: tab === 'users' }"
            @click="tab = 'users'"
          >
            用户 / 角色
          </button>
        </div>
        <div class="spacer"></div>
        <select v-model.number="autoRefresh" aria-label="自动刷新">
          <option :value="0">自动刷新：关闭</option>
          <option :value="5">每 5 秒</option>
          <option :value="10">每 10 秒</option>
        </select>
        <button class="button ghost compact" :disabled="loading" @click="load">
          <Icon name="refresh" :size="14" />
          刷新
        </button>
      </div>

      <div class="server-activity-body">
        <div v-if="loading" class="panel-empty">正在读取服务器状态…</div>
        <div v-else-if="error" class="data-message error">{{ error }}</div>

        <template v-else-if="tab === 'overview' && overview">
          <div class="server-overview-grid">
            <div class="server-overview-card">
              <span>版本</span>
              <strong :title="overview.version">{{ overview.version || "—" }}</strong>
            </div>
            <div class="server-overview-card">
              <span>当前用户</span>
              <strong>{{ overview.currentUser || "—" }}</strong>
            </div>
            <div class="server-overview-card">
              <span>当前数据库</span>
              <strong>{{ overview.currentDatabase || "—" }}</strong>
            </div>
            <div
              v-for="metric in overview.metrics"
              :key="metric.label"
              class="server-overview-card"
            >
              <span>{{ metric.label }}</span>
              <strong>{{ metricValue(metric.label, metric.value) }}</strong>
            </div>
            <div class="server-overview-card">
              <span>采样时间</span>
              <strong>{{ overview.sampledAt || "—" }}</strong>
            </div>
            <div v-if="overview.maxConnections !== null" class="server-overview-card">
              <span>最大连接数</span>
              <strong>{{ formatCount(overview.maxConnections) }}</strong>
            </div>
            <div
              v-if="overview.connectionUsagePercent !== null"
              class="server-overview-card"
            >
              <span>连接使用率</span>
              <strong>{{ formatPercent(overview.connectionUsagePercent) }}</strong>
            </div>
            <div v-if="overview.databaseSizeBytes !== null" class="server-overview-card">
              <span>当前库大小</span>
              <strong>{{ formatBytes(overview.databaseSizeBytes) }}</strong>
            </div>
            <div v-if="overview.tableCount !== null" class="server-overview-card">
              <span>当前库表数</span>
              <strong>{{ formatCount(overview.tableCount) }}</strong>
            </div>
            <div class="server-overview-card">
              <span>QPS</span>
              <strong>{{ formatRate(overview.qps) }}</strong>
            </div>
            <div
              v-if="overview.cacheHitPercent !== null"
              class="server-overview-card"
            >
              <span>缓存命中率</span>
              <strong>{{ formatPercent(overview.cacheHitPercent) }}</strong>
            </div>
            <div
              v-if="overview.activeSessions !== null"
              class="server-overview-card"
            >
              <span>活动会话</span>
              <strong>{{ formatCount(overview.activeSessions) }}</strong>
            </div>
            <div
              v-if="overview.idleSessions !== null"
              class="server-overview-card"
            >
              <span>空闲会话</span>
              <strong>{{ formatCount(overview.idleSessions) }}</strong>
            </div>
          </div>

          <section v-if="overview.topTables.length" class="server-top-tables">
            <h3>Top 5 表（按总大小）</h3>
            <table class="structure-table">
              <thead>
                <tr>
                  <th>表</th>
                  <th>总大小</th>
                  <th>数据</th>
                  <th>索引</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="table in overview.topTables" :key="table.name">
                  <td class="mono">{{ table.name }}</td>
                  <td>{{ formatBytes(table.totalBytes) }}</td>
                  <td>{{ formatBytes(table.dataBytes) }}</td>
                  <td>{{ formatBytes(table.indexBytes) }}</td>
                </tr>
              </tbody>
            </table>
          </section>
        </template>

        <ServerUsersPanel
          v-else-if="tab === 'users'"
          ref="usersPanel"
          :connection-id="connectionId"
          :driver="driver"
          active
        />

        <table v-else-if="tab === 'processes'" class="structure-table server-process-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>用户</th>
              <th>数据库</th>
              <th>状态</th>
              <th>耗时</th>
              <th>SQL</th>
              <th v-if="!readOnly">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="process in processes" :key="process.id">
              <td class="mono">{{ process.id }}</td>
              <td>{{ process.user || "—" }}</td>
              <td>{{ process.database || "—" }}</td>
              <td>{{ process.state || "—" }}</td>
              <td>{{ process.timeSeconds }} s</td>
              <td class="mono server-process-info" :title="process.info">
                {{ process.info || "—" }}
              </td>
              <td v-if="!readOnly">
                <button
                  class="button ghost compact danger"
                  :disabled="terminating"
                  @click="askTerminate(process)"
                >
                  终止
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <ConfirmDialog
        :open="!!terminateTarget"
        title="终止会话"
        :message="`确定终止进程 ${terminateTarget?.id ?? ''}？`"
        :detail="
          terminateTarget?.info
            ? `当前 SQL：${terminateTarget.info.slice(0, 300)}`
            : '该会话正在执行的语句会被中断。'
        "
        confirm-text="终止会话"
        @confirm="confirmTerminate"
        @cancel="terminateTarget = null"
      />
    </div>
  </div>
</template>

<style scoped>
.server-activity-modal {
  width: min(980px, calc(100vw - 48px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
}

.server-activity-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.server-activity-body {
  min-height: 220px;
  max-height: 540px;
  overflow: auto;
  padding: 16px 20px;
}

.server-overview-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 10px;
}

.server-overview-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.server-overview-card span {
  color: var(--muted);
  font-size: 11px;
}

.server-overview-card strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.server-process-table {
  width: 100%;
}

.server-process-info {
  max-width: 360px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.server-top-tables {
  margin-top: 16px;
}

.server-top-tables h3 {
  margin: 0 0 6px;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
}

.server-top-tables .structure-table {
  width: 100%;
}
</style>