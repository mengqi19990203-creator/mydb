<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import {
  addTable,
  aliasIssue,
  buildQuery,
  nullOperator,
  type BuilderState,
  type BuilderTable,
} from "../lib/query-builder";
import type { Driver, ErDiagram, ErTable } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
}>();

const emit = defineEmits<{
  close: [];
  "open-sql": [sql: string, title: string];
  "save-snippet": [sql: string];
}>();

const diagram = ref<ErDiagram | null>(null);
const loading = ref(false);
const error = ref("");
const search = ref("");
const model = ref<BuilderState>({
  tables: [],
  selected: [],
  joins: [],
  filters: [],
  sorts: [],
  limit: 100,
});

const operators = [
  "=",
  "<>",
  ">",
  ">=",
  "<",
  "<=",
  "LIKE",
  "NOT LIKE",
  "IS NULL",
  "IS NOT NULL",
] as const;

const available = computed(() => {
  const term = search.value.trim().toLowerCase();
  const added = new Set(model.value.tables.map((table) => table.name));
  return (diagram.value?.tables ?? []).filter(
    (table) =>
      !added.has(table.name) &&
      (!term || table.name.toLowerCase().includes(term)),
  );
});

const sql = computed(() => buildQuery(model.value, props.driver));

const selectedSet = computed(() => new Set(model.value.selected));

function columnsFor(table: BuilderTable): ErTable | undefined {
  return diagram.value?.tables.find((item) => item.name === table.name);
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    diagram.value = await api.getErDiagram(props.connectionId);
  } catch (reason) {
    error.value = String(reason);
    diagram.value = null;
  } finally {
    loading.value = false;
  }
}

function addTableName(table: ErTable) {
  if (model.value.tables.length >= 5) {
    error.value = "第一版最多添加 5 张表";
    return;
  }
  const suggestions = addTable(
    model.value,
    table.name,
    table.kind,
    diagram.value,
  );
  model.value.joins.push(...suggestions);
  error.value = "";
}

function removeTable(table: BuilderTable) {
  model.value.tables = model.value.tables.filter(
    (item) => item.name !== table.name,
  );
  model.value.selected = model.value.selected.filter(
    (key) => !key.startsWith(`${table.alias}.`),
  );
  model.value.joins = model.value.joins.filter(
    (join) =>
      join.leftAlias !== table.alias && join.rightAlias !== table.alias,
  );
  model.value.filters = model.value.filters.filter(
    (filter) => filter.alias !== table.alias,
  );
  model.value.sorts = model.value.sorts.filter(
    (sort) => sort.alias !== table.alias,
  );
}

function renameAlias(table: BuilderTable, value: string) {
  const next = value.trim();
  const issue = aliasIssue(next, model.value.tables, table.name);
  if (issue) {
    error.value = issue;
    return;
  }
  if (next === table.alias) return;
  const previous = table.alias;
  const updateKey = (key: string) =>
    key.startsWith(`${previous}.`)
      ? `${next}.${key.slice(previous.length + 1)}`
      : key;
  model.value.selected = model.value.selected.map(updateKey);
  for (const join of model.value.joins) {
    if (join.leftAlias === previous) join.leftAlias = next;
    if (join.rightAlias === previous) join.rightAlias = next;
  }
  for (const filter of model.value.filters) {
    if (filter.alias === previous) filter.alias = next;
  }
  for (const sort of model.value.sorts) {
    if (sort.alias === previous) sort.alias = next;
  }
  table.alias = next;
  error.value = "";
}

function toggleColumn(table: BuilderTable, column: string) {
  const key = `${table.alias}.${column}`;
  if (selectedSet.value.has(key)) {
    model.value.selected = model.value.selected.filter((item) => item !== key);
  } else {
    model.value.selected.push(key);
  }
}

function selectAll(table: BuilderTable) {
  const entry = columnsFor(table);
  if (!entry) return;
  const keys = entry.columns.map((column) => `${table.alias}.${column.name}`);
  model.value.selected = [
    ...model.value.selected.filter(
      (key) => !key.startsWith(`${table.alias}.`),
    ),
    ...keys,
  ];
}

function clearTable(table: BuilderTable) {
  model.value.selected = model.value.selected.filter(
    (key) => !key.startsWith(`${table.alias}.`),
  );
}

function addJoin() {
  if (model.value.tables.length < 2) return;
  const left = model.value.tables[0];
  const right = model.value.tables[1];
  const leftEntry = columnsFor(left);
  const rightEntry = columnsFor(right);
  model.value.joins.push({
    leftAlias: left.alias,
    leftColumn: leftEntry?.columns[0]?.name ?? "",
    rightAlias: right.alias,
    rightColumn: rightEntry?.columns[0]?.name ?? "",
    type: "INNER",
  });
}

function addFilter() {
  const first = model.value.tables[0];
  const column = first ? columnsFor(first)?.columns[0]?.name ?? "" : "";
  model.value.filters.push({
    alias: first?.alias ?? "",
    column,
    operator: "=",
    value: "",
  });
}

function addSort() {
  if (model.value.sorts.length >= 3) return;
  const first = model.value.tables[0];
  model.value.sorts.push({
    alias: first?.alias ?? "",
    column: columnsFor(first ?? { name: "", alias: "", kind: "table" })
      ?.columns[0]?.name ?? "",
    direction: "ASC",
  });
}

function tableColumns(alias: string) {
  const table = model.value.tables.find((item) => item.alias === alias);
  return table ? columnsFor(table)?.columns ?? [] : [];
}

function openQuery() {
  if (!sql.value.sql) return;
  emit("open-sql", sql.value.sql, "查询构建器");
  emit("close");
}

function saveSnippet() {
  if (!sql.value.sql) return;
  emit("save-snippet", sql.value.sql);
  emit("close");
}

async function copySql() {
  if (!sql.value.sql) return;
  try {
    await writeClipboard(sql.value.sql);
    error.value = "已复制 SQL";
  } catch {
    error.value = "复制失败";
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      search.value = "";
      error.value = "";
      model.value = {
        tables: [],
        selected: [],
        joins: [],
        filters: [],
        sorts: [],
        limit: 100,
      };
      void load();
    }
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal builder-modal" role="dialog" aria-modal="true" aria-label="查询构建器">
      <header class="modal-header">
        <div>
          <h2>查询构建器</h2>
          <p>{{ scopeLabel }} · 生成 SELECT 后打开到查询标签，不自动执行</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="builder-layout">
        <aside class="builder-tables">
          <div class="builder-search">
            <Icon name="search" :size="13" />
            <input v-model="search" placeholder="搜索表 / 视图" spellcheck="false" />
          </div>
          <p v-if="loading" class="panel-empty compact">正在读取对象…</p>
          <p v-else-if="error" class="data-message error">{{ error }}</p>
          <div v-else class="builder-table-list">
            <button
              v-for="table in available"
              :key="table.name"
              class="builder-table-item"
              @click="addTableName(table)"
            >
              <Icon :name="table.kind === 'view' ? 'view' : 'table'" :size="13" />
              <span>{{ table.name }}</span>
              <span class="builder-kind">{{ table.kind }}</span>
            </button>
            <div v-if="!available.length" class="panel-empty compact">
              没有可添加的表；同名表只能添加一次。
            </div>
          </div>
        </aside>

        <section class="builder-main">
          <div v-if="!model.tables.length" class="panel-empty">
            从左侧添加 1–5 张表开始构建查询。
          </div>

          <div
            v-for="table in model.tables"
            :key="table.name"
            class="builder-table-card"
          >
            <div class="builder-table-head">
              <strong>{{ table.name }}</strong>
              <span class="builder-kind">{{ table.kind }}</span>
              <label>
                别名
                <input
                  :value="table.alias"
                  class="builder-alias"
                  spellcheck="false"
                  @change="renameAlias(table, ($event.target as HTMLInputElement).value)"
                  @blur="renameAlias(table, ($event.target as HTMLInputElement).value)"
                />
              </label>
              <span class="spacer"></span>
              <button class="button ghost compact" @click="selectAll(table)">全选</button>
              <button class="button ghost compact" @click="clearTable(table)">清空</button>
              <button class="button ghost compact danger" @click="removeTable(table)">移除</button>
            </div>
            <div class="builder-columns">
              <label
                v-for="column in columnsFor(table)?.columns ?? []"
                :key="column.name"
                class="builder-column"
              >
                <input
                  type="checkbox"
                  :checked="selectedSet.has(`${table.alias}.${column.name}`)"
                  @change="toggleColumn(table, column.name)"
                />
                <span class="mono">{{ column.name }}</span>
                <small>{{ column.dataType }}</small>
                <span v-if="column.primaryKey" class="pk-badge">PK</span>
              </label>
            </div>
          </div>

          <template v-if="model.tables.length">
            <section class="builder-section">
              <div class="builder-section-head">
                <h3>JOIN 条件</h3>
                <button class="button ghost compact" :disabled="model.tables.length < 2" @click="addJoin">
                  添加条件
                </button>
              </div>
              <div v-if="!model.joins.length" class="hint">
                没有 JOIN 条件；未连接的表会生成 CROSS JOIN 并提示笛卡尔积风险。
              </div>
              <div v-for="(join, index) in model.joins" :key="index" class="builder-row">
                <select v-model="join.type"><option value="INNER">INNER</option><option value="LEFT">LEFT</option></select>
                <select v-model="join.leftAlias">
                  <option v-for="table in model.tables" :key="table.alias" :value="table.alias">{{ table.alias }}</option>
                </select>
                <select v-model="join.leftColumn">
                  <option v-for="column in tableColumns(join.leftAlias)" :key="column.name" :value="column.name">{{ column.name }}</option>
                </select>
                <span>=</span>
                <select v-model="join.rightAlias">
                  <option v-for="table in model.tables" :key="table.alias" :value="table.alias">{{ table.alias }}</option>
                </select>
                <select v-model="join.rightColumn">
                  <option v-for="column in tableColumns(join.rightAlias)" :key="column.name" :value="column.name">{{ column.name }}</option>
                </select>
                <button class="button ghost compact danger" @click="model.joins.splice(index, 1)">删除</button>
              </div>
            </section>

            <section class="builder-section">
              <div class="builder-section-head">
                <h3>筛选（AND）</h3>
                <button class="button ghost compact" @click="addFilter">添加条件</button>
              </div>
              <div v-for="(filter, index) in model.filters" :key="index" class="builder-row">
                <select v-model="filter.alias">
                  <option v-for="table in model.tables" :key="table.alias" :value="table.alias">{{ table.alias }}</option>
                </select>
                <select v-model="filter.column">
                  <option v-for="column in tableColumns(filter.alias)" :key="column.name" :value="column.name">{{ column.name }}</option>
                </select>
                <select v-model="filter.operator">
                  <option v-for="operator in operators" :key="operator" :value="operator">{{ operator }}</option>
                </select>
                <input
                  v-if="!nullOperator(filter.operator)"
                  v-model="filter.value"
                  placeholder="值；LIKE 请包含 %"
                />
                <span v-else class="hint">无值</span>
                <button class="button ghost compact danger" @click="model.filters.splice(index, 1)">删除</button>
              </div>
            </section>

            <section class="builder-section builder-sort-limit">
              <div class="builder-section-head">
                <h3>排序与限制</h3>
                <button class="button ghost compact" :disabled="model.sorts.length >= 3" @click="addSort">添加排序</button>
              </div>
              <div v-for="(sort, index) in model.sorts" :key="index" class="builder-row">
                <select v-model="sort.alias">
                  <option v-for="table in model.tables" :key="table.alias" :value="table.alias">{{ table.alias }}</option>
                </select>
                <select v-model="sort.column">
                  <option v-for="column in tableColumns(sort.alias)" :key="column.name" :value="column.name">{{ column.name }}</option>
                </select>
                <select v-model="sort.direction"><option value="ASC">ASC</option><option value="DESC">DESC</option></select>
                <button class="button ghost compact danger" @click="model.sorts.splice(index, 1)">删除</button>
              </div>
              <label class="builder-limit">
                LIMIT
                <input v-model.number="model.limit" type="number" min="1" max="10000" />
              </label>
            </section>
          </template>
        </section>

        <aside class="builder-preview">
          <div class="builder-preview-head">
            <h3>SQL 预览</h3>
            <button class="button ghost compact" :disabled="!sql.sql" @click="copySql">复制</button>
          </div>
          <p v-if="sql.emptyReason" class="panel-empty compact">{{ sql.emptyReason }}</p>
          <pre v-else class="builder-sql">{{ sql.sql }}</pre>
          <p v-for="warning in sql.warnings" :key="warning" class="builder-warning">
            {{ warning }}
          </p>
          <p v-if="error" class="data-message error">{{ error }}</p>
          <div class="builder-preview-actions">
            <button class="button ghost compact" :disabled="!sql.sql" @click="saveSnippet">
              另存为片段
            </button>
            <button class="button primary compact" :disabled="!sql.sql" @click="openQuery">
              在查询标签打开
            </button>
          </div>
        </aside>
      </div>
    </div>
  </div>
</template>

<style scoped>
.builder-modal {
  width: min(1320px, calc(100vw - 32px));
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
}

.builder-layout {
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr) 380px;
  min-height: 0;
  flex: 1;
  border-top: 1px solid var(--border-subtle);
}

.builder-tables {
  min-height: 0;
  overflow: auto;
  padding: 10px;
  border-right: 1px solid var(--border-subtle);
}

.builder-search {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 7px;
  margin-bottom: 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-well);
}

.builder-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 11px;
}

.builder-table-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.builder-table-item {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 5px 6px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-size: 11px;
}

.builder-table-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.builder-kind {
  margin-left: auto;
  color: var(--faint);
  font-size: 9px;
}

.builder-main {
  min-height: 0;
  overflow: auto;
  padding: 10px 14px 16px;
}

.builder-table-card {
  margin-bottom: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.builder-table-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 7px 9px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
}

.builder-table-head label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--faint);
  font-size: 10px;
}

.builder-alias {
  width: 110px;
  padding: 3px 5px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 10px;
}

.builder-columns {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 8px 9px;
}

.builder-column {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text-secondary);
  font-size: 10px;
}

.builder-column small {
  color: var(--faint);
  font-size: 8px;
}

.builder-section {
  margin-top: 12px;
}

.builder-section-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.builder-section-head h3 {
  margin: 0;
  font-size: 11px;
  font-weight: 600;
}

.builder-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  margin-bottom: 5px;
}

.builder-row select,
.builder-row input {
  padding: 4px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.builder-row input {
  min-width: 150px;
}

.builder-limit {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 5px;
  color: var(--faint);
  font-size: 10px;
}

.builder-limit input {
  width: 90px;
  padding: 4px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 11px;
}

.builder-preview {
  min-height: 0;
  overflow: auto;
  padding: 10px 12px;
  border-left: 1px solid var(--border-subtle);
  background: var(--panel-2);
}

.builder-preview-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.builder-preview-head h3 {
  margin: 0;
  font-size: 11px;
}

.builder-sql {
  min-height: 220px;
  max-height: 420px;
  overflow: auto;
  margin: 0;
  padding: 9px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  white-space: pre-wrap;
}

.builder-warning {
  margin: 6px 0 0;
  padding: 5px 7px;
  border-radius: 5px;
  background: rgba(210, 153, 34, 0.1);
  color: var(--warning, #d29922);
  font-size: 10px;
}

.builder-preview-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  margin-top: 10px;
}
</style>