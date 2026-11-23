<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { ColumnInfo, ColumnValue, Driver } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  table: string;
  databaseLabel?: string;
  driver?: Driver;
}>();

const vFocus = {
  mounted: (element: HTMLInputElement) => element.focus(),
};

interface EditingCell {
  rowIndex: number;
  colIndex: number;
  value: string;
}

interface CellMenu {
  x: number;
  y: number;
  rowIndex: number;
  colIndex: number;
}

interface ResizeState {
  name: string;
  startX: number;
  startWidth: number;
}

const columns = ref<ColumnInfo[]>([]);
const rows = ref<unknown[][]>([]);
const total = ref(0);
const offset = ref(0);
const limit = ref(100);
const loading = ref(false);
const saving = ref(false);
const importing = ref(false);
const error = ref("");
const message = ref("");
let messageTimer = 0;
const editing = ref<EditingCell | null>(null);
const insertOpen = ref(false);
const insertValues = reactive<Record<string, string>>({});
const sortColumn = ref<string | null>(null);
const sortDirection = ref<"asc" | "desc">("asc");
const filterOpen = ref(false);
const filterDraft = ref("");
const appliedFilter = ref("");
const columnWidths = reactive<Record<string, number>>({});
const cellMenu = ref<CellMenu | null>(null);
const deleteRequest = ref<{ keys: ColumnValue[]; detail: string } | null>(null);
const importRequest = ref<{ path: string } | null>(null);
const importHasHeader = ref(true);
let resizeState: ResizeState | null = null;

const primaryKeys = computed(() =>
  columns.value.filter((column) => column.primaryKey),
);
const canEdit = computed(() => primaryKeys.value.length > 0);
const page = computed(() => Math.floor(offset.value / limit.value) + 1);
const pageCount = computed(() =>
  Math.max(1, Math.ceil(total.value / limit.value)),
);
const insertColumns = computed(() =>
  columns.value.filter((column) => !column.autoIncrement),
);

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

function isNull(value: unknown): boolean {
  return value === null || value === undefined;
}

function clearMessage() {
  window.clearTimeout(messageTimer);
  message.value = "";
}

function showMessage(text: string) {
  message.value = text;
  window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(clearMessage, 5000);
}

function columnIndex(name: string): number {
  return columns.value.findIndex((column) => column.name === name);
}

function defaultColumnWidth(column: ColumnInfo): number {
  const name = column.name.toLowerCase();
  if (name === "id" || name.endsWith("_id")) return 90;

  const type = column.dataType.toUpperCase();
  if (type.includes("TEXT") || type.includes("BLOB") || type.includes("JSON")) {
    return 200;
  }
  if (type.includes("DATE") || type.includes("TIME")) {
    return 170;
  }
  if (
    type.includes("DECIMAL") ||
    type.includes("NUMERIC") ||
    type.includes("REAL") ||
    type.includes("DOUBLE") ||
    type.includes("FLOAT")
  ) {
    return 110;
  }
  return 100;
}

function columnWidth(name: string): number {
  const stored = columnWidths[name];
  if (stored) return stored;
  const column = columns.value.find((item) => item.name === name);
  return column ? defaultColumnWidth(column) : 140;
}

function startResize(event: MouseEvent, name: string) {
  event.preventDefault();
  event.stopPropagation();
  resizeState = {
    name,
    startX: event.clientX,
    startWidth: columnWidth(name),
  };
  window.addEventListener("mousemove", moveResize);
  window.addEventListener("mouseup", stopResize);
}

function moveResize(event: MouseEvent) {
  if (!resizeState) return;
  columnWidths[resizeState.name] = Math.max(
    60,
    Math.min(
      600,
      resizeState.startWidth + event.clientX - resizeState.startX,
    ),
  );
}

function stopResize() {
  resizeState = null;
  window.removeEventListener("mousemove", moveResize);
  window.removeEventListener("mouseup", stopResize);
}

function quoteIdentifier(name: string): string {
  if (props.driver === "mysql") {
    return `\`${name.replaceAll("`", "``")}\``;
  }
  return `"${name.replaceAll('"', '""')}"`;
}

function conditionFor(column: ColumnInfo, value: unknown): string {
  const identifier = quoteIdentifier(column.name);
  if (isNull(value)) return `${identifier} IS NULL`;
  if (typeof value === "number") return `${identifier} = ${value}`;
  if (typeof value === "boolean") {
    return `${identifier} = ${value ? 1 : 0}`;
  }
  const text = typeof value === "object" ? JSON.stringify(value) : String(value);
  return `${identifier} = '${text.replaceAll("'", "''")}'`;
}

function openCellMenu(event: MouseEvent, rowIndex: number, colIndex: number) {
  event.preventDefault();
  cellMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 210)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 250)),
    rowIndex,
    colIndex,
  };
}

function closeCellMenu() {
  cellMenu.value = null;
}

function onGlobalKey(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeCellMenu();
    if (insertOpen.value) insertOpen.value = false;
    if (importRequest.value) importRequest.value = null;
  }
}

function menuValue(): unknown {
  if (!cellMenu.value) return null;
  return rows.value[cellMenu.value.rowIndex]?.[cellMenu.value.colIndex];
}

async function copyCellValue() {
  const value = menuValue();
  try {
    await writeClipboard(formatCell(value));
    showMessage("已复制单元格");
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
  closeCellMenu();
}

async function copyRowValues() {
  if (!cellMenu.value) return;
  const row = rows.value[cellMenu.value.rowIndex];
  if (!row) return;
  try {
    await writeClipboard(row.map(formatCell).join("\t"));
    showMessage("已复制整行");
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
  closeCellMenu();
}

function filterByValue() {
  if (!cellMenu.value) return;
  const column = columns.value[cellMenu.value.colIndex];
  if (!column) return;
  const condition = conditionFor(column, menuValue());
  filterDraft.value = condition;
  appliedFilter.value = condition;
  filterOpen.value = true;
  closeCellMenu();
  void load(0);
}

function sortByValue(direction: "asc" | "desc") {
  if (!cellMenu.value) return;
  const column = columns.value[cellMenu.value.colIndex];
  if (!column) return;
  sortColumn.value = column.name;
  sortDirection.value = direction;
  closeCellMenu();
  void load(0);
}

function deleteFromMenu() {
  if (!cellMenu.value) return;
  const rowIndex = cellMenu.value.rowIndex;
  closeCellMenu();
  void deleteRow(rowIndex);
}

onMounted(() => {
  window.addEventListener("click", closeCellMenu);
  window.addEventListener("keydown", onGlobalKey);
});

onBeforeUnmount(() => {
  window.clearTimeout(messageTimer);
  window.removeEventListener("click", closeCellMenu);
  window.removeEventListener("keydown", onGlobalKey);
  stopResize();
});

async function load(nextOffset = offset.value) {
  loading.value = true;
  error.value = "";
  try {
    const pageData = await api.readTableRows(
      props.connectionId,
      props.table,
      limit.value,
      Math.max(0, nextOffset),
      appliedFilter.value || undefined,
      sortColumn.value ?? undefined,
      sortColumn.value ? sortDirection.value : undefined,
    );
    columns.value = pageData.columns;
    rows.value = pageData.rows;
    total.value = pageData.total;
    offset.value = pageData.offset;
  } catch (reason) {
    rows.value = [];
    total.value = 0;
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

watch(
  [() => props.connectionId, () => props.table],
  () => {
    clearMessage();
    sortColumn.value = null;
    sortDirection.value = "asc";
    filterOpen.value = false;
    filterDraft.value = "";
    appliedFilter.value = "";
    for (const key of Object.keys(columnWidths)) {
      delete columnWidths[key];
    }
    cellMenu.value = null;
    columns.value = [];
    rows.value = [];
    total.value = 0;
    void load(0);
  },
  { immediate: true },
);

function applyFilter() {
  appliedFilter.value = filterDraft.value.trim();
  void load(0);
}

function clearFilter() {
  filterDraft.value = "";
  appliedFilter.value = "";
  filterOpen.value = false;
  void load(0);
}

function toggleSort(columnName: string) {
  if (sortColumn.value === columnName) {
    if (sortDirection.value === "asc") {
      sortDirection.value = "desc";
    } else {
      sortColumn.value = null;
      sortDirection.value = "asc";
    }
  } else {
    sortColumn.value = columnName;
    sortDirection.value = "asc";
  }
  void load(0);
}

function startEdit(rowIndex: number, colIndex: number) {
  if (!canEdit.value || saving.value) return;
  const value = rows.value[rowIndex]?.[colIndex];
  editing.value = {
    rowIndex,
    colIndex,
    value: isNull(value) ? "" : String(value),
  };
}

function parseInput(value: string, column: ColumnInfo): unknown {
  if (value.trim().toUpperCase() === "NULL") return null;
  if (value === "") return column.nullable ? null : "";

  const numeric =
    /^(INT|TINYINT|SMALLINT|MEDIUMINT|BIGINT|DECIMAL|NUMERIC|FLOAT|DOUBLE|REAL)/i.test(
      column.dataType,
    );
  if (numeric) {
    const parsed = Number(value);
    if (!Number.isNaN(parsed)) return parsed;
  }

  return value;
}

function rowKeys(row: unknown[]): ColumnValue[] {
  return primaryKeys.value.map((column) => ({
    column: column.name,
    value: row[columnIndex(column.name)],
  }));
}

async function commitEdit() {
  if (!editing.value || saving.value) return;
  const current = editing.value;

  const column = columns.value[current.colIndex];
  const row = rows.value[current.rowIndex];
  if (!column || !row) {
    editing.value = null;
    return;
  }

  const original = row[current.colIndex];
  if (current.value === (isNull(original) ? "" : String(original))) {
    editing.value = null;
    return;
  }

  saving.value = true;
  error.value = "";
  try {
    await api.updateTableRow(
      props.connectionId,
      props.table,
      rowKeys(row),
      [{ column: column.name, value: parseInput(current.value, column) }],
    );
    editing.value = null;
    await load(offset.value);
    showMessage(`已更新 ${props.table} 的 1 行`);
  } catch (reason) {
    error.value = String(reason);
    // Keep the editor and the user's input so a failed UPDATE is recoverable.
    editing.value = current;
  } finally {
    saving.value = false;
  }
}

async function deleteRow(rowIndex: number) {
  if (!canEdit.value || saving.value) return;
  const row = rows.value[rowIndex];
  if (!row) return;

  const keys = rowKeys(row);
  deleteRequest.value = {
    keys,
    detail: keys
      .map((key) => `${key.column} = ${formatCell(key.value)}`)
      .join("\n"),
  };
}

async function confirmDeleteRow() {
  const request = deleteRequest.value;
  deleteRequest.value = null;
  if (!request || saving.value) return;

  saving.value = true;
  error.value = "";
  try {
    await api.deleteTableRow(props.connectionId, props.table, request.keys);
    const nextOffset =
      rows.value.length === 1 && offset.value > 0
        ? Math.max(0, offset.value - limit.value)
        : offset.value;
    await load(nextOffset);
    showMessage("已删除 1 行");
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}

function openInsert() {
  for (const key of Object.keys(insertValues)) {
    delete insertValues[key];
  }
  for (const column of insertColumns.value) {
    insertValues[column.name] = "";
  }
  insertOpen.value = true;
}

async function submitInsert() {
  if (saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    const values = insertColumns.value.map((column) => ({
      column: column.name,
      value: parseInput(insertValues[column.name] ?? "", column),
    }));
    await api.insertTableRow(props.connectionId, props.table, values);
    insertOpen.value = false;
    await load(offset.value);
    showMessage("已插入 1 行");
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}

function csvEscape(value: unknown): string {
  if (isNull(value)) return "";
  const text = typeof value === "object" ? JSON.stringify(value) : String(value);
  if (/[",\n\r]/.test(text)) {
    return `"${text.replaceAll('"', '""')}"`;
  }
  return text;
}

async function exportCsv() {
  const lines = [columns.value.map((column) => csvEscape(column.name)).join(",")];
  for (const row of rows.value) {
    lines.push(row.map(csvEscape).join(","));
  }

  const path = await saveDialog({
    defaultPath: `${props.table}.csv`,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (typeof path !== "string") return;

  try {
    await api.writeTextFile(path, lines.join("\n"));
    showMessage(`已导出当前页到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

async function exportJson() {
  const path = await saveDialog({
    defaultPath: `${props.table}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;

  try {
    const payload = {
      table: props.table,
      columns: columns.value,
      rows: rows.value,
    };
    await api.writeTextFile(path, JSON.stringify(payload, null, 2));
    showMessage(`已导出当前页到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

async function importCsv() {
  if (importing.value) return;
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (typeof path !== "string") return;

  importHasHeader.value = true;
  importRequest.value = { path };
}

async function confirmImport() {
  const request = importRequest.value;
  if (!request || importing.value) return;
  importRequest.value = null;

  importing.value = true;
  error.value = "";
  try {
    const count = await api.importCsv(
      props.connectionId,
      props.table,
      request.path,
      importHasHeader.value,
    );
    await load(offset.value);
    showMessage(`已导入 ${count} 行`);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    importing.value = false;
  }
}

function changePage(next: number) {
  const target = Math.max(0, Math.min(pageCount.value - 1, next)) * limit.value;
  void load(target);
}

function changeSize(event: Event) {
  const size = Number((event.target as HTMLSelectElement).value);
  limit.value = size;
  void load(0);
}
</script>

<template>
  <section class="data-view">
    <header class="data-toolbar">
      <div class="data-title">
        <strong>{{ table }}</strong>
        <span v-if="databaseLabel" class="data-context">
          {{ databaseLabel }}
        </span>
        <span class="hint">{{ total }} 行</span>
        <span v-if="columns.length && !canEdit" class="hint warning">
          该表没有主键，行内编辑和删除已禁用
        </span>
        <span v-else-if="columns.length" class="hint">
          双击单元格编辑
        </span>
      </div>
      <div class="data-actions">
        <button
          class="button ghost compact"
          :disabled="loading"
          @click="load(offset)"
        >
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button
          class="button ghost compact"
          :disabled="!columns.length"
          @click="openInsert"
        >
          <Icon name="plus" :size="14" />
          新增行
        </button>
        <button
          class="button ghost compact"
          :disabled="importing"
          @click="importCsv"
        >
          <Icon name="upload" :size="14" />
          {{ importing ? "导入中…" : "导入 CSV" }}
        </button>
        <button
          class="button ghost compact"
          :disabled="!rows.length"
          @click="exportCsv"
        >
          <Icon name="download" :size="14" />
          导出当前页 CSV
        </button>
        <button
          class="button ghost compact"
          :disabled="!rows.length"
          @click="exportJson"
        >
          <Icon name="download" :size="14" />
          导出当前页 JSON
        </button>
      </div>
    </header>

    <div class="data-filterbar">
      <button
        class="button ghost compact filter-toggle"
        :class="{ active: filterOpen || appliedFilter }"
        @click="filterOpen = !filterOpen"
      >
        <Icon name="filter" :size="14" />
        WHERE
      </button>
      <input
        v-if="filterOpen"
        v-model="filterDraft"
        v-focus
        class="filter-input"
        placeholder="例如 status = 'paid'"
        @keydown.enter="applyFilter"
      />
      <button
        v-if="filterOpen"
        class="button primary compact"
        @click="applyFilter"
      >
        应用
      </button>
      <button
        v-if="appliedFilter"
        class="button ghost compact"
        @click="clearFilter"
      >
        清除
      </button>
      <span
        v-if="appliedFilter"
        class="filter-meta"
        :title="appliedFilter"
      >
        已筛选
      </span>
      <span v-else class="filter-meta">全部行</span>
      <div class="spacer"></div>
      <span v-if="sortColumn" class="filter-meta">
        ORDER BY {{ sortColumn }} {{ sortDirection.toUpperCase() }}
      </span>
    </div>

    <p v-if="message" class="data-message success" role="status">
      <span class="data-message-text">{{ message }}</span>
      <button
        class="data-message-close"
        title="关闭提示"
        aria-label="关闭提示"
        @click="clearMessage"
      >
        <Icon name="close" :size="12" />
      </button>
    </p>
    <p v-if="error" class="data-message error" role="alert">
      <span class="data-message-text">{{ error }}</span>
      <button
        class="data-message-close"
        title="关闭提示"
        aria-label="关闭提示"
        @click="error = ''"
      >
        <Icon name="close" :size="12" />
      </button>
    </p>

    <div class="data-grid-wrap">
      <div v-if="loading" class="panel-empty">加载中…</div>
      <div v-else-if="error" class="panel-empty">加载失败</div>
      <div v-else-if="!rows.length" class="panel-empty">
        暂无数据
      </div>
      <table v-else class="result-grid data-grid">
        <colgroup>
          <col style="width: 44px" />
          <col
            v-for="column in columns"
            :key="column.name"
            :style="{ width: columnWidth(column.name) + 'px' }"
          />
        </colgroup>
        <thead>
          <tr>
            <th class="row-index">#</th>
            <th
              v-for="column in columns"
              :key="column.name"
              class="sortable"
              tabindex="0"
              :aria-sort="
                sortColumn === column.name
                  ? sortDirection === 'asc'
                    ? 'ascending'
                    : 'descending'
                  : 'none'
              "
              @click="toggleSort(column.name)"
              @keydown.enter.prevent="toggleSort(column.name)"
              @keydown.space.prevent="toggleSort(column.name)"
            >
              <span class="column-head">
                <span>{{ column.name }}</span>
                <span v-if="column.primaryKey" class="pk-badge">PK</span>
                <Icon
                  v-if="sortColumn === column.name"
                  :name="sortDirection === 'asc' ? 'chevron-up' : 'chevron-down'"
                  :size="12"
                  class="sort-icon"
                />
              </span>
              <span class="column-type">{{ column.dataType }}</span>
              <span
                class="col-resizer"
                title="拖拽调整列宽"
                @mousedown="startResize($event, column.name)"
                @click.stop
              ></span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, rowIndex) in rows" :key="rowIndex">
            <td class="row-index">{{ offset + rowIndex + 1 }}</td>
            <td
              v-for="(cell, colIndex) in row"
              :key="colIndex"
              :class="{
                null: isNull(cell),
                num: typeof cell === 'number',
                editable: canEdit,
              }"
              :title="formatCell(cell)"
              @dblclick="startEdit(rowIndex, colIndex)"
              @contextmenu.prevent="openCellMenu($event, rowIndex, colIndex)"
            >
              <input
                v-if="
                  editing &&
                  editing.rowIndex === rowIndex &&
                  editing.colIndex === colIndex
                "
                v-model="editing.value"
                v-focus
                class="data-edit-input"
                @keydown.enter.prevent="commitEdit"
                @keydown.esc.prevent="editing = null"
                @blur="commitEdit"
              />
              <template v-else>{{ formatCell(cell) }}</template>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <footer class="data-pagination">
      <span>第 {{ page }} / {{ pageCount }} 页 · 共 {{ total }} 行</span>
      <div class="pagination-actions">
        <select :value="limit" @change="changeSize">
          <option :value="50">50 行/页</option>
          <option :value="100">100 行/页</option>
          <option :value="200">200 行/页</option>
          <option :value="500">500 行/页</option>
        </select>
        <button
          class="button ghost compact icon-only"
          title="上一页"
          aria-label="上一页"
          :disabled="loading || page <= 1"
          @click="changePage(page - 2)"
        >
          <Icon name="chevron-left" :size="14" />
        </button>
        <button
          class="button ghost compact icon-only"
          title="下一页"
          aria-label="下一页"
          :disabled="loading || page >= pageCount"
          @click="changePage(page)"
        >
          <Icon name="chevron-right" :size="14" />
        </button>
      </div>
    </footer>

    <div
      v-if="importRequest"
      class="modal-backdrop"
      @click.self="importRequest = null"
    >
      <div
        class="modal connection-modal"
        role="dialog"
        aria-modal="true"
        aria-label="导入 CSV"
      >
        <header class="modal-header">
          <div>
            <h2>导入 CSV</h2>
            <p>{{ databaseLabel ? `${databaseLabel} · ` : "" }}{{ table }}</p>
          </div>
          <button
            class="icon-button"
            title="关闭"
            aria-label="关闭导入"
            @click="importRequest = null"
          >
            <Icon name="close" :size="14" />
          </button>
        </header>
        <div class="connection-form">
          <div class="form-grid">
            <label class="span-2">
              目标表
              <input
                :value="`${databaseLabel ? databaseLabel + ' · ' : ''}${table}`"
                readonly
              />
            </label>
            <label class="span-2">
              CSV 文件
              <input :value="importRequest.path" readonly />
            </label>
            <label class="checkbox-row span-2">
              <input v-model="importHasHeader" type="checkbox" />
              首行为列名
            </label>
          </div>
          <p class="message warning">
            数据将追加写入现有表，不会清空已有行。
          </p>
        </div>
        <footer class="modal-footer">
          <div class="spacer"></div>
          <button class="button ghost" @click="importRequest = null">
            取消
          </button>
          <button
            class="button primary"
            :disabled="importing"
            @click="confirmImport"
          >
            {{ importing ? "导入中…" : "导入" }}
          </button>
        </footer>
      </div>
    </div>

    <div v-if="insertOpen" class="modal-backdrop" @click.self="insertOpen = false">
      <div
        class="modal connection-modal"
        role="dialog"
        aria-modal="true"
        aria-label="新增行"
      >
        <header class="modal-header">
          <div>
            <h2>新增行</h2>
            <p>{{ table }}</p>
          </div>
          <button
          class="icon-button"
          title="关闭"
          aria-label="关闭新增行"
          @click="insertOpen = false"
        >
          <Icon name="close" :size="14" />
        </button>
        </header>
        <div class="connection-form">
          <div class="form-grid">
            <label
              v-for="column in insertColumns"
              :key="column.name"
              :class="{ 'span-2': insertColumns.length === 1 }"
            >
              {{ column.name }}
              <span class="column-hint">
                {{ column.dataType }}{{ column.nullable ? " · 可空" : "" }}
              </span>
              <input
                v-model="insertValues[column.name]"
                :placeholder="column.nullable ? '留空表示 NULL' : '必填'"
              />
            </label>
          </div>
        </div>
        <footer class="modal-footer">
          <div class="spacer"></div>
          <button class="button ghost" @click="insertOpen = false">取消</button>
          <button class="button primary" :disabled="saving" @click="submitInsert">
            {{ saving ? "插入中…" : "插入" }}
          </button>
        </footer>
      </div>
    </div>

    <ConfirmDialog
      :open="!!deleteRequest"
      title="删除行"
      :message="`确定从「${table}」删除这一行？`"
      :detail="deleteRequest?.detail ?? null"
      confirm-text="删除行"
      @confirm="confirmDeleteRow"
      @cancel="deleteRequest = null"
    />

    <div
      v-if="cellMenu"
      class="context-menu"
      role="menu"
      :style="{ left: cellMenu.x + 'px', top: cellMenu.y + 'px' }"
      @click.stop
    >
      <button class="context-menu-item" @click="copyCellValue">
        <Icon name="copy" :size="14" />
        复制单元格
      </button>
      <button class="context-menu-item" @click="copyRowValues">
        <Icon name="copy" :size="14" />
        复制整行
      </button>
      <div class="context-separator"></div>
      <button class="context-menu-item" @click="filterByValue">
        <Icon name="filter" :size="14" />
        按此值筛选
      </button>
      <button class="context-menu-item" @click="sortByValue('asc')">
        <Icon name="chevron-up" :size="14" />
        升序排序
      </button>
      <button class="context-menu-item" @click="sortByValue('desc')">
        <Icon name="chevron-down" :size="14" />
        降序排序
      </button>
      <template v-if="canEdit">
        <div class="context-separator"></div>
        <button class="context-menu-item danger" @click="deleteFromMenu">
          <Icon name="trash" :size="14" />
          删除此行
        </button>
      </template>
    </div>
  </section>
</template>
