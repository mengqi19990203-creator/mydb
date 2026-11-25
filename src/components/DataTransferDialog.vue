<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { useSyncScope } from "../lib/sync-scope";
import type {
  ConnectionConfig,
  TableInfo,
  TransferProgress,
  TransferResult,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connections: ConnectionConfig[];
  currentConnectionId: string;
}>();

const emit = defineEmits<{
  close: [];
  transferred: [result: TransferResult];
}>();

const sourceId = ref("");
const targetId = ref("");
const sourceTables = ref<TableInfo[]>([]);
const selected = ref<Set<string>>(new Set());
const loading = ref(false);
const transferring = ref(false);
const error = ref("");
const result = ref<TransferResult | null>(null);
const progress = ref<TransferProgress | null>(null);
const mode = ref<"append" | "upsert" | "truncate">("append");
const createMissing = ref(false);
const confirmTruncate = ref(false);
const cancelRequested = ref(false);
const operationId = ref("");
let loadToken = 0;
let unlisten: UnlistenFn | undefined;

const {
  sourceDatabases,
  sourceSchemas,
  sourceDatabase,
  sourceSchema,
  targetOptions,
  targetDatabases,
  targetSchemas,
  targetDatabase,
  targetSchema,
  sourceScopeValid,
  targetScopeValid,
  targetScopeEqualsSource,
  loading: scopeLoading,
  refresh: refreshScope,
} = useSyncScope({
  connections: computed(() => props.connections),
  sourceId,
  targetId,
  onError: (value) => {
    error.value = value;
  },
});

const targetConnection = computed(
  () => props.connections.find((item) => item.id === targetId.value) ?? null,
);
const targetReadOnly = computed(() => Boolean(targetConnection.value?.readOnly));
const sameTarget = computed(
  () =>
    sourceId.value === targetId.value &&
    targetScopeEqualsSource.value &&
    Boolean(sourceDatabase.value || sourceSchema.value),
);
const availableTables = computed(() =>
  sourceTables.value.filter((table) => table.kind === "table"),
);
const selectedTables = computed(() =>
  availableTables.value.filter((table) => selected.value.has(table.name)),
);
const canTransfer = computed(
  () =>
    !loading.value &&
    !transferring.value &&
    sourceScopeValid.value &&
    targetScopeValid.value &&
    selectedTables.value.length > 0 &&
    !targetReadOnly.value &&
    !sameTarget.value &&
    (mode.value !== "truncate" || confirmTruncate.value),
);

async function loadSourceTables() {
  if (!sourceId.value || !sourceScopeValid.value) {
    sourceTables.value = [];
    selected.value = new Set();
    return;
  }
  const token = ++loadToken;
  loading.value = true;
  error.value = "";
  try {
    const tables = await api.listContextTables(
      sourceId.value,
      sourceDatabase.value || null,
      sourceSchema.value || null,
    );
    if (token !== loadToken) return;
    sourceTables.value = tables;
    selected.value = new Set(
      tables.filter((table) => table.kind === "table").map((table) => table.name),
    );
  } catch (reason) {
    if (token === loadToken) {
      sourceTables.value = [];
      selected.value = new Set();
      error.value = String(reason);
    }
  } finally {
    if (token === loadToken) loading.value = false;
  }
}

function toggleTable(name: string) {
  if (transferring.value) return;
  const next = new Set(selected.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  selected.value = next;
}

function toggleAll() {
  if (transferring.value) return;
  selected.value =
    selected.value.size === availableTables.value.length
      ? new Set()
      : new Set(availableTables.value.map((table) => table.name));
}

async function transfer() {
  if (!canTransfer.value) return;
  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `transfer-${Date.now()}`;
  operationId.value = id;
  transferring.value = true;
  cancelRequested.value = false;
  error.value = "";
  result.value = null;
  progress.value = null;
  try {
    const outcome = await api.transferTableData(
      sourceId.value,
      targetId.value,
      sourceDatabase.value || null,
      sourceSchema.value || null,
      targetDatabase.value || null,
      targetSchema.value || null,
      selectedTables.value.map((table) => ({
        sourceTable: table.name,
        targetTable: table.name,
      })),
      { mode: mode.value, createMissing: createMissing.value },
      id,
    );
    result.value = outcome;
    emit("transferred", outcome);
  } catch (reason) {
    error.value = cancelRequested.value ? "传输已取消" : String(reason);
  } finally {
    transferring.value = false;
    cancelRequested.value = false;
    operationId.value = "";
  }
}

async function cancelTransfer() {
  if (!transferring.value || !operationId.value) return;
  cancelRequested.value = true;
  try {
    await api.cancelDataTransfer(operationId.value);
  } catch (reason) {
    error.value = String(reason);
  }
}

function requestClose() {
  if (transferring.value) return;
  emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    sourceId.value =
      props.connections.find(
        (item) => item.id === props.currentConnectionId,
      )?.id ??
      props.connections.find((item) => item.driver !== "redis")?.id ??
      "";
    targetId.value = "";
    sourceDatabase.value = "";
    sourceSchema.value = "";
    targetDatabase.value = "";
    targetSchema.value = "";
    selected.value = new Set();
    sourceTables.value = [];
    error.value = "";
    result.value = null;
    progress.value = null;
    mode.value = "append";
    createMissing.value = false;
    confirmTruncate.value = false;
    void refreshScope().then(() => {
      targetId.value = targetOptions.value[0]?.id ?? "";
      void loadSourceTables();
    });
  },
  { immediate: true },
);

watch(
  [sourceId, sourceDatabase, sourceSchema],
  () => {
    result.value = null;
    void loadSourceTables();
  },
);

watch([targetId, targetDatabase, targetSchema], () => {
  result.value = null;
});

onMounted(async () => {
  unlisten = await listen<TransferProgress>(
    "data-transfer:progress",
    (event) => {
      if (event.payload.operationId === operationId.value) {
        progress.value = event.payload;
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
      class="modal transfer-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="transfer-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="transfer-title">数据传输</h2>
          <p>同类型数据库之间按同名字段复制表数据</p>
        </div>
        <button
          class="icon-button"
          title="关闭"
          :disabled="transferring"
          @click="requestClose"
        >
          <Icon name="close" />
        </button>
      </header>

      <div class="transfer-body">
        <section class="transfer-scope">
          <div class="form-grid">
            <label>
              源连接
              <select v-model="sourceId">
                <option
                  v-for="connection in connections.filter(
                    (item) => item.driver !== 'redis',
                  )"
                  :key="connection.id"
                  :value="connection.id"
                >
                  {{ connection.name }} · {{ connection.driver }}
                </option>
              </select>
            </label>
            <label>
              目标连接
              <select v-model="targetId">
                <option
                  v-for="connection in targetOptions"
                  :key="connection.id"
                  :value="connection.id"
                >
                  {{ connection.name }} · {{ connection.driver }}
                </option>
              </select>
            </label>
            <label v-if="sourceDatabases.length">
              源数据库
              <select v-model="sourceDatabase">
                <option
                  v-for="database in sourceDatabases"
                  :key="database"
                  :value="database"
                >
                  {{ database }}
                </option>
              </select>
            </label>
            <label v-if="sourceSchemas.length">
              源模式
              <select v-model="sourceSchema">
                <option
                  v-for="schema in sourceSchemas"
                  :key="schema"
                  :value="schema"
                >
                  {{ schema }}
                </option>
              </select>
            </label>
            <label v-if="targetDatabases.length">
              目标数据库
              <select v-model="targetDatabase">
                <option
                  v-for="database in targetDatabases"
                  :key="database"
                  :value="database"
                >
                  {{ database }}
                </option>
              </select>
            </label>
            <label v-if="targetSchemas.length">
              目标模式
              <select v-model="targetSchema">
                <option
                  v-for="schema in targetSchemas"
                  :key="schema"
                  :value="schema"
                >
                  {{ schema }}
                </option>
              </select>
            </label>
          </div>
          <p v-if="scopeLoading" class="draft-hint">正在读取数据库 / 模式…</p>
          <p v-else-if="!targetOptions.length" class="draft-hint">
            没有可用的同类型目标连接。
          </p>
          <p v-else-if="sameTarget" class="draft-hint">
            源和目标范围相同，不能传输到自身。
          </p>
          <p v-else-if="targetReadOnly" class="message error">
            目标连接为只读模式，无法写入。
          </p>
        </section>

        <section class="transfer-tables">
          <div class="transfer-section-head">
            <strong>表</strong>
            <span class="hint">
              已选 {{ selected.size }} / {{ availableTables.length }}
            </span>
            <button
              class="button ghost compact"
              :disabled="!availableTables.length || transferring"
              @click="toggleAll"
            >
              {{ selected.size === availableTables.length ? "全不选" : "全选" }}
            </button>
          </div>
          <div v-if="loading" class="panel-empty compact">正在读取表…</div>
          <div v-else-if="!availableTables.length" class="panel-empty compact">
            当前源范围没有表
          </div>
          <div v-else class="transfer-table-list">
            <label
              v-for="table in availableTables"
              :key="table.name"
              class="transfer-table-row"
            >
              <input
                type="checkbox"
                :checked="selected.has(table.name)"
                :disabled="transferring"
                @change="toggleTable(table.name)"
              />
              <Icon name="table" :size="13" />
              <span :title="table.name">{{ table.name }}</span>
              <span class="transfer-target-name">→ 同名表</span>
            </label>
          </div>
        </section>

        <section class="transfer-options">
          <label>
            传输模式
            <select v-model="mode">
              <option value="append">仅追加（INSERT）</option>
              <option value="upsert">按主键新增 / 更新</option>
              <option value="truncate">先清空目标表再写入（危险）</option>
            </select>
          </label>
          <label class="checkbox-row">
            <input v-model="createMissing" type="checkbox" :disabled="transferring" />
            目标表不存在时按源表结构创建
          </label>
          <label v-if="mode === 'truncate'" class="checkbox-row danger">
            <input v-model="confirmTruncate" type="checkbox" />
            我确认先清空目标表
          </label>
          <p class="draft-hint">
            第一版按同名字段映射，不提供逐列映射；同类型数据库，目标表需有主键才能使用 upsert。
          </p>

          <div v-if="transferring" class="transfer-progress">
            <span>
              {{ progress?.message ?? "准备传输…" }} · 表
              {{ progress?.tableIndex ?? 0 }} / {{ progress?.tableTotal ?? selected.size }}
            </span>
            <span class="hint">已传输 {{ progress?.rows ?? 0 }} 行</span>
            <button
              class="button ghost compact"
              :disabled="cancelRequested"
              @click="cancelTransfer"
            >
              {{ cancelRequested ? "正在取消…" : "取消传输" }}
            </button>
          </div>

          <div v-if="result" class="transfer-result">
            <p class="message success">
              {{ result.cancelled ? "传输已取消" : "传输完成" }} · 已传输
              {{ result.tables }} 张表 · {{ result.rows }} 行
            </p>
            <div v-if="result.failures.length" class="transfer-failures">
              <p v-for="failure in result.failures" :key="failure.table">
                {{ failure.table }}：{{ failure.message }}
              </p>
            </div>
          </div>

          <p v-if="error" class="message error">{{ error }}</p>
        </section>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" :disabled="transferring" @click="requestClose">
          关闭
        </button>
        <div class="spacer"></div>
        <button
          class="button primary"
          :disabled="!canTransfer"
          @click="transfer"
        >
          {{ transferring ? "传输中…" : `传输 ${selectedTables.length} 张表` }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.transfer-modal {
  width: min(900px, calc(100vw - 48px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
}

.transfer-body {
  min-height: 0;
  overflow-y: auto;
  padding: 0 20px 10px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.transfer-scope,
.transfer-tables,
.transfer-options {
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.transfer-section-head,
.transfer-table-row,
.transfer-progress {
  display: flex;
  align-items: center;
  gap: 8px;
}

.transfer-section-head {
  justify-content: flex-start;
  margin-bottom: 8px;
}

.transfer-section-head .hint {
  flex: 1;
}

.transfer-table-list {
  max-height: 240px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: var(--surface-well);
}

.transfer-table-row {
  padding: 4px 8px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 12px;
}

.transfer-table-row:last-child {
  border-bottom: none;
}

.transfer-table-row span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-target-name {
  margin-left: auto;
  color: var(--muted);
  font-size: 10px;
}

.transfer-options {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 18px;
}

.transfer-options .draft-hint,
.transfer-options .transfer-progress,
.transfer-options .transfer-result,
.transfer-options .message {
  flex-basis: 100%;
}

.transfer-progress {
  flex-wrap: wrap;
  padding: 8px;
  border-radius: 6px;
  background: var(--surface-well);
  font-size: 11px;
}

.transfer-progress .hint {
  flex: 1;
}

.transfer-failures {
  color: var(--danger);
  font-size: 11px;
}

.transfer-failures p {
  margin: 3px 0;
}

.checkbox-row.danger,
.message.error {
  color: var(--danger);
}
</style>