<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type {
  DatabaseExportProgress,
  DatabaseExportSummary,
  Driver,
  ExportObject,
  TableInfo,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
  database?: string;
  schema?: string | null;
}>();

const emit = defineEmits<{
  close: [];
  exported: [summary: DatabaseExportSummary];
}>();

const GROUP_ORDER = [
  "table",
  "view",
  "matview",
  "sequence",
  "procedure",
  "function",
  "trigger",
  "event",
] as const;

const GROUP_LABELS: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
  sequence: "序列",
  procedure: "存储过程",
  function: "函数",
  trigger: "触发器",
  event: "事件",
};

const ALLOWED_KINDS = new Set([
  "table",
  "view",
  "matview",
  "sequence",
  "procedure",
  "function",
  "trigger",
  "event",
]);

const objects = ref<TableInfo[]>([]);
const selected = ref<Set<string>>(new Set());
const loading = ref(false);
const exporting = ref(false);
const error = ref("");
const search = ref("");
const includeStructure = ref(true);
const includeData = ref(true);
const includeDrops = ref(false);
const createDatabase = ref(false);
const removeDefiner = ref(true);
const rowsPerInsert = ref(100);
const useTransaction = ref(true);
const progress = ref<DatabaseExportProgress | null>(null);
const summary = ref<DatabaseExportSummary | null>(null);
const cancelRequested = ref(false);
const operationId = ref("");
let unlisten: UnlistenFn | undefined;

const isMysql = computed(() => props.driver === "mysql");

function keyOf(object: TableInfo): string {
  return `${object.kind}:${object.name}`;
}

const availableObjects = computed(() =>
  objects.value.filter((object) => ALLOWED_KINDS.has(object.kind)),
);

const filteredObjects = computed(() => {
  const query = search.value.trim().toLowerCase();
  if (!query) return availableObjects.value;
  return availableObjects.value.filter((object) =>
    object.name.toLowerCase().includes(query),
  );
});

const groups = computed(() =>
  GROUP_ORDER.flatMap((kind) => {
    const items = filteredObjects.value.filter((object) => object.kind === kind);
    return items.length
      ? [
          {
            kind,
            label: GROUP_LABELS[kind] ?? kind,
            items,
            selectedCount: items.filter((object) => selected.value.has(keyOf(object)))
              .length,
            allSelected: items.every((object) =>
              selected.value.has(keyOf(object)),
            ),
          },
        ]
      : [];
  }),
);

const selectedObjects = computed<ExportObject[]>(() =>
  availableObjects.value
    .filter((object) => selected.value.has(keyOf(object)))
    .map((object) => ({ kind: object.kind, name: object.name })),
);

const selectedCount = computed(() => selectedObjects.value.length);
const allVisibleSelected = computed(
  () =>
    filteredObjects.value.length > 0 &&
    filteredObjects.value.every((object) => selected.value.has(keyOf(object))),
);

const canExport = computed(
  () =>
    !loading.value &&
    !exporting.value &&
    selectedCount.value > 0 &&
    (includeStructure.value || includeData.value),
);

const progressPercent = computed(() => {
  const current = progress.value;
  if (!current || !current.objectTotal) return 0;
  return Math.min(
    100,
    Math.round((current.objectIndex / current.objectTotal) * 100),
  );
});

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(2)} MB`;
}

function reset() {
  objects.value = [];
  selected.value = new Set();
  loading.value = false;
  exporting.value = false;
  error.value = "";
  search.value = "";
  includeStructure.value = true;
  includeData.value = true;
  includeDrops.value = false;
  createDatabase.value = false;
  removeDefiner.value = true;
  rowsPerInsert.value = 100;
  useTransaction.value = true;
  progress.value = null;
  summary.value = null;
  cancelRequested.value = false;
  operationId.value = "";
}

async function loadObjects() {
  if (!props.connectionId) return;
  loading.value = true;
  error.value = "";
  try {
    let raw: TableInfo[] = [];
    if (isMysql.value) {
      const databases = await api.listDatabaseObjects(props.connectionId);
      const target = props.database || databases[0]?.database;
      raw =
        databases.find((item) => item.database === target)?.objects ?? [];
    } else {
      raw = await api.listTables(props.connectionId);
    }
    const seen = new Set<string>();
    const next: TableInfo[] = [];
    for (const object of raw) {
      if (!ALLOWED_KINDS.has(object.kind)) continue;
      const key = keyOf(object);
      if (seen.has(key)) continue;
      seen.add(key);
      next.push(object);
    }
    objects.value = next;
    selected.value = new Set(next.map(keyOf));
  } catch (reason) {
    objects.value = [];
    selected.value = new Set();
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function toggleObject(object: TableInfo) {
  if (exporting.value) return;
  const next = new Set(selected.value);
  const key = keyOf(object);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  selected.value = next;
}

function toggleGroup(kind: string) {
  if (exporting.value) return;
  const items = filteredObjects.value.filter(
    (object) => object.kind === kind,
  );
  const next = new Set(selected.value);
  const allSelected = items.every((object) => next.has(keyOf(object)));
  for (const object of items) {
    if (allSelected) next.delete(keyOf(object));
    else next.add(keyOf(object));
  }
  selected.value = next;
}

function toggleAllVisible() {
  if (exporting.value) return;
  const next = new Set(selected.value);
  if (allVisibleSelected.value) {
    for (const object of filteredObjects.value) next.delete(keyOf(object));
  } else {
    for (const object of filteredObjects.value) next.add(keyOf(object));
  }
  selected.value = next;
}

async function exportSql() {
  if (!canExport.value) return;
  error.value = "";
  summary.value = null;
  const defaultName = `${props.scopeLabel.replace(/[\\/:*?"<>|]/g, "-")}-${
    new Date().toISOString().slice(0, 10)
  }.sql`;
  const path = await saveDialog({
    defaultPath: defaultName,
    filters: [{ name: "SQL 文件", extensions: ["sql"] }],
  });
  if (!path) return;

  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `export-${Date.now()}`;
  operationId.value = id;
  exporting.value = true;
  cancelRequested.value = false;
  progress.value = null;
  try {
    const result = await api.exportDatabaseSql(
      props.connectionId,
      path,
      selectedObjects.value,
      {
        includeStructure: includeStructure.value,
        includeData: includeData.value,
        includeDrops: includeDrops.value,
        createDatabase: isMysql.value ? createDatabase.value : false,
        removeDefiner: isMysql.value ? removeDefiner.value : false,
        rowsPerInsert: Math.max(1, Math.min(500, Number(rowsPerInsert.value) || 100)),
        useTransaction: useTransaction.value,
      },
      id,
    );
    summary.value = result;
    emit("exported", result);
  } catch (reason) {
    error.value = cancelRequested.value ? "导出已取消" : String(reason);
  } finally {
    exporting.value = false;
    cancelRequested.value = false;
  }
}

async function cancelExport() {
  if (!exporting.value || !operationId.value) return;
  cancelRequested.value = true;
  try {
    await api.cancelExport(operationId.value);
  } catch (reason) {
    error.value = String(reason);
  }
}

function requestClose() {
  if (exporting.value) return;
  emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      void loadObjects();
    }
  },
  { immediate: true },
);

watch(includeStructure, (value) => {
  if (!value) includeDrops.value = false;
});

onMounted(async () => {
  unlisten = await listen<DatabaseExportProgress>(
    "database-export:progress",
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
      class="modal database-export-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="database-export-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="database-export-title">导出数据库 SQL</h2>
          <p>
            {{ scopeLabel }} · 结构 + 数据逻辑导出，生成可执行 `.sql` 文件
          </p>
        </div>
        <button
          class="icon-button"
          title="关闭"
          :disabled="exporting"
          @click="requestClose"
        >
          <Icon name="close" />
        </button>
      </header>

      <div class="database-export-body">
        <section class="export-pane">
          <div class="export-pane-head">
            <strong>对象</strong>
            <span class="hint">已选 {{ selectedCount }} / {{ availableObjects.length }}</span>
          </div>
          <div class="export-tools">
            <input
              v-model="search"
              class="export-search"
              placeholder="搜索对象…"
              :disabled="exporting"
            />
            <button
              class="button ghost compact"
              :disabled="exporting || !filteredObjects.length"
              @click="toggleAllVisible"
            >
              {{ allVisibleSelected ? "全不选" : "全选" }}
            </button>
          </div>

          <div v-if="loading" class="panel-empty compact">正在读取对象…</div>
          <div v-else-if="error && !objects.length" class="data-message error">
            {{ error }}
          </div>
          <div v-else-if="!availableObjects.length" class="panel-empty compact">
            当前范围没有可导出的对象
          </div>
          <div v-else class="export-object-list">
            <div v-for="group in groups" :key="group.kind" class="export-group">
              <label class="export-group-head">
                <input
                  type="checkbox"
                  :checked="group.allSelected"
                  :indeterminate="
                    group.selectedCount > 0 && !group.allSelected
                  "
                  :disabled="exporting"
                  @change="toggleGroup(group.kind)"
                />
                <strong>{{ group.label }}</strong>
                <span class="hint">{{ group.selectedCount }} / {{ group.items.length }}</span>
              </label>
              <label
                v-for="object in group.items"
                :key="`${object.kind}:${object.name}`"
                class="export-object-row"
              >
                <input
                  type="checkbox"
                  :checked="selected.has(`${object.kind}:${object.name}`)"
                  :disabled="exporting"
                  @change="toggleObject(object)"
                />
                <span class="export-object-name" :title="object.name">
                  {{ object.name }}
                </span>
              </label>
            </div>
          </div>
        </section>

        <section class="export-pane export-options">
          <strong>选项</strong>
          <label class="checkbox-row">
            <input v-model="includeStructure" type="checkbox" :disabled="exporting" />
            包含结构（表 / 视图 / 序列 / 例程 / 触发器）
          </label>
          <label class="checkbox-row">
            <input v-model="includeData" type="checkbox" :disabled="exporting" />
            包含表数据（多行 INSERT）
          </label>
          <label class="checkbox-row danger">
            <input
              v-model="includeDrops"
              type="checkbox"
              :disabled="exporting || !includeStructure"
            />
            包含 DROP ... IF EXISTS（危险）
          </label>

          <label v-if="isMysql" class="checkbox-row">
            <input v-model="createDatabase" type="checkbox" :disabled="exporting" />
            生成 CREATE DATABASE 与 USE
          </label>
          <label v-if="isMysql" class="checkbox-row">
            <input v-model="removeDefiner" type="checkbox" :disabled="exporting" />
            移除 DEFINER
          </label>

          <label class="export-number">
            每条 INSERT 行数
            <input
              v-model.number="rowsPerInsert"
              type="number"
              min="1"
              max="500"
              :disabled="exporting"
            />
          </label>
          <label class="checkbox-row">
            <input v-model="useTransaction" type="checkbox" :disabled="exporting" />
            数据段使用事务
          </label>

          <p class="draft-hint">
            逻辑备份，不替代原生 mysqldump / pg_dump；二进制按十六进制字面量导出。
          </p>
          <p v-if="!includeStructure && !includeData" class="message error">
            至少选择结构或数据。
          </p>

          <div v-if="exporting" class="export-progress">
            <div class="export-progress-head">
              <span>{{ progress?.message ?? "准备导出…" }}</span>
              <span>{{ progressPercent }}%</span>
            </div>
            <div class="export-progress-track">
              <div
                class="export-progress-fill"
                :style="{ width: `${progressPercent}%` }"
              ></div>
            </div>
            <div class="hint">
              {{ progress?.rows ?? 0 }} 行 · {{ formatBytes(progress?.bytes ?? 0) }}
            </div>
            <button
              class="button ghost compact"
              :disabled="cancelRequested"
              @click="cancelExport"
            >
              {{ cancelRequested ? "正在取消…" : "取消导出" }}
            </button>
          </div>

          <div v-if="summary" class="export-summary">
            <strong>导出完成</strong>
            <p class="export-path" :title="summary.path">{{ summary.path }}</p>
            <p class="hint">
              {{ summary.objectCount }} 个对象 · {{ summary.tableCount }} 张表 ·
              {{ summary.rows }} 行 · {{ formatBytes(summary.bytes) }}
            </p>
            <div v-if="summary.failures.length" class="export-problems">
              <strong>失败对象</strong>
              <p v-for="failure in summary.failures" :key="`${failure.kind}:${failure.name}`">
                {{ failure.kind }} {{ failure.name }}：{{ failure.message }}
              </p>
            </div>
            <div v-if="summary.warnings.length" class="export-problems">
              <strong>提示</strong>
              <p v-for="warning in summary.warnings" :key="warning">{{ warning }}</p>
            </div>
          </div>

          <p v-if="error" class="message error">{{ error }}</p>
        </section>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" :disabled="exporting" @click="requestClose">
          关闭
        </button>
        <div class="spacer"></div>
        <button
          class="button primary"
          :disabled="!canExport"
          @click="exportSql"
        >
          {{ exporting ? "导出中…" : "选择文件并导出" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.database-export-modal {
  width: min(920px, calc(100vw - 48px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
}

.database-export-body {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(280px, 0.9fr);
  gap: 18px;
  min-height: 0;
  overflow: hidden;
  padding: 0 20px 8px;
}

.export-pane {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: hidden;
}

.export-pane-head,
.export-group-head,
.export-progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.export-tools {
  display: flex;
  gap: 8px;
}

.export-search {
  min-width: 0;
  flex: 1;
}

.export-object-list {
  min-height: 180px;
  max-height: 360px;
  overflow: auto;
  padding: 4px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--surface-well);
}

.export-group {
  padding: 4px 6px;
}

.export-group-head {
  justify-content: flex-start;
  padding: 4px 2px;
  color: var(--text);
  cursor: pointer;
}

.export-object-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 4px 3px 22px;
  border-radius: 5px;
  color: var(--text-secondary);
  cursor: pointer;
}

.export-object-row:hover {
  background: var(--hover);
}

.export-object-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.export-options {
  gap: 9px;
  overflow-y: auto;
  padding-right: 2px;
}

.export-number {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.export-number input {
  width: 90px;
}

.checkbox-row.danger {
  color: var(--danger);
}

.export-progress,
.export-summary {
  padding: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.export-progress-track {
  height: 6px;
  margin: 8px 0 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--surface-well);
}

.export-progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 160ms ease;
}

.export-path {
  margin: 6px 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11px;
}

.export-problems {
  margin-top: 8px;
}

.export-problems p {
  margin: 3px 0;
  color: var(--danger);
  font-size: 11px;
}
</style>