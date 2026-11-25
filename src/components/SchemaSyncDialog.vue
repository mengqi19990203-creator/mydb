<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import { useSyncScope } from "../lib/sync-scope";
import type {
  ConnectionConfig,
  SchemaApplyProgress,
  SchemaApplyResult,
  SchemaDiff,
  SchemaTableDiff,
  TableInfo,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connections: ConnectionConfig[];
  currentConnectionId: string;
}>();

const emit = defineEmits<{
  close: [];
  applied: [];
}>();

const sourceId = ref("");
const targetId = ref("");
const sourceTables = ref<TableInfo[]>([]);
const targetTables = ref<TableInfo[]>([]);
const selectedTable = ref("");
const targetTable = ref("");
const syncAllTables = ref(true);
const includeIndexes = ref(true);
const includeForeignKeys = ref(true);
const includeDrops = ref(false);
const tablesLoading = ref(false);
const comparing = ref(false);
const error = ref("");
const message = ref("");
const diff = ref<SchemaDiff | null>(null);
const applyOpen = ref(false);
const applying = ref(false);
const applyError = ref("");
const applyResult = ref<SchemaApplyResult | null>(null);
const applyProgress = ref<SchemaApplyProgress | null>(null);
const confirmDrops = ref(false);
const confirmScope = ref("");
const cancelRequested = ref(false);
const operationId = ref("");
let applyUnlisten: UnlistenFn | undefined;
let tableLoadToken = 0;
let targetTableLoadToken = 0;

const targetConnection = computed(
  () => props.connections.find((item) => item.id === targetId.value) ?? null,
);
const targetReadOnly = computed(() => Boolean(targetConnection.value?.readOnly));
const targetScopeName = computed(
  () =>
    targetSchema.value ||
    targetDatabase.value ||
    targetConnection.value?.name ||
    "",
);
const diffStatements = computed<string[]>(
  () => diff.value?.tables.flatMap((table) => table.sql) ?? [],
);
const dangerousStatements = computed(() =>
  diffStatements.value.filter((statement) =>
    statement.trim().toUpperCase().startsWith("DROP "),
  ),
);
const statementKinds = computed(() => {
  const counts = { create: 0, alter: 0, drop: 0, other: 0 };
  for (const statement of diffStatements.value) {
    const upper = statement.trim().toUpperCase();
    if (upper.startsWith("CREATE ")) counts.create += 1;
    else if (upper.startsWith("ALTER ")) counts.alter += 1;
    else if (upper.startsWith("DROP ")) counts.drop += 1;
    else counts.other += 1;
  }
  return counts;
});
const applyReady = computed(() => {
  if (!diffStatements.value.length || targetReadOnly.value) return false;
  if (dangerousStatements.value.length) {
    return (
      confirmDrops.value &&
      confirmScope.value.trim() === targetScopeName.value.trim()
    );
  }
  return true;
});

const available = computed(() =>
  props.connections.filter((connection) => connection.driver !== "redis"),
);

const {
  driver,
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
  sourceContextDiffers,
  targetContextDiffers,
  targetScopeEqualsSource,
  targetScopeValid,
  loading: scopeLoading,
  optionLabel,
  refresh: refreshScope,
} = useSyncScope({
  connections: computed(() => props.connections),
  sourceId,
  targetId,
  onError: (value) => {
    error.value = value;
  },
});

const targetTableOptions = computed<TableInfo[]>(() => {
  if (syncAllTables.value || !selectedTable.value) {
    return targetTables.value;
  }
  if (targetTables.value.some((table) => table.name === selectedTable.value)) {
    return targetTables.value;
  }
  return [...targetTables.value, { name: selectedTable.value, kind: "table" }];
});

const sameTableTarget = computed(
  () =>
    !syncAllTables.value &&
    targetScopeEqualsSource.value &&
    selectedTable.value === targetTable.value,
);

const canCompare = computed(
  () =>
    Boolean(sourceId.value) &&
    Boolean(targetId.value) &&
    targetOptions.value.some((item) => item.id === targetId.value) &&
    !scopeLoading.value &&
    !tablesLoading.value &&
    sourceScopeValid.value &&
    targetScopeValid.value &&
    !sameTableTarget.value &&
    (syncAllTables.value ||
      (Boolean(selectedTable.value) && Boolean(targetTable.value))),
);

async function reloadSourceTables() {
  const token = ++tableLoadToken;
  if (!sourceId.value) {
    sourceTables.value = [];
    tablesLoading.value = false;
    return;
  }
  tablesLoading.value = true;
  try {
    const tables = sourceContextDiffers.value
      ? await api.listContextTables(
          sourceId.value,
          sourceDatabase.value || null,
          sourceSchema.value || null,
        )
      : await api.listTables(sourceId.value);
    if (token !== tableLoadToken) return;
    sourceTables.value = tables.filter((table) => table.kind === "table");
    if (!sourceTables.value.some((table) => table.name === selectedTable.value)) {
      selectedTable.value = sourceTables.value[0]?.name ?? "";
    }
  } catch (reason) {
    if (token === tableLoadToken) {
      sourceTables.value = [];
      error.value = String(reason);
    }
  } finally {
    if (token === tableLoadToken) tablesLoading.value = false;
  }
}

async function reloadTargetTables() {
  const token = ++targetTableLoadToken;
  if (!targetId.value || !targetScopeValid.value) {
    targetTables.value = [];
    if (!targetTable.value) {
      targetTable.value = "";
    }
    return;
  }
  try {
    const tables = targetContextDiffers.value
      ? await api.listContextTables(
          targetId.value,
          targetDatabase.value || null,
          targetSchema.value || null,
        )
      : await api.listTables(targetId.value);
    if (token !== targetTableLoadToken) return;
    targetTables.value = tables.filter((table) => table.kind === "table");
    if (!targetTables.value.some((table) => table.name === targetTable.value)) {
      targetTable.value = selectedTable.value || targetTables.value[0]?.name || "";
    }
  } catch (reason) {
    if (token === targetTableLoadToken) {
      targetTables.value = [];
      error.value = String(reason);
    }
  }
}

async function refreshScopeAndTables() {
  await refreshScope();
  await Promise.all([reloadSourceTables(), reloadTargetTables()]);
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const current =
      available.value.find(
        (connection) => connection.id === props.currentConnectionId,
      ) ?? available.value[0];
    sourceId.value = current?.id ?? "";
    targetId.value = "";
    sourceDatabase.value = "";
    sourceSchema.value = "";
    targetDatabase.value = "";
    targetSchema.value = "";
    selectedTable.value = "";
    targetTable.value = "";
    syncAllTables.value = true;
    includeIndexes.value = true;
    includeForeignKeys.value = true;
    includeDrops.value = false;
    diff.value = null;
    error.value = "";
    message.value = "";
    comparing.value = false;
    targetId.value = targetOptions.value[0]?.id ?? "";
    void refreshScopeAndTables();
  },
  { immediate: true },
);

watch([sourceId, sourceDatabase, sourceSchema], () => {
  diff.value = null;
  void reloadSourceTables();
});

watch([targetId, targetDatabase, targetSchema], () => {
  diff.value = null;
  void reloadTargetTables();
});

watch(selectedTable, () => {
  diff.value = null;
  if (!syncAllTables.value) {
    targetTable.value = selectedTable.value;
  }
});

watch(syncAllTables, (all) => {
  diff.value = null;
  if (!all) {
    targetTable.value = selectedTable.value;
  }
});

watch(targetTable, () => {
  diff.value = null;
});

watch(
  [includeIndexes, includeForeignKeys, includeDrops],
  () => {
    diff.value = null;
  },
);

async function fetchDiff(): Promise<SchemaDiff> {
  const scopedSource = driver.value === "mysql" || driver.value === "postgres";
  const scopedTarget = scopedSource;
  return api.compareSchemas(
    sourceId.value,
    targetId.value,
    scopedSource ? sourceDatabase.value || null : null,
    scopedSource ? sourceSchema.value || null : null,
    scopedTarget ? targetDatabase.value || null : null,
    scopedTarget ? targetSchema.value || null : null,
    syncAllTables.value ? null : selectedTable.value || null,
    syncAllTables.value ? null : targetTable.value || null,
    includeIndexes.value,
    includeForeignKeys.value,
    includeDrops.value,
  );
}

async function compare() {
  if (!canCompare.value || comparing.value) return;
  comparing.value = true;
  error.value = "";
  message.value = "";
  applyOpen.value = false;
  applyResult.value = null;
  try {
    diff.value = await fetchDiff();
  } catch (reason) {
    diff.value = null;
    error.value = String(reason);
  } finally {
    comparing.value = false;
  }
}

function statusLabel(status: SchemaTableDiff["status"]): string {
  switch (status) {
    case "onlySource":
      return "目标缺表";
    case "onlyTarget":
      return "目标多出";
    case "changed":
      return "结构不同";
    default:
      return "一致";
  }
}

function statusClass(status: SchemaTableDiff["status"]): string {
  switch (status) {
    case "onlySource":
      return "added";
    case "onlyTarget":
      return "removed";
    case "changed":
      return "changed";
    default:
      return "";
  }
}

function rowsLabel(value: number | null): string {
  return value === null ? "—" : `${value} 行`;
}

function targetTableLabel(name: string): string {
  return targetTables.value.some((table) => table.name === name)
    ? name
    : `${name}（目标缺表）`;
}

async function copySql() {
  if (!diff.value?.sql) return;
  try {
    await writeClipboard(diff.value.sql);
    message.value = "迁移 SQL 已复制到剪贴板";
  } catch (reason) {
    error.value = String(reason);
  }
}

async function saveSql() {
  if (!diff.value?.sql) return;
  const path = await saveDialog({
    defaultPath: "schema-diff.sql",
    filters: [{ name: "SQL", extensions: ["sql"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, diff.value.sql);
    message.value = `已保存到 ${path}`;
  } catch (reason) {
    error.value = String(reason);
  }
}

function openApply() {
  if (!diffStatements.value.length || targetReadOnly.value) return;
  applyOpen.value = true;
  applyResult.value = null;
  applyError.value = "";
  applyProgress.value = null;
  confirmDrops.value = false;
  confirmScope.value = "";
}

async function applySchema() {
  if (!applyReady.value || applying.value) return;
  if (dangerousStatements.value.length) {
    if (!confirmDrops.value) {
      applyError.value = "请先确认执行删除语句";
      return;
    }
    if (confirmScope.value.trim() !== targetScopeName.value.trim()) {
      applyError.value = `请输入目标名称 ${targetScopeName.value} 以确认`;
      return;
    }
  }
  applying.value = true;
  applyError.value = "";
  applyResult.value = null;
  applyProgress.value = null;
  cancelRequested.value = false;
  const expected = diffStatements.value.slice();
  try {
    const fresh = await fetchDiff();
    const latest = fresh.tables.flatMap((table) => table.sql);
    if (
      latest.length !== expected.length ||
      latest.some((statement, index) => statement !== expected[index])
    ) {
      applyError.value = "差异已变化，请重新对比后再应用；未执行任何语句";
      diff.value = fresh;
      return;
    }
    const id =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `schema-${Date.now()}`;
    operationId.value = id;
    const result = await api.applySchemaSync(
      targetId.value,
      targetDatabase.value || null,
      targetSchema.value || null,
      expected,
      id,
    );
    applyResult.value = result;
    if (!result.error && !result.cancelled) {
      emit("applied");
      try {
        diff.value = await fetchDiff();
      } catch {
        // Keep the apply result even if the follow-up comparison fails.
      }
    }
  } catch (reason) {
    applyError.value = String(reason);
  } finally {
    applying.value = false;
    operationId.value = "";
    cancelRequested.value = false;
  }
}

async function cancelApply() {
  if (!applying.value || !operationId.value) return;
  cancelRequested.value = true;
  try {
    await api.cancelSchemaSync(operationId.value);
  } catch (reason) {
    applyError.value = String(reason);
  }
}

onMounted(async () => {
  applyUnlisten = await listen<SchemaApplyProgress>(
    "schema-sync:progress",
    (event) => {
      if (event.payload.operationId === operationId.value) {
        applyProgress.value = event.payload;
      }
    },
  );
});

onBeforeUnmount(() => {
  applyUnlisten?.();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal schema-sync-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="schema-sync-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="schema-sync-title">结构同步</h2>
          <p>对比两个同类型连接，或同一 MySQL / PostgreSQL 连接下的不同库 / 模式，生成迁移 SQL；只生成不执行，删除类变更仅提示。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="schema-sync-body">
        <div class="form-grid schema-sync-pickers">
          <label>
            源连接（结构基准）
            <select v-model="sourceId">
              <option
                v-for="connection in available"
                :key="connection.id"
                :value="connection.id"
              >
                {{ connection.name }} · {{ connection.driver }}
              </option>
            </select>
          </label>
          <label v-if="driver === 'mysql'">
            源数据库
            <select v-model="sourceDatabase" :disabled="scopeLoading">
              <option
                v-for="database in sourceDatabases"
                :key="database"
                :value="database"
              >
                {{ database }}
              </option>
            </select>
          </label>
          <template v-if="driver === 'postgres'">
            <label>
              源数据库
              <select v-model="sourceDatabase" :disabled="scopeLoading">
                <option
                  v-for="database in sourceDatabases"
                  :key="database"
                  :value="database"
                >
                  {{ database }}
                </option>
              </select>
            </label>
            <label>
              源模式
              <select v-model="sourceSchema" :disabled="scopeLoading">
                <option
                  v-for="schema in sourceSchemas"
                  :key="schema"
                  :value="schema"
                >
                  {{ schema }}
                </option>
              </select>
            </label>
          </template>
          <label>
            目标连接（待迁移）
            <select v-model="targetId" :disabled="!targetOptions.length">
              <option
                v-for="connection in targetOptions"
                :key="connection.id"
                :value="connection.id"
              >
                {{ optionLabel(connection) }}
              </option>
            </select>
          </label>
          <label v-if="driver === 'mysql'">
            目标数据库
            <select v-model="targetDatabase" :disabled="scopeLoading">
              <option
                v-for="database in targetDatabases"
                :key="database"
                :value="database"
              >
                {{ database }}
              </option>
            </select>
          </label>
          <template v-if="driver === 'postgres'">
            <label>
              目标数据库
              <select v-model="targetDatabase" :disabled="scopeLoading">
                <option
                  v-for="database in targetDatabases"
                  :key="database"
                  :value="database"
                >
                  {{ database }}
                </option>
              </select>
            </label>
            <label>
              目标模式
              <select v-model="targetSchema" :disabled="scopeLoading">
                <option
                  v-for="schema in targetSchemas"
                  :key="schema"
                  :value="schema"
                >
                  {{ schema }}
                </option>
              </select>
            </label>
          </template>
          <label v-if="!syncAllTables">
            源表
            <select
              v-model="selectedTable"
              :disabled="tablesLoading || !sourceTables.length"
            >
              <option
                v-for="table in sourceTables"
                :key="table.name"
                :value="table.name"
              >
                {{ table.name }}
              </option>
            </select>
          </label>
          <label v-if="!syncAllTables">
            目标表
            <select
              v-model="targetTable"
              :disabled="
                tablesLoading ||
                !targetTableOptions.length ||
                !targetScopeValid
              "
            >
              <option
                v-for="table in targetTableOptions"
                :key="table.name"
                :value="table.name"
              >
                {{ targetTableLabel(table.name) }}
              </option>
            </select>
          </label>
        </div>

        <div class="schema-sync-actions">
          <button
            class="button primary"
            :disabled="!canCompare || comparing"
            @click="compare"
          >
            {{ comparing ? "对比中…" : "开始对比" }}
          </button>
          <label class="checkbox-row">
            <input v-model="syncAllTables" type="checkbox" />
            全部表
          </label>
          <label class="checkbox-row">
            <input v-model="includeIndexes" type="checkbox" />
            包含索引
          </label>
          <label class="checkbox-row">
            <input v-model="includeForeignKeys" type="checkbox" />
            包含外键
          </label>
          <label class="checkbox-row danger">
            <input v-model="includeDrops" type="checkbox" />
            包含删除语句（危险）
          </label>
          <p v-if="scopeLoading || tablesLoading" class="draft-hint">
            正在读取源 / 目标范围与表列表…
          </p>
          <p v-else-if="!targetOptions.length" class="draft-hint">
            SQLite 需要另一条连接作为目标；请先新建目标连接。
          </p>
          <p v-else-if="!targetScopeValid" class="draft-hint">
            请选择目标数据库 / 模式。
          </p>
          <p v-else-if="sameTableTarget" class="draft-hint">
            源表和目标表相同，请选择不同的目标表。
          </p>
          <p
            v-else-if="syncAllTables && targetScopeEqualsSource"
            class="draft-hint"
          >
            源和目标范围相同，全部表对比结果会全部一致。
          </p>
          <p v-else-if="driver === 'mysql'" class="draft-hint">
            源：{{ sourceDatabase || "—" }}
          </p>
          <p v-else-if="driver === 'postgres'" class="draft-hint">
            源：{{ sourceDatabase || "—" }} · {{ sourceSchema || "—" }}
          </p>
        </div>

        <p v-if="error" class="schema-sync-error">{{ error }}</p>
        <p v-if="message" class="schema-sync-message">{{ message }}</p>

        <template v-if="diff">
          <div class="schema-sync-summary">
            <span class="schema-sync-chip ok">一致 {{ diff.sameCount }}</span>
            <span class="schema-sync-chip warn">
              结构不同 {{ diff.changedCount }}
            </span>
            <span class="schema-sync-chip added">
              目标缺表 {{ diff.onlySourceCount }}
            </span>
            <span class="schema-sync-chip removed">
              目标多出 {{ diff.onlyTargetCount }}
            </span>
            <span class="schema-sync-chip">
              {{ diff.sourceLabel }} → {{ diff.targetLabel }}
            </span>
          </div>

          <div class="schema-sync-tables">
            <div
              v-for="table in diff.tables"
              :key="table.name"
              class="schema-sync-table"
            >
              <div class="schema-sync-table-head">
                <Icon name="table" :size="13" />
                <span class="schema-sync-table-name">{{ table.name }}</span>
                <span
                  class="schema-sync-status"
                  :class="statusClass(table.status)"
                >
                  {{ statusLabel(table.status) }}
                </span>
                <span class="hint">
                  源 {{ rowsLabel(table.sourceRows) }} · 目标
                  {{ rowsLabel(table.targetRows) }}
                </span>
              </div>

              <div v-if="table.addedColumns.length" class="schema-sync-detail">
                新增字段：{{ table.addedColumns.join(", ") }}
              </div>
              <div v-if="table.missingColumns.length" class="schema-sync-detail">
                目标多出字段：{{ table.missingColumns.join(", ") }}
              </div>
              <div
                v-for="change in table.changedColumns"
                :key="change.name"
                class="schema-sync-detail"
              >
                字段 {{ change.name }}：{{ change.details.join("；") }}
              </div>
              <div v-if="table.addedIndexes.length" class="schema-sync-detail">
                新增索引：{{ table.addedIndexes.join(", ") }}
              </div>
              <div v-if="table.extraIndexes.length" class="schema-sync-detail">
                目标多出索引：{{ table.extraIndexes.join(", ") }}
              </div>
              <div v-if="table.changedIndexes.length" class="schema-sync-detail">
                索引定义不同：{{ table.changedIndexes.join(", ") }}
              </div>
              <div
                v-if="table.addedForeignKeys.length"
                class="schema-sync-detail"
              >
                新增外键：{{ table.addedForeignKeys.join(", ") }}
              </div>
              <div
                v-if="table.extraForeignKeys.length"
                class="schema-sync-detail"
              >
                目标多出外键：{{ table.extraForeignKeys.join(", ") }}
              </div>
              <div
                v-if="table.changedForeignKeys.length"
                class="schema-sync-detail"
              >
                外键定义不同：{{ table.changedForeignKeys.join(", ") }}
              </div>
              <div
                v-for="warning in table.warnings"
                :key="warning"
                class="schema-sync-warning"
              >
                {{ warning }}
              </div>
            </div>
          </div>

          <section class="ddl-section">
            <h3>迁移 SQL</h3>
            <pre class="ddl-preview schema-sync-sql">{{
              diff.sql || "-- 没有需要执行的变更"
            }}</pre>
          </section>

          <section v-if="applyOpen" class="schema-apply-panel">
            <div class="schema-apply-head">
              <strong>应用到目标</strong>
              <span class="hint">
                {{ diff.targetLabel }} · {{ diffStatements.length }} 条语句
              </span>
            </div>
            <div class="schema-apply-kinds">
              <span>CREATE {{ statementKinds.create }}</span>
              <span>ALTER {{ statementKinds.alter }}</span>
              <span :class="{ danger: statementKinds.drop }">
                DROP {{ statementKinds.drop }}
              </span>
              <span v-if="statementKinds.other">其他 {{ statementKinds.other }}</span>
            </div>
            <p v-if="targetReadOnly" class="message error">
              目标连接为只读模式，无法应用结构变更。
            </p>
            <template v-if="dangerousStatements.length">
              <p class="schema-apply-danger">
                包含 {{ dangerousStatements.length }} 条删除语句（DROP），执行后不可自动恢复。
              </p>
              <label class="checkbox-row danger">
                <input v-model="confirmDrops" type="checkbox" />
                我确认执行删除语句
              </label>
              <label>
                输入目标名称确认
                <input
                  v-model="confirmScope"
                  :placeholder="targetScopeName"
                  autocomplete="off"
                />
              </label>
            </template>
            <p class="draft-hint">
              应用前会重新对比一次；若差异已变化会中止，不执行旧语句。PostgreSQL /
              SQLite 使用单个事务，MySQL DDL 会隐式提交、失败后无法自动回滚。第一版不生成目标结构快照。
            </p>

            <div v-if="applying" class="schema-apply-progress">
              <span>
                第 {{ applyProgress?.index ?? 0 }} / {{ applyProgress?.total ?? diffStatements.length }} 条
              </span>
              <span class="hint">{{ applyProgress?.preview ?? "准备执行…" }}</span>
              <button
                class="button ghost compact"
                :disabled="cancelRequested"
                @click="cancelApply"
              >
                {{ cancelRequested ? "正在取消…" : "取消应用" }}
              </button>
            </div>

            <div v-if="applyResult" class="schema-apply-result">
              <p :class="applyResult.error ? 'message error' : 'message success'">
                {{
                  applyResult.cancelled
                    ? "已取消应用"
                    : applyResult.error
                      ? "应用失败"
                      : "应用成功"
                }}
                · 已执行 {{ applyResult.applied }} / {{ applyResult.total }} 条 ·
                {{ applyResult.elapsedMs }} ms
                <template v-if="applyResult.rolledBack"> · 已全部回滚</template>
              </p>
              <p v-if="applyResult.error" class="schema-apply-error">
                {{ applyResult.error }}
              </p>
            </div>

            <div class="schema-apply-actions">
              <button
                class="button ghost compact"
                :disabled="applying"
                @click="applyOpen = false"
              >
                收起
              </button>
              <button
                class="button primary compact"
                :disabled="!applyReady || applying"
                @click="applySchema"
              >
                {{ applying ? "应用中…" : "确认应用" }}
              </button>
            </div>
          </section>
        </template>

        <p v-else class="draft-hint">
          选择源和目标后开始对比。新增表 / 字段会生成可执行 SQL，字段差异会以注释形式给出建议；同类型连接可指向同一 MySQL / PostgreSQL 实例的另一个库 / 模式。
        </p>
      </div>

      <footer class="modal-footer">
        <button
          class="button ghost"
          :disabled="!diff?.sql"
          @click="copySql"
        >
          复制 SQL
        </button>
        <button
          class="button ghost"
          :disabled="!diff?.sql"
          @click="saveSql"
        >
          保存 SQL…
        </button>
        <button
          class="button primary"
          :disabled="
            !diffStatements.length ||
            targetReadOnly ||
            applying ||
            comparing
          "
          :title="
            targetReadOnly
              ? '目标连接为只读模式'
              : '对比后审阅并应用到目标'
          "
          @click="openApply"
        >
          应用到目标…
        </button>
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">关闭</button>
      </footer>
    </div>
  </div>
</template>