<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import ColumnEditorDialog from "./ColumnEditorDialog.vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import ForeignKeyEditorDialog, {
  type ForeignKeyDraft,
} from "./ForeignKeyEditorDialog.vue";
import Icon from "./Icon.vue";
import IndexEditorDialog from "./IndexEditorDialog.vue";
import * as api from "../lib/api";
import {
  buildDropColumnStatement,
  buildDropIndexStatement,
} from "../lib/alter";
import { writeClipboard } from "../lib/clipboard";
import { quoteIdentifier } from "../lib/sql-literal";
import type {
  ColumnInfo,
  Driver,
  IndexInfo,
  TableInfo,
  TableStructure,
} from "../lib/types";

const props = defineProps<{
  connectionId: string;
  table: string;
  databaseLabel?: string;
  driver: Driver;
  kind?: string;
  focusIndex?: string | null;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  refresh: [];
}>();

const structure = ref<TableStructure | null>(null);
const loading = ref(false);
const error = ref("");
const message = ref("");
const actionError = ref("");
const columnDialogOpen = ref(false);
const editingColumn = ref<ColumnInfo | null>(null);
const indexDialogOpen = ref(false);
const dropTarget = ref<{
  kind: "column" | "index" | "foreignKey";
  name: string;
} | null>(null);
const dropOpen = ref(false);
const indexSection = ref<HTMLElement | null>(null);
const foreignKeyOpen = ref(false);
const availableTables = ref<TableInfo[]>([]);

const canAlter = computed(
  () => (props.kind ?? "table") === "table" && !props.readOnly,
);

async function load() {
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    structure.value = await api.getTableStructure(
      props.connectionId,
      props.table,
    );
    availableTables.value = (await api.listTables(props.connectionId)).filter(
      (table) => table.kind === "table",
    );
  } catch (reason) {
    structure.value = null;
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

watch(
  [() => props.connectionId, () => props.table],
  () => {
    void load();
  },
  { immediate: true },
);

async function scrollToFocusedIndex() {
  if (!props.focusIndex) return;
  await nextTick();
  const rows = indexSection.value?.querySelectorAll<HTMLElement>(
    "tr[data-index-name]",
  );
  if (!rows) return;
  const row = Array.from(rows).find(
    (item) => item.dataset.indexName === props.focusIndex,
  );
  row?.scrollIntoView({ block: "center", behavior: "smooth" });
}

watch([() => props.focusIndex, structure], () => {
  void scrollToFocusedIndex();
});

function columnsLabel(columns: string[]): string {
  return columns.length ? columns.join(", ") : "—";
}

async function copyDdl() {
  if (!structure.value?.ddl) return;
  try {
    await writeClipboard(structure.value.ddl);
    message.value = "DDL 已复制到剪贴板";
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

function openAddColumn() {
  editingColumn.value = null;
  columnDialogOpen.value = true;
  actionError.value = "";
}

function openEditColumn(column: ColumnInfo) {
  editingColumn.value = column;
  columnDialogOpen.value = true;
  actionError.value = "";
}

function openAddIndex() {
  indexDialogOpen.value = true;
  actionError.value = "";
}

function askDropColumn(column: ColumnInfo) {
  dropTarget.value = { kind: "column", name: column.name };
  dropOpen.value = true;
}

function askDropIndex(index: IndexInfo) {
  if (index.primary) return;
  dropTarget.value = { kind: "index", name: index.name };
  dropOpen.value = true;
}

function cancelDrop() {
  dropOpen.value = false;
  dropTarget.value = null;
}

async function reloadWithMessage(text: string) {
  await load();
  message.value = text;
  emit("refresh");
}

async function onStructureChanged(text: string) {
  columnDialogOpen.value = false;
  indexDialogOpen.value = false;
  await reloadWithMessage(text);
}

function askDropForeignKey(name: string) {
  if (props.driver === "sqlite") {
    actionError.value = "SQLite 需重建表，暂不支持在结构页删除外键";
    return;
  }
  dropTarget.value = { kind: "foreignKey", name };
  dropOpen.value = true;
}

function openAddForeignKey() {
  if (props.driver === "sqlite") {
    actionError.value = "SQLite 需重建表，暂不支持在结构页添加外键";
    return;
  }
  foreignKeyOpen.value = true;
}

async function applyForeignKey(draft: ForeignKeyDraft) {
  if (!canAlter.value) return;
  const tableRef = quoteIdentifier(props.table, props.driver);
  const localColumns = draft.columns
    .map((column) => quoteIdentifier(column, props.driver))
    .join(", ");
  const referencedColumns = draft.referencedColumns
    .map((column) => quoteIdentifier(column, props.driver))
    .join(", ");
  let sql = `ALTER TABLE ${tableRef} ADD CONSTRAINT ${quoteIdentifier(
    draft.name,
    props.driver,
  )} FOREIGN KEY (${localColumns}) REFERENCES ${quoteIdentifier(
    draft.referencedTable,
    props.driver,
  )} (${referencedColumns})`;
  if (draft.onDelete !== "NO ACTION") {
    sql += ` ON DELETE ${draft.onDelete}`;
  }
  if (draft.onUpdate !== "NO ACTION") {
    sql += ` ON UPDATE ${draft.onUpdate}`;
  }
  actionError.value = "";
  try {
    await api.executeQuery(props.connectionId, `${sql};`, 1);
    foreignKeyOpen.value = false;
    await reloadWithMessage("已添加外键");
  } catch (reason) {
    actionError.value = String(reason);
  }
}

function buildDropForeignKeyStatement(name: string): string {
  const tableRef = quoteIdentifier(props.table, props.driver);
  const keyRef = quoteIdentifier(name, props.driver);
  if (props.driver === "mysql") {
    return `ALTER TABLE ${tableRef} DROP FOREIGN KEY ${keyRef};`;
  }
  return `ALTER TABLE ${tableRef} DROP CONSTRAINT ${keyRef};`;
}

async function confirmDrop() {
  const target = dropTarget.value;
  dropOpen.value = false;
  dropTarget.value = null;
  if (!target) return;

  actionError.value = "";
  const sql =
    target.kind === "column"
      ? buildDropColumnStatement(props.driver, props.table, target.name)
      : target.kind === "index"
        ? buildDropIndexStatement(props.driver, props.table, target.name)
        : buildDropForeignKeyStatement(target.name);
  try {
    await api.executeQuery(props.connectionId, sql, 1);
    await reloadWithMessage(
      target.kind === "column"
        ? `已删除字段 ${target.name}`
        : target.kind === "index"
          ? `已删除索引 ${target.name}`
          : `已删除外键 ${target.name}`,
    );
  } catch (reason) {
    actionError.value = String(reason);
  }
}
</script>

<template>
  <section class="structure-view">
    <header class="data-toolbar">
      <div class="data-title">
        <strong>{{ table }}</strong>
        <span v-if="databaseLabel" class="data-context">
          {{ databaseLabel }}
        </span>
        <span class="hint" v-if="structure">
          {{ structure.columns.length }} 字段 ·
          {{ structure.indexes.length }} 索引 ·
          {{ structure.foreignKeys.length }} 外键
        </span>
      </div>
      <div class="data-actions">
        <button
          v-if="canAlter"
          class="button ghost compact"
          :disabled="loading || !structure"
          @click="openAddColumn"
        >
          <Icon name="plus" :size="14" />
          添加字段
        </button>
        <button
          v-if="canAlter"
          class="button ghost compact"
          :disabled="loading || !structure?.columns.length"
          @click="openAddIndex"
        >
          <Icon name="plus" :size="14" />
          添加索引
        </button>
        <button
          class="button ghost compact"
          :disabled="loading"
          @click="load"
        >
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button
          class="button ghost compact"
          :disabled="!structure?.ddl"
          @click="copyDdl"
        >
          <Icon name="copy" :size="14" />
          复制 DDL
        </button>
      </div>
    </header>

    <div class="structure-scroll">
      <div v-if="loading" class="panel-empty">加载中…</div>
      <div v-else-if="error" class="data-message error">{{ error }}</div>

      <template v-else-if="structure">
        <p v-if="message" class="data-message success">{{ message }}</p>
        <p v-if="actionError" class="data-message error">{{ actionError }}</p>

        <section class="structure-section">
          <h3>字段 <span>{{ structure.columns.length }}</span></h3>
          <table class="structure-table">
            <thead>
              <tr>
                <th>字段</th>
                <th>类型</th>
                <th>可空</th>
                <th>主键</th>
                <th>自增</th>
                <th>默认值</th>
                <th>注释</th>
                <th v-if="canAlter">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="column in structure.columns" :key="column.name">
                <td class="mono">{{ column.name }}</td>
                <td class="mono">{{ column.dataType }}</td>
                <td>{{ column.nullable ? "是" : "否" }}</td>
                <td>{{ column.primaryKey ? "是" : "否" }}</td>
                <td>{{ column.autoIncrement ? "是" : "否" }}</td>
                <td class="mono">{{ column.defaultValue ?? "—" }}</td>
                <td>{{ column.comment || "—" }}</td>
                <td v-if="canAlter">
                  <div class="structure-actions">
                    <button
                      class="icon-button"
                      title="编辑字段"
                      aria-label="编辑字段"
                      @click="openEditColumn(column)"
                    >
                      <Icon name="edit" :size="14" />
                    </button>
                    <button
                      class="icon-button"
                      title="删除字段"
                      aria-label="删除字段"
                      @click="askDropColumn(column)"
                    >
                      <Icon name="trash" :size="14" />
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </section>

        <section ref="indexSection" class="structure-section">
          <h3>索引 <span>{{ structure.indexes.length }}</span></h3>
          <table v-if="structure.indexes.length" class="structure-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>列</th>
                <th>唯一</th>
                <th>主键</th>
                <th v-if="canAlter">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="index in structure.indexes"
                :key="index.name"
                :data-index-name="index.name"
                :class="{ 'index-focused': index.name === focusIndex }"
              >
                <td class="mono">{{ index.name }}</td>
                <td class="mono">{{ columnsLabel(index.columns) }}</td>
                <td>{{ index.unique ? "是" : "否" }}</td>
                <td>{{ index.primary ? "是" : "否" }}</td>
                <td v-if="canAlter">
                  <div class="structure-actions">
                    <button
                      class="icon-button"
                      :disabled="index.primary"
                      :title="
                        index.primary ? '主键索引不能删除' : '删除索引'
                      "
                      aria-label="删除索引"
                      @click="askDropIndex(index)"
                    >
                      <Icon name="trash" :size="14" />
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-else class="panel-empty compact">暂无索引</div>
        </section>

        <section class="structure-section">
          <div class="structure-section-head">
            <h3>外键 <span>{{ structure.foreignKeys.length }}</span></h3>
            <button
              v-if="canAlter"
              class="button ghost compact"
              :disabled="driver === 'sqlite'"
              :title="
                driver === 'sqlite'
                  ? 'SQLite 需重建表，暂不支持'
                  : '添加外键'
              "
              @click="openAddForeignKey"
            >
              <Icon name="plus" :size="14" />
              添加外键
            </button>
          </div>
          <table v-if="structure.foreignKeys.length" class="structure-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>列</th>
                <th>引用表</th>
                <th>引用列</th>
                <th>ON DELETE</th>
                <th>ON UPDATE</th>
                <th v-if="canAlter">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="key in structure.foreignKeys" :key="key.name">
                <td class="mono">{{ key.name }}</td>
                <td class="mono">{{ columnsLabel(key.columns) }}</td>
                <td class="mono">{{ key.referencedTable }}</td>
                <td class="mono">
                  {{ columnsLabel(key.referencedColumns) }}
                </td>
                <td>{{ key.onDelete || "—" }}</td>
                <td>{{ key.onUpdate || "—" }}</td>
                <td v-if="canAlter">
                  <button
                    class="icon-button"
                    :disabled="driver === 'sqlite'"
                    :title="
                      driver === 'sqlite'
                        ? 'SQLite 需重建表，暂不支持'
                        : '删除外键'
                    "
                    aria-label="删除外键"
                    @click="askDropForeignKey(key.name)"
                  >
                    <Icon name="trash" :size="14" />
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-else class="panel-empty compact">暂无外键</div>
        </section>

        <section class="structure-section">
          <h3>DDL</h3>
          <pre class="ddl-preview">{{ structure.ddl || "无法读取 DDL" }}</pre>
        </section>
      </template>

      <div v-else class="panel-empty">选择表查看结构</div>
    </div>
  </section>

  <ColumnEditorDialog
    :open="columnDialogOpen"
    :connection-id="connectionId"
    :driver="driver"
    :table="table"
    :column="editingColumn"
    @close="columnDialogOpen = false"
    @done="onStructureChanged"
  />

  <IndexEditorDialog
    :open="indexDialogOpen"
    :connection-id="connectionId"
    :driver="driver"
    :table="table"
    :columns="structure?.columns ?? []"
    @close="indexDialogOpen = false"
    @done="onStructureChanged"
  />

  <ForeignKeyEditorDialog
    :open="foreignKeyOpen"
    :table="table"
    :driver="driver"
    :columns="structure?.columns ?? []"
    :tables="availableTables"
    @close="foreignKeyOpen = false"
    @apply="applyForeignKey"
  />

  <ConfirmDialog
    :open="dropOpen"
    :title="
      dropTarget?.kind === 'index'
        ? '删除索引'
        : dropTarget?.kind === 'foreignKey'
          ? '删除外键'
          : '删除字段'
    "
    :message="`确定删除${
      dropTarget?.kind === 'index'
        ? '索引'
        : dropTarget?.kind === 'foreignKey'
          ? '外键'
          : '字段'
    }「${dropTarget?.name ?? ''}」？`"
    :detail="
      dropTarget?.kind === 'index'
        ? '删除后依赖该索引的查询可能变慢。'
        : dropTarget?.kind === 'foreignKey'
          ? '删除后该列不再受引用完整性约束。'
          : '字段及其数据会被删除，此操作不可撤销。'
    "
    confirm-text="删除"
    @confirm="confirmDrop"
    @cancel="cancelDrop"
  />
</template>
