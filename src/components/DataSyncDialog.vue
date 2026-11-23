<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import { useSyncScope } from "../lib/sync-scope";
import type {
  ConnectionConfig,
  RowChange,
  TableDataDiff,
  TableInfo,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connections: ConnectionConfig[];
  currentConnectionId: string;
  initialTable?: string;
}>();

const emit = defineEmits<{
  close: [];
}>();

const sourceId = ref("");
const targetId = ref("");
const sourceTable = ref("");
const targetTable = ref("");
const sourceTables = ref<TableInfo[]>([]);
const targetTables = ref<TableInfo[]>([]);
const tablesLoading = ref(false);
const maxRows = ref(5000);
const comparing = ref(false);
const applying = ref(false);
const error = ref("");
const message = ref("");
const diff = ref<TableDataDiff | null>(null);
const applyConfirmOpen = ref(false);
let tableLoadToken = 0;

const available = computed(() =>
  props.connections.filter((connection) => connection.driver !== "redis"),
);

const {
  targetOptions,
  sourceDatabases,
  sourceSchemas,
  sourceDatabase,
  sourceSchema,
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
  driver,
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

const targetConnection = computed(
  () => available.value.find((item) => item.id === targetId.value) ?? null,
);

const sameTableTarget = computed(
  () =>
    targetScopeEqualsSource.value &&
    sourceTable.value === targetTable.value,
);

const canCompare = computed(
  () =>
    Boolean(sourceId.value) &&
    Boolean(targetId.value) &&
    Boolean(sourceTable.value) &&
    Boolean(targetTable.value) &&
    targetOptions.value.some((item) => item.id === targetId.value) &&
    !scopeLoading.value &&
    sourceScopeValid.value &&
    targetScopeValid.value &&
    !sameTableTarget.value &&
    Number.isFinite(maxRows.value) &&
    maxRows.value >= 100,
);

const applyDetail = computed(() => {
  const statements = diff.value?.statements ?? [];
  if (!statements.length) return null;
  const preview = statements.slice(0, 8).join("\n");
  return statements.length > 8 ? `${preview}\n…` : preview;
});

const canApply = computed(
  () =>
    Boolean(diff.value?.statements.length) &&
    !targetConnection.value?.readOnly &&
    !applying.value &&
    !comparing.value,
);

async function reloadTables() {
  const token = ++tableLoadToken;
  if (!sourceId.value || !targetId.value) {
    sourceTables.value = [];
    targetTables.value = [];
    tablesLoading.value = false;
    return;
  }
  tablesLoading.value = true;
  try {
    const sourcePromise = sourceContextDiffers.value
      ? api.listContextTables(
          sourceId.value,
          sourceDatabase.value || null,
          sourceSchema.value || null,
        )
      : api.listTables(sourceId.value);
    const targetPromise = targetContextDiffers.value
      ? api.listContextTables(
          targetId.value,
          targetDatabase.value || null,
          targetSchema.value || null,
        )
      : api.listTables(targetId.value);
    const [sourceList, targetList] = await Promise.all([
      sourcePromise,
      targetPromise,
    ]);
    if (token !== tableLoadToken) return;
    sourceTables.value = sourceList.filter((table) => table.kind === "table");
    targetTables.value = targetList.filter((table) => table.kind === "table");

    if (!sourceTables.value.some((table) => table.name === sourceTable.value)) {
      const preferred =
        props.initialTable &&
        sourceTables.value.some((table) => table.name === props.initialTable)
          ? props.initialTable
          : sourceTables.value[0]?.name ?? "";
      sourceTable.value = preferred;
    }
    if (!targetTables.value.some((table) => table.name === targetTable.value)) {
      const preferred = targetTables.value.some(
        (table) => table.name === sourceTable.value,
      )
        ? sourceTable.value
        : targetTables.value[0]?.name ?? "";
      targetTable.value = preferred;
    }
  } catch (reason) {
    if (token === tableLoadToken) {
      sourceTables.value = [];
      targetTables.value = [];
      error.value = String(reason);
    }
  } finally {
    if (token === tableLoadToken) tablesLoading.value = false;
  }
}

async function refreshAndLoad() {
  await refreshScope();
  if (!props.open) return;
  await reloadTables();
}

watch(
  () => props.open,
  (open) => {
    applyConfirmOpen.value = false;
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
    sourceTable.value = "";
    targetTable.value = "";
    diff.value = null;
    error.value = "";
    message.value = "";
    comparing.value = false;
    applying.value = false;
    targetId.value = targetOptions.value[0]?.id ?? "";
    void refreshAndLoad();
  },
  { immediate: true },
);

watch(
  [
    sourceId,
    sourceDatabase,
    sourceSchema,
    targetId,
    targetDatabase,
    targetSchema,
  ],
  () => {
    diff.value = null;
    void reloadTables();
  },
);

watch(sourceTable, () => {
  diff.value = null;
  if (targetTables.value.some((table) => table.name === sourceTable.value)) {
    targetTable.value = sourceTable.value;
  }
});

watch(targetTable, () => {
  diff.value = null;
});

watch(maxRows, () => {
  diff.value = null;
});

async function compare() {
  if (!canCompare.value || comparing.value) return;
  comparing.value = true;
  error.value = "";
  message.value = "";
  const scopedSource = driver.value === "mysql" || driver.value === "postgres";
  const scopedTarget = scopedSource;
  try {
    diff.value = await api.compareTableData(
      sourceId.value,
      targetId.value,
      sourceTable.value,
      targetTable.value,
      scopedSource ? sourceDatabase.value || null : null,
      scopedSource ? sourceSchema.value || null : null,
      scopedTarget ? targetDatabase.value || null : null,
      scopedTarget ? targetSchema.value || null : null,
      Math.round(maxRows.value),
    );
  } catch (reason) {
    diff.value = null;
    error.value = String(reason);
  } finally {
    comparing.value = false;
  }
}

function requestApply() {
  if (!canApply.value) return;
  applyConfirmOpen.value = true;
}

async function applyChanges() {
  if (!diff.value?.statements.length || applying.value) return;
  applyConfirmOpen.value = false;
  applying.value = true;
  error.value = "";
  try {
    const count = await api.applyTableDataSync(
      targetId.value,
      targetDatabase.value || null,
      targetSchema.value || null,
      diff.value.statements,
    );
    await compare();
    if (!error.value) {
      message.value = `已在目标连接执行 ${count} 条语句`;
    }
  } catch (reason) {
    error.value = String(reason);
  } finally {
    applying.value = false;
  }
}

function changeLabel(change: RowChange): string {
  switch (change.status) {
    case "added":
      return "新增";
    case "updated":
      return "更新";
    default:
      return "删除";
  }
}

function changeClass(change: RowChange): string {
  switch (change.status) {
    case "added":
      return "added";
    case "updated":
      return "changed";
    default:
      return "removed";
  }
}

async function copySql() {
  if (!diff.value?.sql) return;
  try {
    await writeClipboard(diff.value.sql);
    message.value = "同步 SQL 已复制到剪贴板";
  } catch (reason) {
    error.value = String(reason);
  }
}

async function saveSql() {
  if (!diff.value?.sql) return;
  const path = await saveDialog({
    defaultPath: "data-diff.sql",
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
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal schema-sync-modal data-sync-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="data-sync-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="data-sync-title">数据同步</h2>
          <p>
            按主键逐行比较两张表，生成 INSERT / UPDATE / DELETE；支持另一个同类型连接，或同一 MySQL / PostgreSQL 实例下的另一个库 / 模式。
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="schema-sync-body">
        <div class="form-grid schema-sync-pickers">
          <label>
            源连接（数据基准）
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
            目标连接（待同步）
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
          <label>
            源表
            <select
              v-model="sourceTable"
              :disabled="
                tablesLoading ||
                !sourceTables.length ||
                !sourceScopeValid
              "
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
          <label>
            目标表
            <select
              v-model="targetTable"
              :disabled="
                tablesLoading ||
                !targetTables.length ||
                !targetScopeValid
              "
            >
              <option
                v-for="table in targetTables"
                :key="table.name"
                :value="table.name"
              >
                {{ table.name }}
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
          <label class="data-sync-limit">
            每侧最多比较
            <input
              v-model.number="maxRows"
              type="number"
              min="100"
              max="50000"
              step="100"
            />
            行
          </label>
          <p v-if="scopeLoading || tablesLoading" class="draft-hint">
            正在读取目标范围与表列表…
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
            <span class="schema-sync-chip added">新增 {{ diff.insertCount }}</span>
            <span class="schema-sync-chip warn">更新 {{ diff.updateCount }}</span>
            <span class="schema-sync-chip removed">
              删除 {{ diff.deleteCount }}
            </span>
            <span v-if="diff.truncated" class="schema-sync-chip warn">
              已截断
            </span>
            <span class="schema-sync-chip">
              {{ diff.sourceLabel }} · {{ diff.sourceTable }} →
              {{ diff.targetLabel }} · {{ diff.targetTable }}
            </span>
          </div>

          <div class="data-sync-meta">
            <span>主键：{{ diff.keyColumns.join(", ") }}</span>
            <span>比较字段：{{ diff.comparedColumns.join(", ") }}</span>
            <span>
              行数：源 {{ diff.sourceTotal }} · 目标 {{ diff.targetTotal }}
            </span>
            <span v-if="diff.skippedColumns.length">
              跳过字段：{{ diff.skippedColumns.join(", ") }}
            </span>
            <span v-if="diff.sourceOnlyColumns.length">
              目标缺字段：{{ diff.sourceOnlyColumns.join(", ") }}
            </span>
            <span v-if="diff.targetOnlyColumns.length">
              目标多出字段：{{ diff.targetOnlyColumns.join(", ") }}
            </span>
          </div>

          <div v-for="warning in diff.warnings" :key="warning" class="schema-sync-warning">
            {{ warning }}
          </div>

          <div v-if="diff.changes.length" class="data-sync-changes">
            <div
              v-for="change in diff.changes"
              :key="`${change.status}:${change.key}`"
              class="data-sync-change"
            >
              <span
                class="schema-sync-status"
                :class="changeClass(change)"
              >
                {{ changeLabel(change) }}
              </span>
              <span class="data-sync-key">{{ change.key }}</span>
              <span v-if="change.changedColumns.length" class="hint">
                {{ change.changedColumns.join(", ") }}
              </span>
            </div>
            <p v-if="diff.changesTruncated" class="draft-hint">
              只展示前 {{ diff.changes.length }} 条差异，完整内容见下方 SQL。
            </p>
          </div>

          <section class="ddl-section">
            <h3>同步 SQL</h3>
            <pre class="ddl-preview schema-sync-sql">{{
              diff.sql || "-- 没有需要执行的变更"
            }}</pre>
          </section>
        </template>

        <p v-else class="draft-hint">
          选择源表和目标表后开始对比。两张表都需要主键；二进制和数组字段暂不参与同步。
        </p>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" :disabled="!diff?.sql" @click="copySql">
          复制 SQL
        </button>
        <button class="button ghost" :disabled="!diff?.sql" @click="saveSql">
          保存 SQL…
        </button>
        <div class="spacer"></div>
        <button
          v-if="targetConnection?.readOnly"
          class="button ghost"
          disabled
          title="目标连接为只读模式"
        >
          目标连接只读
        </button>
        <button
          v-else
          class="button"
          :disabled="!canApply"
          @click="requestApply"
        >
          {{ applying ? "同步中…" : "应用到目标…" }}
        </button>
        <button class="button ghost" @click="emit('close')">关闭</button>
      </footer>
    </div>
  </div>

  <ConfirmDialog
    :open="applyConfirmOpen"
    title="应用数据同步"
    :message="`将在目标连接「${diff?.targetLabel ?? ''}」执行 ${
      diff?.statements.length ?? 0
    } 条语句：新增 ${diff?.insertCount ?? 0} 行、更新 ${
      diff?.updateCount ?? 0
    } 行、删除 ${diff?.deleteCount ?? 0} 行。所有语句在单个事务中执行，失败会整体回滚。`"
    :detail="applyDetail"
    confirm-text="执行同步"
    @confirm="applyChanges"
    @cancel="applyConfirmOpen = false"
  />
</template>