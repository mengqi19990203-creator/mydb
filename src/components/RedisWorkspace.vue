<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { RedisKeyDetail, RedisKeyEntry } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  connectionName?: string;
  database?: string;
}>();

const databaseLabel = computed(() => {
  const database = (props.database ?? "0").trim() || "0";
  return /^\d+$/.test(database)
    ? `db${database.padStart(2, "0")}`
    : `db${database}`;
});

const pattern = ref("*");
const limit = ref(500);
const keys = ref<RedisKeyEntry[]>([]);
const scanned = ref(0);
const truncated = ref(false);
const scanning = ref(false);
const scanError = ref("");
const selectedKey = ref<string | null>(null);
const detail = ref<RedisKeyDetail | null>(null);
const detailLoading = ref(false);
const detailError = ref("");
const deleting = ref(false);
const deleteOpen = ref(false);
const pendingKey = ref("");
const riskyCommand = ref("");
const riskyWarning = ref("");
const command = ref("PING");
const output = ref("");
const commandError = ref("");
const commandRunning = ref(false);
const commandElapsed = ref(0);
const toast = ref("");
let detailRequest = 0;
let toastTimer = 0;

function flashToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 1800);
}

function ttlLabel(ttl: number): string {
  if (ttl === -1) return "永久";
  if (ttl === -2) return "不存在";
  return `${ttl} 秒`;
}

async function scan() {
  scanning.value = true;
  scanError.value = "";
  try {
    const result = await api.redisScanKeys(
      props.connectionId,
      pattern.value,
      limit.value,
    );
    keys.value = result.keys;
    scanned.value = result.scanned;
    truncated.value = result.truncated;
    if (
      selectedKey.value &&
      !keys.value.some((entry) => entry.key === selectedKey.value)
    ) {
      selectedKey.value = null;
      detail.value = null;
    }
  } catch (reason) {
    keys.value = [];
    scanned.value = 0;
    truncated.value = false;
    scanError.value = String(reason);
  } finally {
    scanning.value = false;
  }
}

async function openKey(key: string) {
  selectedKey.value = key;
  detail.value = null;
  detailError.value = "";
  detailLoading.value = true;
  const request = ++detailRequest;
  try {
    const result = await api.redisKeyDetail(props.connectionId, key);
    if (request !== detailRequest) return;
    detail.value = result;
  } catch (reason) {
    if (request !== detailRequest) return;
    detailError.value = String(reason);
  } finally {
    if (request === detailRequest) {
      detailLoading.value = false;
    }
  }
}

function removeKey() {
  if (!detail.value || deleting.value) return;
  pendingKey.value = detail.value.key;
  deleteOpen.value = true;
}

async function confirmRemoveKey() {
  const key = pendingKey.value;
  pendingKey.value = "";
  deleteOpen.value = false;
  if (!key || deleting.value) return;

  deleting.value = true;
  detailError.value = "";
  try {
    await api.redisDeleteKey(props.connectionId, key);
    selectedKey.value = null;
    detail.value = null;
    flashToast("已删除键");
    await scan();
  } catch (reason) {
    detailError.value = String(reason);
  } finally {
    deleting.value = false;
  }
}

function riskyWarningFor(line: string): string {
  const parts = line.trim().split(/\s+/);
  const head = parts[0]?.toUpperCase() ?? "";
  const second = parts[1]?.toUpperCase() ?? "";

  if (head === "FLUSHALL") {
    return "FLUSHALL 会清空当前 Redis 实例的全部数据库。";
  }
  if (head === "FLUSHDB") {
    return "FLUSHDB 会清空当前数据库的全部键。";
  }
  if (head === "SHUTDOWN") {
    return "SHUTDOWN 会关闭 Redis 服务；若未配置持久化，可能丢失数据。";
  }
  if (head === "DEBUG") {
    return "DEBUG 命令可能影响服务稳定性或触发大量输出。";
  }
  if (
    head === "CONFIG" &&
    ["SET", "RESETSTAT", "REWRITE"].includes(second)
  ) {
    return "CONFIG 命令会修改正在运行的 Redis 配置。";
  }
  if (head === "SCRIPT" && second === "FLUSH") {
    return "SCRIPT FLUSH 会清空脚本缓存，可能影响 EVALSHA 调用。";
  }
  if (head === "CLUSTER" && second === "RESET") {
    return "CLUSTER RESET 会重置集群节点状态。";
  }
  return "";
}

function executeCommand() {
  const line = command.value.trim();
  if (!line || commandRunning.value) return;

  const warning = riskyWarningFor(line);
  if (warning) {
    riskyCommand.value = line;
    riskyWarning.value = warning;
    return;
  }
  void runCommand(line);
}

async function confirmRiskyCommand() {
  const line = riskyCommand.value;
  riskyCommand.value = "";
  riskyWarning.value = "";
  if (line) await runCommand(line);
}

async function runCommand(line: string) {
  commandRunning.value = true;
  commandError.value = "";
  try {
    const result = await api.redisCommand(props.connectionId, line);
    output.value = result.output;
    commandElapsed.value = result.elapsedMs;
    if (result.truncated) {
      output.value += "\n\n(输出过长，已截断)";
    }
  } catch (reason) {
    commandError.value = String(reason);
  } finally {
    commandRunning.value = false;
  }
}

async function copyText(text: string, message: string) {
  try {
    await writeClipboard(text);
    flashToast(message);
  } catch {
    flashToast("复制失败");
  }
}

function reset() {
  keys.value = [];
  scanned.value = 0;
  truncated.value = false;
  scanError.value = "";
  selectedKey.value = null;
  detail.value = null;
  detailError.value = "";
  output.value = "";
  commandError.value = "";
  commandElapsed.value = 0;
  detailRequest += 1;
}

watch(
  () => props.connectionId,
  () => {
    reset();
    void scan();
  },
);

onMounted(() => {
  void scan();
});
</script>

<template>
  <section class="redis-workspace">
    <aside class="redis-key-panel">
      <div class="redis-panel-header">
        <span>
          键
          <span class="redis-db-badge" :title="`当前 Redis 数据库：${databaseLabel}`">
            {{ databaseLabel }}
          </span>
        </span>
        <span class="hint">
          {{ scanned }} 个已扫描 · {{ keys.length }} 个显示
        </span>
      </div>
      <div class="redis-key-toolbar">
        <input
          v-model="pattern"
          class="redis-pattern"
          placeholder="匹配模式，如 user:*"
          @keydown.enter="scan"
        />
        <button class="button ghost compact" :disabled="scanning" @click="scan">
          <Icon name="refresh" :size="14" />
          {{ scanning ? "扫描中…" : "扫描" }}
        </button>
      </div>

      <p v-if="scanError" class="redis-error">{{ scanError }}</p>

      <div class="redis-key-list">
        <button
          v-for="entry in keys"
          :key="entry.key"
          class="redis-key-item"
          :class="{ active: selectedKey === entry.key }"
          :title="entry.key"
          @click="openKey(entry.key)"
        >
          <span class="redis-key-name">{{ entry.key }}</span>
          <span class="redis-key-meta">
            <span class="redis-type">{{ entry.keyType }}</span>
            <span>{{ ttlLabel(entry.ttl) }}</span>
          </span>
        </button>
        <div v-if="!scanning && !keys.length" class="redis-empty">
          {{ scanError ? "扫描失败" : "没有匹配的键" }}
        </div>
      </div>

      <div class="redis-key-foot">
        <span v-if="truncated" class="hint">
          结果已截断，仅显示前 {{ keys.length }} 个键
        </span>
        <span v-else class="hint">
          连接：{{ connectionName }} · {{ databaseLabel }}
        </span>
      </div>
    </aside>

    <div class="redis-main">
      <div class="redis-detail">
        <div class="redis-detail-header">
          <span class="redis-detail-title" :title="selectedKey ?? undefined">
            {{ selectedKey ?? "键详情" }}
          </span>
          <span v-if="detail" class="redis-type">{{ detail.keyType }}</span>
          <span v-if="detail" class="hint">{{ ttlLabel(detail.ttl) }}</span>
          <div class="spacer"></div>
          <button
            v-if="selectedKey"
            class="button ghost compact"
            @click="copyText(selectedKey, '已复制键名')"
          >
            复制键名
          </button>
          <button
            v-if="selectedKey"
            class="button ghost compact redis-delete"
            :disabled="deleting"
            @click="removeKey"
          >
            {{ deleting ? "删除中…" : "删除键" }}
          </button>
        </div>

        <div class="redis-detail-body">
          <div v-if="detailLoading" class="redis-empty">读取中…</div>
          <p v-else-if="detailError" class="redis-error">{{ detailError }}</p>
          <pre v-else-if="detail" class="redis-value">{{ detail.value }}</pre>
          <div v-else class="redis-empty">
            从左侧选择一个键查看内容，或直接在下方执行命令
          </div>
          <p v-if="detail?.truncated" class="hint redis-truncated">
            内容较长，已截断显示。
          </p>
        </div>
      </div>

      <div class="redis-console">
        <div class="redis-console-header">
          <span>Redis 命令</span>
          <span v-if="commandElapsed" class="hint">{{ commandElapsed }} ms</span>
          <div class="spacer"></div>
          <button
            v-if="output"
            class="button ghost compact"
            @click="copyText(output, '已复制输出')"
          >
            复制输出
          </button>
          <button
            v-if="output"
            class="link-button redis-clear"
            @click="output = ''; commandError = ''"
          >
            清空
          </button>
        </div>
        <div class="redis-command-row">
          <input
            v-model="command"
            class="redis-command-input"
            placeholder="例如：GET user:1 或 HGETALL session:abc"
            @keydown.enter.prevent="executeCommand"
          />
          <button
            class="button primary compact"
            :disabled="commandRunning"
            @click="executeCommand"
          >
            {{ commandRunning ? "执行中…" : "执行" }}
          </button>
        </div>
        <p v-if="commandError" class="redis-error redis-command-error">
          {{ commandError }}
        </p>
        <pre
          v-else
          class="redis-output"
          :class="{ 'is-empty': !output }"
        >{{ output || "执行命令后在这里查看结果" }}</pre>
      </div>
    </div>

    <ConfirmDialog
      :open="!!riskyCommand"
      title="执行高风险命令"
      :message="riskyWarning"
      :detail="`${connectionName ?? ''} · ${databaseLabel}\n${riskyCommand}`"
      confirm-text="执行命令"
      @confirm="confirmRiskyCommand"
      @cancel="
        riskyCommand = '';
        riskyWarning = '';
      "
    />

    <ConfirmDialog
      :open="deleteOpen"
      title="删除键"
      :message="`确定删除键「${pendingKey}」？`"
      detail="此操作不可撤销。"
      confirm-text="删除键"
      @confirm="confirmRemoveKey"
      @cancel="deleteOpen = false"
    />

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>
  </section>
</template>
