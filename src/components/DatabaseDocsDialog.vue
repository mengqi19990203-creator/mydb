<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  docsScopeKey,
  emptyScope,
  GROUP_COLORS,
  loadScope,
  noteFor,
  orphanTables,
  saveScope,
  tablesInGroup,
  type DocsGroup,
  type DocsScope,
} from "../lib/database-docs";
import { buildDbml } from "../lib/dbml";
import {
  buildHtml,
  buildMarkdown,
  type DocsMeta,
} from "../lib/database-docs-export";
import { writeClipboard } from "../lib/clipboard";
import type { DatabaseDocsSnapshot, DocsTable, Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  connectionName: string;
  database: string;
  schema: string;
}>();

const emit = defineEmits<{
  close: [];
}>();

const snapshot = ref<DatabaseDocsSnapshot | null>(null);
const loading = ref(false);
const error = ref("");
const search = ref("");
const activeGroup = ref<"all" | "ungrouped" | string>("all");
const selectedTable = ref("");
const selectedColumn = ref("");
const scope = ref<DocsScope>(emptyScope());
const tableNote = ref("");
const columnNote = ref("");
const noteStatus = ref("已保存");
const newGroupName = ref("");
const newGroupColor = ref(GROUP_COLORS[0]);
const deleteGroup = ref<DocsGroup | null>(null);
const exportFormat = ref<"dbml" | "markdown" | "html">("markdown");
const copying = ref(false);
const notice = ref("");
let saveTimer = 0;
let scopeKey = "";

const visibleTables = computed(() => {
  if (!snapshot.value) return [];
  let tables =
    activeGroup.value === "all"
      ? snapshot.value.tables
      : tablesInGroup(
          snapshot.value.tables,
          scope.value,
          activeGroup.value === "ungrouped" ? null : activeGroup.value,
        );
  const term = search.value.trim().toLowerCase();
  if (term) {
    tables = tables.filter((table) =>
      table.name.toLowerCase().includes(term),
    );
  }
  return tables;
});

const currentTable = computed<DocsTable | null>(
  () =>
    snapshot.value?.tables.find(
      (table) => table.name === selectedTable.value,
    ) ?? null,
);

const orphans = computed(() => orphanTables(scope.value, snapshot.value));

const relationCount = computed(
  () =>
    snapshot.value?.tables.reduce(
      (count, table) => count + table.structure.foreignKeys.length,
      0,
    ) ?? 0,
);

const meta = computed<DocsMeta>(() => ({
  connectionName: props.connectionName,
  database: props.database,
  schema: props.schema,
  driver: props.driver,
  generatedAt: new Date().toLocaleString("zh-CN", { hour12: false }),
}));

const dbml = computed(() => {
  if (!snapshot.value) return { text: "", warnings: [] as string[] };
  return buildDbml(
    snapshot.value.tables,
    scope.value.notes,
    props.connectionName || "mydb_docs",
    props.driver,
  );
});

const preview = computed(() => {
  if (!snapshot.value) {
    return { text: "", warnings: [] as string[] };
  }
  if (exportFormat.value === "dbml") return dbml.value;
  if (exportFormat.value === "markdown") {
    return {
      text: buildMarkdown(snapshot.value, scope.value.notes, meta.value),
      warnings: [],
    };
  }
  return {
    text: buildHtml(snapshot.value, scope.value.notes, meta.value),
    warnings: [],
  };
});

const byteSize = computed(
  () => new TextEncoder().encode(preview.value.text).length,
);

function stageLabel(kind: string): string {
  return kind === "view" ? "视图" : kind === "matview" ? "物化视图" : "表";
}

function loadScopeState() {
  scopeKey = docsScopeKey(props.connectionId, props.database, props.schema);
  scope.value = loadScope(scopeKey);
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    snapshot.value = await api.databaseDocsSnapshot(props.connectionId);
    loadScopeState();
    if (!visibleTables.value.length && snapshot.value.tables.length) {
      activeGroup.value = "all";
    }
    const still = snapshot.value.tables.find(
      (table) => table.name === selectedTable.value,
    );
    selectTable(still?.name ?? snapshot.value.tables[0]?.name ?? "");
  } catch (reason) {
    error.value = String(reason);
    snapshot.value = null;
  } finally {
    loading.value = false;
  }
}

function commitCurrentNote() {
  const name = selectedTable.value;
  if (!name) return;
  const note = noteFor(scope.value, name);
  note.tableNote = tableNote.value;
  if (selectedColumn.value) {
    note.columns[selectedColumn.value] = columnNote.value;
  }
  scope.value.notes[name] = note;
}

function flushNote() {
  window.clearTimeout(saveTimer);
  saveTimer = 0;
  commitCurrentNote();
  persistScope();
}

function persistScope() {
  try {
    saveScope(scopeKey, scope.value);
    noteStatus.value = "已保存";
  } catch {
    noteStatus.value = "保存失败";
  }
}

function scheduleNoteSave() {
  noteStatus.value = "保存中…";
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    commitCurrentNote();
    persistScope();
  }, 400);
}

function selectTable(name: string) {
  flushNote();
  selectedTable.value = name;
  selectedColumn.value = "";
  const note = noteFor(scope.value, name);
  tableNote.value = note.tableNote;
  columnNote.value = "";
}

function selectColumn(name: string) {
  flushNote();
  selectedColumn.value = name;
  columnNote.value = noteFor(scope.value, selectedTable.value).columns[name] ?? "";
}

function addGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  if (scope.value.groups.some((group) => group.name === name)) {
    error.value = "分组名称已存在";
    return;
  }
  scope.value.groups.push({
    id: crypto.randomUUID?.() ?? `group-${Date.now()}`,
    name,
    color: newGroupColor.value,
  });
  newGroupName.value = "";
  persistScope();
}

function renameGroup(group: DocsGroup, value: string) {
  const name = value.trim();
  if (!name) return;
  group.name = name;
  persistScope();
}

function confirmDeleteGroup() {
  const target = deleteGroup.value;
  deleteGroup.value = null;
  if (!target) return;
  scope.value.groups = scope.value.groups.filter(
    (group) => group.id !== target.id,
  );
  for (const [table, groupId] of Object.entries(scope.value.tableGroups)) {
    if (groupId === target.id) delete scope.value.tableGroups[table];
  }
  if (activeGroup.value === target.id) activeGroup.value = "all";
  persistScope();
}

function moveTable(groupId: string) {
  if (!selectedTable.value) return;
  if (!groupId) {
    delete scope.value.tableGroups[selectedTable.value];
  } else {
    scope.value.tableGroups[selectedTable.value] = groupId;
  }
  persistScope();
}

function deleteOrphan(name: string) {
  delete scope.value.notes[name];
  persistScope();
}

async function copyPreview() {
  if (!preview.value.text) return;
  copying.value = true;
  try {
    await writeClipboard(preview.value.text);
    notice.value = "已复制全文";
  } catch {
    notice.value = "复制失败";
  } finally {
    copying.value = false;
  }
}

async function saveExport() {
  if (!preview.value.text) return;
  if (byteSize.value > 20 * 1024 * 1024) {
    notice.value = "导出内容超过 20 MB，请减少表数量";
    return;
  }
  const extension =
    exportFormat.value === "dbml"
      ? "dbml"
      : exportFormat.value === "html"
        ? "html"
        : "md";
  const path = await saveDialog({
    defaultPath: `mydb-docs-${Date.now()}.${extension}`,
    filters: [
      {
        name:
          exportFormat.value === "dbml"
            ? "DBML"
            : exportFormat.value === "html"
              ? "HTML"
              : "Markdown",
        extensions: [extension],
      },
    ],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, preview.value.text);
    notice.value = `已导出到 ${path}`;
  } catch (reason) {
    notice.value = String(reason);
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      search.value = "";
      activeGroup.value = "all";
      selectedTable.value = "";
      selectedColumn.value = "";
      tableNote.value = "";
      columnNote.value = "";
      error.value = "";
      notice.value = "";
      exportFormat.value = "markdown";
      void load();
    } else {
      flushNote();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  flushNote();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal docs-modal" role="dialog" aria-modal="true" aria-label="数据库文档">
      <header class="modal-header">
        <div>
          <h2>数据库文档</h2>
          <p>{{ connectionName }} · {{ database }}{{ schema ? ` / ${schema}` : "" }} · 只读元数据</p>
        </div>
        <span class="spacer"></span>
        <button class="button ghost compact" :disabled="loading" @click="load">
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <p v-if="error" class="data-message error docs-message">{{ error }}</p>
      <p v-if="notice" class="data-message success docs-message">{{ notice }}</p>

      <div v-if="loading" class="panel-empty">正在读取数据库文档…</div>
      <div v-else-if="snapshot" class="docs-layout">
        <aside class="docs-sidebar">
          <div class="docs-search">
            <Icon name="search" :size="13" />
            <input v-model="search" placeholder="搜索表 / 视图" spellcheck="false" />
          </div>
          <div class="docs-groups">
            <button :class="{ active: activeGroup === 'all' }" @click="activeGroup = 'all'">
              全部 {{ snapshot.tables.length }}
            </button>
            <button :class="{ active: activeGroup === 'ungrouped' }" @click="activeGroup = 'ungrouped'">
              未分组
            </button>
            <button
              v-for="group in scope.groups"
              :key="group.id"
              :class="{ active: activeGroup === group.id }"
              @click="activeGroup = group.id"
            >
              <span class="docs-dot" :style="{ background: group.color }"></span>
              {{ group.name }}
            </button>
          </div>

          <div class="docs-table-list">
            <button
              v-for="table in visibleTables"
              :key="table.name"
              :class="{ active: table.name === selectedTable }"
              @click="selectTable(table.name)"
            >
              <Icon :name="table.kind === 'table' ? 'table' : 'view'" :size="12" />
              <span>{{ table.name }}</span>
              <small>{{ stageLabel(table.kind) }}</small>
            </button>
            <div v-if="!visibleTables.length" class="panel-empty compact">没有匹配的表。</div>
          </div>

          <div v-if="orphans.length" class="docs-orphans">
            <strong>孤立笔记 {{ orphans.length }}</strong>
            <div v-for="name in orphans" :key="name" class="docs-orphan">
              <span>{{ name }}（对象不存在）</span>
              <button class="button ghost compact" @click="deleteOrphan(name)">删除笔记</button>
            </div>
          </div>

          <div class="docs-group-admin">
            <strong>分组</strong>
            <div class="docs-group-form">
              <input v-model="newGroupName" placeholder="新分组名称" @keydown.enter.prevent="addGroup" />
              <input v-model="newGroupColor" type="color" aria-label="分组颜色" />
              <button class="button ghost compact" @click="addGroup">添加</button>
            </div>
            <div v-for="group in scope.groups" :key="group.id" class="docs-group-row">
              <span class="docs-dot" :style="{ background: group.color }"></span>
              <input
                :value="group.name"
                @change="renameGroup(group, ($event.target as HTMLInputElement).value)"
              />
              <button class="button ghost compact danger" @click="deleteGroup = group">删除</button>
            </div>
          </div>
        </aside>

        <section class="docs-detail">
          <div v-if="!currentTable" class="panel-empty">选择一张表查看文档。</div>
          <template v-else>
            <div class="docs-table-head">
              <h3>{{ currentTable.name }}</h3>
              <span class="docs-badge">{{ stageLabel(currentTable.kind) }}</span>
              <span class="spacer"></span>
              <label class="docs-move">
                分组
                <select
                  :value="scope.tableGroups[currentTable.name] ?? ''"
                  @change="moveTable(($event.target as HTMLSelectElement).value)"
                >
                  <option value="">未分组</option>
                  <option v-for="group in scope.groups" :key="group.id" :value="group.id">
                    {{ group.name }}
                  </option>
                </select>
              </label>
            </div>

            <div class="docs-notes">
              <div class="docs-notes-head">
                <strong>本地笔记</strong>
                <span :class="{ failed: noteStatus === '保存失败' }">{{ noteStatus }}</span>
              </div>
              <textarea
                v-model="tableNote"
                rows="3"
                placeholder="表级说明，仅保存在本机"
                @input="scheduleNoteSave"
              ></textarea>
              <div v-if="selectedColumn" class="docs-column-note">
                <span>字段 <code>{{ selectedColumn }}</code></span>
                <textarea
                  v-model="columnNote"
                  rows="2"
                  placeholder="字段级说明"
                  @input="scheduleNoteSave"
                ></textarea>
              </div>
            </div>

            <table class="structure-table docs-column-table">
              <thead>
                <tr>
                  <th>字段</th>
                  <th>类型</th>
                  <th>可空</th>
                  <th>默认值</th>
                  <th>主键</th>
                  <th>数据库注释</th>
                  <th>本地笔记</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="column in currentTable.structure.columns"
                  :key="column.name"
                  :class="{ selected: column.name === selectedColumn }"
                  @click="selectColumn(column.name)"
                >
                  <td class="mono">{{ column.name }}</td>
                  <td class="mono">{{ column.dataType }}</td>
                  <td>{{ column.nullable ? "是" : "否" }}</td>
                  <td class="docs-default">{{ column.defaultValue ?? "—" }}</td>
                  <td><span v-if="column.primaryKey" class="pk-badge">PK</span></td>
                  <td>{{ column.comment || "—" }}</td>
                  <td>{{ noteFor(scope, currentTable.name).columns[column.name] || "—" }}</td>
                </tr>
              </tbody>
            </table>

            <div class="docs-sections">
              <section>
                <h4>索引 {{ currentTable.structure.indexes.length }}</h4>
                <ul>
                  <li v-for="index in currentTable.structure.indexes" :key="index.name">
                    <code>{{ index.name }}</code>（{{ index.columns.join(", ") }}）
                    {{ index.primary ? "· 主键" : index.unique ? "· 唯一" : "" }}
                  </li>
                  <li v-if="!currentTable.structure.indexes.length">—</li>
                </ul>
              </section>
              <section>
                <h4>外键 {{ currentTable.structure.foreignKeys.length }}</h4>
                <ul>
                  <li v-for="foreignKey in currentTable.structure.foreignKeys" :key="foreignKey.name">
                    <code>{{ foreignKey.name }}</code>：{{ foreignKey.columns.join(", ") }} →
                    {{ foreignKey.referencedTable }}（{{ foreignKey.referencedColumns.join(", ") }}）
                    · DELETE {{ foreignKey.onDelete || "—" }} / UPDATE {{ foreignKey.onUpdate || "—" }}
                  </li>
                  <li v-if="!currentTable.structure.foreignKeys.length">—</li>
                </ul>
              </section>
            </div>

            <p class="hint">
              全库关系数：{{ relationCount }} · 表按当前库 / 模式读取；读取失败的表会出现在导出报告中。
            </p>
          </template>
        </section>

        <aside class="docs-export">
          <div class="docs-export-head">
            <h3>导出预览</h3>
            <select v-model="exportFormat">
              <option value="dbml">DBML</option>
              <option value="markdown">Markdown</option>
              <option value="html">HTML</option>
            </select>
          </div>
          <p class="hint">
            当前内容 {{ (byteSize / 1024).toFixed(1) }} KB · 上限 20 MB · DBML 仅包含表 / 字段 / 主键 / 索引 / 外键
          </p>
          <p v-for="warning in preview.warnings" :key="warning" class="docs-warning">{{ warning }}</p>
          <p v-if="snapshot.failures.length" class="docs-warning">
            {{ snapshot.failures.length }} 张表读取失败：{{ snapshot.failures.map((item) => item.name).join(", ") }}
          </p>
          <pre class="docs-preview">{{ preview.text || "—" }}</pre>
          <div class="docs-export-actions">
            <button class="button ghost compact" :disabled="copying" @click="copyPreview">
              复制全文
            </button>
            <button class="button primary compact" :disabled="!preview.text" @click="saveExport">
              导出…
            </button>
          </div>
        </aside>
      </div>

      <ConfirmDialog
        :open="!!deleteGroup"
        title="删除分组"
        :message="`确定删除分组「${deleteGroup?.name ?? ''}」？`"
        detail="表不会被删除，只会回到未分组；本地笔记不会丢失。"
        confirm-text="删除分组"
        danger
        @confirm="confirmDeleteGroup"
        @cancel="deleteGroup = null"
      />
    </div>
  </div>
</template>

<style scoped>
.docs-modal {
  width: min(1420px, calc(100vw - 28px));
  max-height: calc(100vh - 28px);
  display: flex;
  flex-direction: column;
}

.docs-message {
  margin: 8px 20px 0;
}

.docs-layout {
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr) 380px;
  min-height: 0;
  flex: 1;
  border-top: 1px solid var(--border-subtle);
}

.docs-sidebar {
  min-height: 0;
  overflow: auto;
  padding: 10px;
  border-right: 1px solid var(--border-subtle);
}

.docs-search {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 7px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-well);
}

.docs-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 11px;
}

.docs-groups {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 8px 0;
}

.docs-groups button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  background: transparent;
  color: var(--muted);
  font-size: 10px;
}

.docs-groups button.active {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--text);
}

.docs-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: 0 0 7px;
}

.docs-table-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.docs-table-list button {
  display: flex;
  align-items: center;
  gap: 5px;
  width: 100%;
  padding: 4px 5px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-size: 11px;
}

.docs-table-list button.active,
.docs-table-list button:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.docs-table-list button span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.docs-table-list button small {
  color: var(--faint);
  font-size: 8px;
}

.docs-orphans {
  margin-top: 10px;
  padding: 7px;
  border: 1px solid var(--warning-border, rgba(210, 153, 34, 0.35));
  border-radius: 6px;
  color: var(--text-secondary);
  font-size: 10px;
}

.docs-orphan {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
}

.docs-orphan span {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.docs-group-admin {
  margin-top: 12px;
  padding-top: 8px;
  border-top: 1px solid var(--border-subtle);
  font-size: 10px;
}

.docs-group-form {
  display: flex;
  gap: 4px;
  margin: 5px 0;
}

.docs-group-form input:first-child {
  flex: 1;
  min-width: 0;
  padding: 3px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.docs-group-form input[type="color"] {
  width: 26px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: transparent;
}

.docs-group-row {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 3px;
}

.docs-group-row input {
  flex: 1;
  min-width: 0;
  padding: 2px 4px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 10px;
}

.docs-group-row input:focus {
  border-color: var(--border);
  background: var(--surface-well);
  outline: none;
}

.docs-detail {
  min-height: 0;
  overflow: auto;
  padding: 12px 16px;
}

.docs-table-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.docs-table-head h3 {
  margin: 0;
  font-size: 15px;
  font-family: var(--font-mono);
}

.docs-badge {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--panel-3);
  color: var(--muted);
  font-size: 9px;
}

.docs-move {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--faint);
  font-size: 10px;
}

.docs-move select {
  padding: 3px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.docs-notes {
  margin: 8px 0 10px;
  padding: 8px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--panel-2);
}

.docs-notes-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
}

.docs-notes-head span {
  color: var(--faint);
  font-size: 9px;
}

.docs-notes-head span.failed {
  color: var(--danger);
}

.docs-notes textarea,
.docs-column-note textarea {
  width: 100%;
  margin-top: 5px;
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 11px;
  line-height: 1.5;
  resize: vertical;
}

.docs-column-note {
  margin-top: 6px;
  font-size: 10px;
  color: var(--muted);
}

.docs-column-table {
  width: 100%;
}

.docs-column-table tbody tr {
  cursor: pointer;
}

.docs-column-table tbody tr.selected {
  background: var(--accent-soft);
}

.docs-default {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 10px;
}

.docs-sections {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 12px;
}

.docs-sections h4 {
  margin: 0 0 4px;
  color: var(--muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.docs-sections ul {
  margin: 0;
  padding-left: 16px;
  color: var(--text-secondary);
  font-size: 10px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.docs-export {
  min-height: 0;
  overflow: auto;
  padding: 10px 12px;
  border-left: 1px solid var(--border-subtle);
  background: var(--panel-2);
}

.docs-export-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.docs-export-head h3 {
  margin: 0;
  font-size: 11px;
}

.docs-export-head select {
  padding: 3px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.docs-warning {
  margin: 6px 0 0;
  padding: 5px 7px;
  border-radius: 5px;
  background: rgba(210, 153, 34, 0.1);
  color: var(--warning, #d29922);
  font-size: 10px;
}

.docs-preview {
  max-height: 420px;
  overflow: auto;
  margin: 8px 0;
  padding: 8px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 9px;
  line-height: 1.5;
  white-space: pre-wrap;
}

.docs-export-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}
</style>