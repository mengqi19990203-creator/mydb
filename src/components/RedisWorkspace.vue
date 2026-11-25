<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type {
  RedisDatabaseInfo,
  RedisKeyDetail,
  RedisKeyEntry,
} from "../lib/types";

const props = defineProps<{
  connectionId: string;
  connectionName?: string;
  database?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "database-change": [database: string];
}>();

function normalizeDatabase(value: string | null | undefined): number {
  const parsed = Number((value ?? "0").trim() || "0");
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : 0;
}

function databaseLabelFor(index: number): string {
  return `db${String(index).padStart(2, "0")}`;
}

const currentDatabase = ref(normalizeDatabase(props.database));
const databaseLabel = computed(() => databaseLabelFor(currentDatabase.value));
const dbOptions = ref<RedisDatabaseInfo[]>([]);
const clusterMode = ref(false);
const showDatabasePicker = computed(
  () => clusterMode.value || dbOptions.value.length > 1,
);
const databaseBadgeTitle = computed(() => {
  if (clusterMode.value) {
    return "Redis Cluster 仅支持 db0，点击查看";
  }
  return showDatabasePicker.value
    ? `当前 Redis 数据库：${databaseLabel.value}，点击切换`
    : `当前 Redis 数据库：${databaseLabel.value}`;
});
const dbMenuOpen = ref(false);
const dbLoading = ref(false);
const dbSwitching = ref(false);
const dbError = ref("");
const dbPicker = ref<HTMLElement | null>(null);

const pattern = ref("*");
const limit = ref(500);
const keys = ref<RedisKeyEntry[]>([]);
const scanned = ref(0);
const truncated = ref(false);
const cursor = ref(0);
const scanning = ref(false);
const loadingMore = ref(false);
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
const ttlInput = ref("60");
const renameInput = ref("");
const renameOverwriteOpen = ref(false);
const renameTargetName = ref("");
const mutationBusy = ref(false);
const mutationError = ref("");
const stringValue = ref("");
const hashDrafts = ref<Record<string, string>>({});
const newHashField = ref("");
const newHashValue = ref("");
const listDrafts = ref<Record<number, string>>({});
const listNewValue = ref("");
const listSide = ref<"left" | "right">("right");
const setNewMember = ref("");
const zsetDrafts = ref<Record<string, number>>({});
const zsetNewMember = ref("");
const zsetNewScore = ref(0);
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

async function scan(reset = true) {
  if (reset) {
    scanning.value = true;
  } else {
    loadingMore.value = true;
  }
  scanError.value = "";
  try {
    const result = await api.redisScanKeys(
      props.connectionId,
      pattern.value,
      limit.value,
      reset ? 0 : cursor.value,
    );
    keys.value = reset ? result.keys : [...keys.value, ...result.keys];
    scanned.value = reset ? result.scanned : scanned.value + result.scanned;
    cursor.value = result.cursor;
    truncated.value = result.cursor !== 0;
    if (
      selectedKey.value &&
      !keys.value.some((entry) => entry.key === selectedKey.value)
    ) {
      selectedKey.value = null;
      detail.value = null;
    }
  } catch (reason) {
    if (reset) {
      keys.value = [];
      scanned.value = 0;
      cursor.value = 0;
      truncated.value = false;
    }
    scanError.value = String(reason);
  } finally {
    scanning.value = false;
    loadingMore.value = false;
  }
}

async function loadMore() {
  if (loadingMore.value || scanning.value || cursor.value === 0) return;
  await scan(false);
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

async function saveTtl(permanent: boolean) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  let seconds: number | null = null;
  if (!permanent) {
    const parsed = Number(ttlInput.value);
    if (!Number.isInteger(parsed) || parsed < 0) {
      mutationError.value = "TTL 必须是非负整数";
      return;
    }
    seconds = parsed;
  }
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisSetTtl(props.connectionId, current.key, seconds);
    flashToast(permanent ? "已设为永久" : `TTL 已设为 ${seconds} 秒`);
    await openKey(current.key);
    await scan();
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function renameKey() {
  const current = detail.value;
  const next = renameInput.value.trim();
  if (!current || mutationBusy.value) return;
  if (!next) {
    mutationError.value = "新键名不能为空";
    return;
  }
  if (next === current.key) {
    mutationError.value = "新键名与原键名相同";
    return;
  }
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisRenameKey(props.connectionId, current.key, next, false);
    selectedKey.value = next;
    flashToast(`已重命名为 ${next}`);
    await scan();
    await openKey(next);
  } catch (reason) {
    const message = String(reason);
    if (message.includes("目标键已存在")) {
      renameTargetName.value = next;
      renameOverwriteOpen.value = true;
    } else {
      mutationError.value = message;
    }
  } finally {
    mutationBusy.value = false;
  }
}

async function confirmRenameOverwrite() {
  const current = detail.value;
  const next = renameTargetName.value;
  renameOverwriteOpen.value = false;
  if (!current || !next) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisRenameKey(props.connectionId, current.key, next, true);
    selectedKey.value = next;
    flashToast(`已覆盖并重命名为 ${next}`);
    await scan();
    await openKey(next);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function saveString() {
  const current = detail.value;
  if (!current || mutationBusy.value || current.truncated) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisSetString(
      props.connectionId,
      current.key,
      stringValue.value,
    );
    flashToast("字符串值已保存（TTL 保持不变）");
    await openKey(current.key);
    await scan();
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function saveHashField(field: string) {
  const current = detail.value;
  if (!current || mutationBusy.value || !field) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisHashSet(
      props.connectionId,
      current.key,
      field,
      hashDrafts.value[field] ?? "",
    );
    flashToast(`字段 ${field} 已保存`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function addHashField() {
  const current = detail.value;
  const field = newHashField.value.trim();
  if (!current || mutationBusy.value || !field) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisHashSet(
      props.connectionId,
      current.key,
      field,
      newHashValue.value,
    );
    newHashField.value = "";
    newHashValue.value = "";
    flashToast(`已新增字段 ${field}`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function deleteHashField(field: string) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisHashDeleteField(props.connectionId, current.key, field);
    flashToast(`已删除字段 ${field}`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function saveListEntry(index: number) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisListSet(
      props.connectionId,
      current.key,
      index,
      listDrafts.value[index] ?? "",
    );
    flashToast(`已保存第 ${index + 1} 个元素`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function removeListEntry(value: string) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisListRemove(props.connectionId, current.key, value);
    flashToast("已删除第一个匹配的元素");
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function addListEntry() {
  const current = detail.value;
  if (!current || mutationBusy.value || !listNewValue.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisListPush(
      props.connectionId,
      current.key,
      listNewValue.value,
      listSide.value === "left",
    );
    listNewValue.value = "";
    flashToast(listSide.value === "left" ? "已从左侧插入" : "已从右侧插入");
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function addSetMember() {
  const current = detail.value;
  const member = setNewMember.value;
  if (!current || mutationBusy.value || !member) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisSetAdd(props.connectionId, current.key, member);
    setNewMember.value = "";
    flashToast("已添加成员");
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function removeSetMember(member: string) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisSetRemove(props.connectionId, current.key, member);
    flashToast(`已移除成员 ${member}`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function saveZsetMember(member: string) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  const score = Number(zsetDrafts.value[member]);
  if (!Number.isFinite(score)) {
    mutationError.value = "score 必须是有限数字";
    return;
  }
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisZsetSet(props.connectionId, current.key, member, score);
    flashToast(`已更新成员 ${member} 的 score`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function addZsetMember() {
  const current = detail.value;
  const member = zsetNewMember.value;
  const score = Number(zsetNewScore.value);
  if (!current || mutationBusy.value || !member) return;
  if (!Number.isFinite(score)) {
    mutationError.value = "score 必须是有限数字";
    return;
  }
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisZsetSet(props.connectionId, current.key, member, score);
    zsetNewMember.value = "";
    zsetNewScore.value = 0;
    flashToast("已添加 ZSet 成员");
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

async function removeZsetMember(member: string) {
  const current = detail.value;
  if (!current || mutationBusy.value) return;
  mutationBusy.value = true;
  mutationError.value = "";
  try {
    await api.redisZsetRemove(props.connectionId, current.key, member);
    flashToast(`已移除成员 ${member}`);
    await openKey(current.key);
  } catch (reason) {
    mutationError.value = String(reason);
  } finally {
    mutationBusy.value = false;
  }
}

watch(
  detail,
  (value) => {
    mutationError.value = "";
    if (!value) return;
    ttlInput.value = value.ttl > 0 ? String(value.ttl) : "60";
    renameInput.value = value.key;
    stringValue.value = value.keyType === "string" ? value.value : "";
    const drafts: Record<string, string> = {};
    for (const entry of value.hash ?? []) {
      drafts[entry.field] = entry.value;
    }
    hashDrafts.value = drafts;
    newHashField.value = "";
    newHashValue.value = "";
    const listDraftValues: Record<number, string> = {};
    for (const entry of value.listEntries ?? []) {
      listDraftValues[entry.index] = entry.value;
    }
    listDrafts.value = listDraftValues;
    listNewValue.value = "";
    setNewMember.value = "";
    const zsetDraftValues: Record<string, number> = {};
    for (const entry of value.zsetEntries ?? []) {
      zsetDraftValues[entry.member] = entry.score;
    }
    zsetDrafts.value = zsetDraftValues;
    zsetNewMember.value = "";
    zsetNewScore.value = 0;
  },
);

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

async function loadDatabases() {
  dbLoading.value = true;
  dbError.value = "";
  try {
    const result = await api.redisDatabases(props.connectionId);
    dbOptions.value = result.databases;
    clusterMode.value = result.cluster;
    if (result.cluster && currentDatabase.value !== 0) {
      currentDatabase.value = 0;
      emit("database-change", "0");
    }
    if (!showDatabasePicker.value) {
      dbMenuOpen.value = false;
    }
  } catch (reason) {
    dbError.value = String(reason);
  } finally {
    dbLoading.value = false;
  }
}

function toggleDatabaseMenu() {
  if (dbSwitching.value) return;
  dbMenuOpen.value = !dbMenuOpen.value;
  if (dbMenuOpen.value) {
    void loadDatabases();
  }
}

async function applyDatabase(index: number, notify = true) {
  if (dbSwitching.value) return;
  dbSwitching.value = true;
  dbError.value = "";
  try {
    const selected = await api.redisSelectDatabase(
      props.connectionId,
      index,
    );
    currentDatabase.value = selected;
    dbMenuOpen.value = false;
    if (notify || selected !== index) {
      emit("database-change", String(selected));
    }
    if (notify) {
      flashToast(`已切换到 ${databaseLabelFor(selected)}`);
    }
    reset();
    await Promise.all([scan(), loadDatabases()]);
  } catch (reason) {
    dbError.value = String(reason);
    scanError.value = String(reason);
  } finally {
    dbSwitching.value = false;
  }
}

function chooseDatabase(index: number) {
  if (index === currentDatabase.value) {
    dbMenuOpen.value = false;
    return;
  }
  void applyDatabase(index);
}

function onDocumentMouseDown(event: MouseEvent) {
  if (!dbMenuOpen.value) return;
  const target = event.target as Node | null;
  if (target && dbPicker.value?.contains(target)) return;
  dbMenuOpen.value = false;
}

function reset() {
  keys.value = [];
  scanned.value = 0;
  truncated.value = false;
  cursor.value = 0;
  scanError.value = "";
  selectedKey.value = null;
  detail.value = null;
  detailError.value = "";
  output.value = "";
  commandError.value = "";
  commandElapsed.value = 0;
  mutationError.value = "";
  detailRequest += 1;
}

watch(
  () => props.connectionId,
  () => {
    currentDatabase.value = normalizeDatabase(props.database);
    reset();
    void applyDatabase(currentDatabase.value, false);
  },
);

watch(
  () => props.database,
  (value) => {
    const next = normalizeDatabase(value);
    if (next === currentDatabase.value) return;
    currentDatabase.value = next;
    reset();
    void applyDatabase(next, false);
  },
);

onMounted(() => {
  window.addEventListener("mousedown", onDocumentMouseDown);
  void applyDatabase(currentDatabase.value, false);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousedown", onDocumentMouseDown);
});
</script>

<template>
  <section class="redis-workspace">
    <aside class="redis-key-panel">
      <div class="redis-panel-header">
        <span class="redis-db-control">
          键
          <template v-if="showDatabasePicker">
            <span ref="dbPicker" class="redis-db-picker">
              <button
                class="redis-db-badge redis-db-button"
                :title="databaseBadgeTitle"
                :disabled="dbSwitching"
                aria-haspopup="menu"
                :aria-expanded="dbMenuOpen"
                @click.stop="toggleDatabaseMenu"
                @keydown.esc.stop="dbMenuOpen = false"
              >
                {{ databaseLabel }}
                <Icon name="chevron-down" :size="10" />
              </button>
              <div v-if="dbMenuOpen" class="redis-db-menu" role="menu">
                <div v-if="dbLoading" class="redis-db-menu-hint">
                  加载数据库…
                </div>
                <div
                  v-else-if="clusterMode"
                  class="redis-db-menu-hint"
                >
                  Redis Cluster 仅支持 db0
                </div>
                <button
                  v-for="option in dbOptions"
                  :key="option.index"
                  class="redis-db-option"
                  :class="{ active: option.index === currentDatabase }"
                  role="menuitem"
                  @click="chooseDatabase(option.index)"
                >
                  <span>{{ databaseLabelFor(option.index) }}</span>
                  <span
                    v-if="option.keys !== null"
                    class="redis-db-keys"
                  >
                    {{ option.keys }} 个键
                  </span>
                </button>
                <p v-if="dbError" class="redis-db-menu-error">{{ dbError }}</p>
              </div>
            </span>
          </template>
          <span
            v-else
            class="redis-db-badge"
            :title="databaseBadgeTitle"
          >
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
          @keydown.enter="scan(true)"
        />
        <button class="button ghost compact" :disabled="scanning" @click="scan(true)">
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
        <button
          v-if="cursor !== 0"
          class="button ghost compact"
          :disabled="loadingMore || scanning"
          @click="loadMore"
        >
          {{ loadingMore ? "加载中…" : "加载更多" }}
        </button>
        <span v-else-if="keys.length" class="hint">已全部加载</span>
        <span class="hint">
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
            v-if="selectedKey && !readOnly"
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
          <template v-else-if="detail">
            <pre class="redis-value">{{ detail.value }}</pre>
            <p v-if="detail?.truncated" class="hint redis-truncated">
              内容较长，已截断显示。
            </p>
            <p v-if="mutationError" class="redis-error">{{ mutationError }}</p>

            <template v-if="!readOnly">
              <div class="redis-edit-section">
                <strong>TTL</strong>
                <span class="hint">当前：{{ ttlLabel(detail.ttl) }}</span>
                <input
                  v-model="ttlInput"
                  type="number"
                  min="0"
                  class="redis-ttl-input"
                  :disabled="mutationBusy"
                />
                <span class="hint">秒</span>
                <button class="button ghost compact" :disabled="mutationBusy" @click="saveTtl(false)">
                  设置
                </button>
                <button class="button ghost compact" :disabled="mutationBusy" @click="saveTtl(true)">
                  永久
                </button>
              </div>

              <div class="redis-edit-section">
                <strong>重命名</strong>
                <input
                  v-model="renameInput"
                  class="redis-rename-input"
                  placeholder="新键名"
                  :disabled="mutationBusy"
                />
                <button class="button ghost compact" :disabled="mutationBusy" @click="renameKey">
                  重命名
                </button>
              </div>

              <div v-if="detail.keyType === 'string'" class="redis-edit-block">
                <strong>编辑字符串值</strong>
                <textarea
                  v-model="stringValue"
                  rows="6"
                  class="redis-value-editor"
                  :disabled="mutationBusy || detail.truncated"
                  placeholder="值"
                ></textarea>
                <p v-if="detail.truncated" class="hint">
                  内容已被截断，为避免覆盖完整值，禁止在此编辑；请使用命令控制台。
                </p>
                <button
                  class="button primary compact"
                  :disabled="mutationBusy || detail.truncated"
                  @click="saveString"
                >
                  保存字符串
                </button>
              </div>

              <div v-if="detail.keyType === 'hash'" class="redis-edit-block">
                <strong>Hash 字段</strong>
                <div
                  v-for="entry in detail.hash"
                  :key="entry.field"
                  class="redis-hash-row"
                >
                  <code>{{ entry.field }}</code>
                  <input
                    v-model="hashDrafts[entry.field]"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy" @click="saveHashField(entry.field)">
                    保存
                  </button>
                  <button class="button ghost compact danger" :disabled="mutationBusy" @click="deleteHashField(entry.field)">
                    删除
                  </button>
                </div>
                <div class="redis-hash-row">
                  <input
                    v-model="newHashField"
                    placeholder="新字段"
                    :disabled="mutationBusy"
                  />
                  <input
                    v-model="newHashValue"
                    placeholder="值"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy || !newHashField.trim()" @click="addHashField">
                    新增字段
                  </button>
                </div>
              </div>

              <div v-if="detail.keyType === 'list'" class="redis-edit-block">
                <strong>List 元素</strong>
                <div
                  v-for="entry in detail.listEntries"
                  :key="entry.index"
                  class="redis-hash-row"
                >
                  <code>#{{ entry.index + 1 }}</code>
                  <input
                    v-model="listDrafts[entry.index]"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy" @click="saveListEntry(entry.index)">
                    保存
                  </button>
                  <button class="button ghost compact danger" :disabled="mutationBusy" @click="removeListEntry(entry.value)">
                    删除
                  </button>
                </div>
                <div class="redis-hash-row">
                  <select v-model="listSide" :disabled="mutationBusy">
                    <option value="right">右侧</option>
                    <option value="left">左侧</option>
                  </select>
                  <input
                    v-model="listNewValue"
                    placeholder="新元素值"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy || !listNewValue" @click="addListEntry">
                    插入元素
                  </button>
                </div>
                <p class="hint">删除按值移除第一个匹配元素；如需精确控制顺序请使用命令控制台。</p>
              </div>

              <div v-if="detail.keyType === 'set'" class="redis-edit-block">
                <strong>Set 成员</strong>
                <div
                  v-for="member in detail.setMembers"
                  :key="member"
                  class="redis-hash-row"
                >
                  <code>{{ member }}</code>
                  <span class="spacer"></span>
                  <button class="button ghost compact danger" :disabled="mutationBusy" @click="removeSetMember(member)">
                    删除
                  </button>
                </div>
                <div class="redis-hash-row">
                  <input
                    v-model="setNewMember"
                    placeholder="新成员"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy || !setNewMember" @click="addSetMember">
                    添加成员
                  </button>
                </div>
              </div>

              <div v-if="detail.keyType === 'zset'" class="redis-edit-block">
                <strong>ZSet 成员</strong>
                <div
                  v-for="entry in detail.zsetEntries"
                  :key="entry.member"
                  class="redis-hash-row"
                >
                  <code>{{ entry.member }}</code>
                  <input
                    v-model.number="zsetDrafts[entry.member]"
                    type="number"
                    step="any"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy" @click="saveZsetMember(entry.member)">
                    保存 score
                  </button>
                  <button class="button ghost compact danger" :disabled="mutationBusy" @click="removeZsetMember(entry.member)">
                    删除
                  </button>
                </div>
                <div class="redis-hash-row">
                  <input
                    v-model="zsetNewMember"
                    placeholder="新成员"
                    :disabled="mutationBusy"
                  />
                  <input
                    v-model.number="zsetNewScore"
                    type="number"
                    step="any"
                    placeholder="score"
                    :disabled="mutationBusy"
                  />
                  <button class="button ghost compact" :disabled="mutationBusy || !zsetNewMember" @click="addZsetMember">
                    添加成员
                  </button>
                </div>
              </div>
            </template>
            <p v-else class="hint">只读连接：键内容可查看，TTL / 重命名 / 值编辑已禁用。</p>
          </template>
          <div v-else class="redis-empty">
            从左侧选择一个键查看内容，或直接在下方执行命令
          </div>
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

    <ConfirmDialog
      :open="renameOverwriteOpen"
      title="覆盖目标键"
      :message="`目标键「${renameTargetName}」已存在，确定覆盖？`"
      detail="覆盖后原目标键的值会被重命名后的键替换，此操作不可撤销。"
      confirm-text="覆盖并重命名"
      danger
      @confirm="confirmRenameOverwrite"
      @cancel="renameOverwriteOpen = false"
    />

    <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
      {{ toast }}
    </div>
  </section>
</template>
