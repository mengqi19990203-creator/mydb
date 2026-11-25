<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  allSnippets,
  driverMatches,
  exportSnippetsJson,
  loadUserSnippets,
  newSnippet,
  parseSnippetsJson,
  saveUserSnippets,
  searchSnippets,
  SNIPPET_IMPORT_LIMIT,
  SNIPPET_LIMIT,
  triggerIssue,
  uniqueTrigger,
  validateSnippet,
  type SnippetDriver,
  type SqlSnippet,
} from "../lib/sql-snippets";
import type { Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  driver: Driver;
  initialBody?: string;
}>();

const emit = defineEmits<{
  close: [];
  insert: [text: string];
}>();

const userSnippets = ref<SqlSnippet[]>([]);
const search = ref("");
const filter = ref<"all" | "mine" | "builtin" | "driver">("all");
const sort = ref<"name" | "trigger" | "tags">("name");
const selectedId = ref("");
const editor = ref<SqlSnippet | null>(null);
const editorTagsText = ref("");
const editorError = ref("");
const deleteTarget = ref<SqlSnippet | null>(null);
const insertTarget = ref<SqlSnippet | null>(null);
const message = ref("");
const error = ref("");
let messageTimer = 0;

const currentDriver = computed<SnippetDriver>(() =>
  props.driver === "mysql" || props.driver === "postgres" || props.driver === "sqlite"
    ? props.driver
    : "any",
);

const everything = computed(() => allSnippets(userSnippets.value));

const filtered = computed(() => {
  let list = everything.value;
  if (filter.value === "mine") {
    list = list.filter((snippet) => !snippet.builtin);
  } else if (filter.value === "builtin") {
    list = list.filter((snippet) => snippet.builtin);
  } else if (filter.value === "driver") {
    list = list.filter((snippet) =>
      driverMatches(snippet, currentDriver.value),
    );
  }
  list = searchSnippets(list, search.value);
  return [...list].sort((left, right) => {
    if (sort.value === "trigger") {
      return left.trigger.localeCompare(right.trigger);
    }
    if (sort.value === "tags") {
      return (left.tags[0] ?? "").localeCompare(right.tags[0] ?? "");
    }
    return left.name.localeCompare(right.name, "zh-CN");
  });
});

const selected = computed(
  () => filtered.value.find((snippet) => snippet.id === selectedId.value) ?? null,
);

const driverLabels: Record<SnippetDriver, string> = {
  any: "通用",
  mysql: "MySQL",
  postgres: "PostgreSQL",
  sqlite: "SQLite",
};

const mismatch = computed(
  () =>
    selected.value &&
    !driverMatches(selected.value, currentDriver.value),
);

function driverLabel(driver: SnippetDriver): string {
  return driverLabels[driver];
}

function flash(text: string) {
  message.value = text;
  window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(() => {
    message.value = "";
  }, 1800);
}

function reload() {
  userSnippets.value = loadUserSnippets();
}

function persist() {
  try {
    saveUserSnippets(userSnippets.value);
    flash("已保存片段");
    reload();
  } catch (reason) {
    error.value = `保存片段失败: ${reason}`;
  }
}

function startCreate() {
  if (userSnippets.value.length >= SNIPPET_LIMIT) {
    error.value = `最多保存 ${SNIPPET_LIMIT} 条片段`;
    return;
  }
  editor.value = newSnippet("", currentDriver.value);
  editorTagsText.value = "";
  editorError.value = "";
}

function startEdit(snippet: SqlSnippet) {
  if (snippet.builtin) return;
  editor.value = structuredClone(snippet);
  editorTagsText.value = snippet.tags.join(", ");
  editorError.value = "";
}

function duplicate(snippet: SqlSnippet) {
  if (userSnippets.value.length >= SNIPPET_LIMIT) {
    error.value = `最多保存 ${SNIPPET_LIMIT} 条片段`;
    return;
  }
  const copy = newSnippet(snippet.body, snippet.driver);
  copy.name = `${snippet.name} 副本`;
  copy.trigger = uniqueTrigger(`${snippet.trigger}-copy`, everything.value);
  copy.tags = [...snippet.tags];
  editor.value = copy;
  editorTagsText.value = copy.tags.join(", ");
  editorError.value = "";
}

function saveEditor() {
  const draft = editor.value;
  if (!draft) return;
  const issue = validateSnippet(draft, everything.value);
  if (issue) {
    editorError.value = issue;
    return;
  }
  draft.name = draft.name.trim();
  draft.trigger = draft.trigger.trim();
  draft.tags = editorTagsText.value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean)
    .slice(0, 20);
  draft.updatedAt = new Date().toISOString();
  const index = userSnippets.value.findIndex((item) => item.id === draft.id);
  if (index >= 0) {
    userSnippets.value[index] = { ...draft };
  } else {
    userSnippets.value.push({ ...draft });
  }
  const savedId = draft.id;
  editor.value = null;
  editorError.value = "";
  persist();
  selectedId.value = savedId;
}

function removeSnippet() {
  const target = deleteTarget.value;
  deleteTarget.value = null;
  if (!target || target.builtin) return;
  userSnippets.value = userSnippets.value.filter(
    (snippet) => snippet.id !== target.id,
  );
  persist();
  if (selectedId.value === target.id) selectedId.value = "";
}

function requestInsert(snippet: SqlSnippet) {
  if (!driverMatches(snippet, currentDriver.value)) {
    insertTarget.value = snippet;
    return;
  }
  doInsert(snippet);
}

function doInsert(snippet: SqlSnippet) {
  emit("insert", snippet.body);
  emit("close");
}

function confirmInsertAnyway() {
  const target = insertTarget.value;
  insertTarget.value = null;
  if (target) doInsert(target);
}

async function exportJson() {
  if (!userSnippets.value.length) {
    error.value = "没有可导出的自定义片段";
    return;
  }
  const path = await saveDialog({
    defaultPath: "mydb-snippets.json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, exportSnippetsJson(userSnippets.value));
    flash("片段已导出");
  } catch (reason) {
    error.value = String(reason);
  }
}

async function importJson() {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;
  error.value = "";
  try {
    const raw = await api.readTextFile(path);
    if (new TextEncoder().encode(raw).length > SNIPPET_IMPORT_LIMIT) {
      throw new Error("JSON 文件不能超过 2 MB");
    }
    const result = parseSnippetsJson(raw, everything.value);
    const room = Math.max(0, SNIPPET_LIMIT - userSnippets.value.length);
    const accepted = result.snippets.slice(0, room);
    userSnippets.value = [...userSnippets.value, ...accepted];
    persist();
    if (result.skipped || accepted.length < result.snippets.length) {
      flash(
        `已导入 ${accepted.length} 条，跳过 ${result.skipped + result.snippets.length - accepted.length} 条`,
      );
    } else {
      flash(`已导入 ${accepted.length} 条片段`);
    }
  } catch (reason) {
    error.value = String(reason);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape" && !editor.value && !deleteTarget.value && !insertTarget.value) {
    event.preventDefault();
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    search.value = "";
    filter.value = "all";
    sort.value = "name";
    editor.value = null;
    deleteTarget.value = null;
    insertTarget.value = null;
    error.value = "";
    message.value = "";
    reload();
    if (props.initialBody?.trim()) {
      editor.value = newSnippet(props.initialBody, currentDriver.value);
      editorTagsText.value = "";
      editorError.value = "";
    }
  },
  { immediate: true },
);

watch(
  filtered,
  (list) => {
    if (selectedId.value && list.some((snippet) => snippet.id === selectedId.value)) {
      return;
    }
    selectedId.value = list[0]?.id ?? "";
  },
  { immediate: true },
);

watch(
  () => props.open,
  (open) => {
    if (open) {
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  window.clearTimeout(messageTimer);
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal snippet-modal"
      role="dialog"
      aria-modal="true"
      aria-label="SQL 片段库"
    >
      <header class="modal-header">
        <div>
          <h2>SQL 片段库</h2>
          <p>保存在本机，不要保存密码、令牌或完整连接串；不会执行 SQL 或写入历史。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="snippet-toolbar">
        <div class="snippet-search">
          <Icon name="search" :size="14" />
          <input
            v-model="search"
            placeholder="搜索名称、trigger、标签或正文"
            spellcheck="false"
          />
        </div>
        <select v-model="filter" aria-label="片段过滤">
          <option value="all">全部</option>
          <option value="mine">我的</option>
          <option value="builtin">内置</option>
          <option value="driver">适用于 {{ driverLabel(currentDriver) }}</option>
        </select>
        <select v-model="sort" aria-label="片段排序">
          <option value="name">按名称</option>
          <option value="trigger">按 trigger</option>
          <option value="tags">按标签</option>
        </select>
        <span class="spacer"></span>
        <button class="button ghost compact" @click="importJson">导入 JSON</button>
        <button
          class="button ghost compact"
          :disabled="!userSnippets.length"
          @click="exportJson"
        >
          导出 JSON
        </button>
        <button class="button primary compact" @click="startCreate">
          <Icon name="plus" :size="14" />
          新建
        </button>
      </div>

      <p v-if="error" class="message error snippet-message">{{ error }}</p>
      <p v-else-if="message" class="message success snippet-message">{{ message }}</p>

      <div v-if="!filtered.length" class="panel-empty snippet-empty">
        <p>没有匹配的片段。</p>
        <p class="hint">选中 SQL → 保存为片段，或在右上角新建。</p>
      </div>

      <div v-else class="snippet-layout">
        <div class="snippet-list" role="listbox" aria-label="SQL 片段">
          <button
            v-for="snippet in filtered"
            :key="snippet.id"
            class="snippet-row"
            :class="{ selected: snippet.id === selectedId }"
            role="option"
            :aria-selected="snippet.id === selectedId"
            @click="selectedId = snippet.id"
            @dblclick="requestInsert(snippet)"
          >
            <span class="snippet-row-name">{{ snippet.name }}</span>
            <span class="snippet-trigger">{{ snippet.trigger }}</span>
            <span v-if="snippet.builtin" class="snippet-builtin">内置</span>
            <span
              v-if="!driverMatches(snippet, currentDriver)"
              class="snippet-driver-mismatch"
            >
              不适用
            </span>
            <span class="snippet-row-meta">
              {{ driverLabel(snippet.driver) }}
            </span>
          </button>
        </div>

        <aside v-if="selected" class="snippet-preview">
          <div class="snippet-preview-head">
            <div>
              <h3>{{ selected.name }}</h3>
              <div class="snippet-preview-meta">
                <span class="snippet-trigger">{{ selected.trigger }}</span>
                <span>{{ driverLabel(selected.driver) }}</span>
                <span>{{ new Date(selected.updatedAt).toLocaleString("zh-CN", { hour12: false }) }}</span>
              </div>
            </div>
            <button class="button primary compact" @click="requestInsert(selected)">
              插入
            </button>
          </div>

          <div v-if="selected.tags.length" class="snippet-tags">
            <span v-for="tag in selected.tags" :key="tag">{{ tag }}</span>
          </div>

          <p v-if="mismatch" class="snippet-mismatch-note">
            该片段标记为 {{ driverLabel(selected.driver) }}，当前连接是 {{ driverLabel(currentDriver) }}；插入前会再次确认。
          </p>

          <pre class="snippet-body">{{ selected.body }}</pre>

          <div class="snippet-actions">
            <button
              class="button ghost compact"
              :disabled="selected.builtin"
              @click="startEdit(selected)"
            >
              编辑
            </button>
            <button class="button ghost compact" @click="duplicate(selected)">
              复制为我的片段
            </button>
            <button
              class="button ghost compact danger"
              :disabled="selected.builtin"
              @click="deleteTarget = selected"
            >
              删除
            </button>
          </div>
        </aside>
      </div>

      <p class="snippet-foot hint">
        {{ userSnippets.length }} / {{ SNIPPET_LIMIT }} 条自定义片段 · 存储在 localStorage
      </p>

      <div v-if="editor" class="snippet-editor-backdrop">
        <form class="snippet-editor" @submit.prevent="saveEditor">
          <header class="modal-header">
            <h2>{{ userSnippets.some((item) => item.id === editor?.id) ? "编辑片段" : "新建片段" }}</h2>
            <button
              type="button"
              class="icon-button"
              title="关闭"
              @click="editor = null"
            >
              <Icon name="close" />
            </button>
          </header>
          <div class="snippet-form">
            <label class="snippet-field">
              <span>名称</span>
              <input v-model="editor.name" required maxlength="80" />
            </label>
            <label class="snippet-field">
              <span>trigger</span>
              <input
                v-model="editor.trigger"
                required
                maxlength="40"
                placeholder="字母、数字、_、-"
              />
              <small v-if="triggerIssue(editor.trigger, everything, editor.id)">
                {{ triggerIssue(editor.trigger, everything, editor.id) }}
              </small>
            </label>
            <label class="snippet-field">
              <span>驱动</span>
              <select v-model="editor.driver">
                <option value="any">通用</option>
                <option value="mysql">MySQL</option>
                <option value="postgres">PostgreSQL</option>
                <option value="sqlite">SQLite</option>
              </select>
            </label>
            <label class="snippet-field">
              <span>标签</span>
              <input
                v-model="editorTagsText"
                placeholder="用逗号分隔，例如 查询, 统计"
              />
            </label>
            <label class="snippet-field wide">
              <span>SQL 正文</span>
              <textarea
                v-model="editor.body"
                rows="12"
                spellcheck="false"
                placeholder="使用 ${cursor} 标记插入后的光标位置"
              ></textarea>
              <small class="hint">
                ${cursor} 只处理第一个；插入是一个编辑器事务，可一次撤销。
              </small>
            </label>
          </div>
          <p v-if="editorError" class="message error">{{ editorError }}</p>
          <div class="confirm-actions">
            <button type="button" class="button ghost" @click="editor = null">
              取消
            </button>
            <button class="button primary">保存片段</button>
          </div>
        </form>
      </div>

      <ConfirmDialog
        :open="!!deleteTarget"
        title="删除 SQL 片段"
        :message="`确定删除「${deleteTarget?.name ?? ''}」？`"
        detail="删除只影响本机片段库；建议先导出 JSON 备份。"
        confirm-text="删除"
        danger
        @confirm="removeSnippet"
        @cancel="deleteTarget = null"
      />

      <ConfirmDialog
        :open="!!insertTarget"
        title="驱动不匹配"
        :message="`「${insertTarget?.name ?? ''}」标记为 ${insertTarget ? driverLabel(insertTarget.driver) : ''} 片段。`"
        :detail="`当前连接是 ${driverLabel(currentDriver)}，插入后可能无法执行。仍要插入吗？`"
        confirm-text="仍然插入"
        @confirm="confirmInsertAnyway"
        @cancel="insertTarget = null"
      />
    </div>
  </div>
</template>

<style scoped>
.snippet-modal {
  width: min(960px, calc(100vw - 48px));
  max-height: calc(100vh - 56px);
  display: flex;
  flex-direction: column;
  position: relative;
}

.snippet-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.snippet-search {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  max-width: 320px;
  padding: 5px 8px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--surface-well);
}

.snippet-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 12px;
}

.snippet-message {
  margin: 8px 20px 0;
}

.snippet-empty {
  padding: 32px 20px;
}

.snippet-layout {
  display: grid;
  grid-template-columns: minmax(240px, 0.8fr) minmax(0, 1.2fr);
  gap: 12px;
  min-height: 0;
  padding: 12px 20px 4px;
}

.snippet-list {
  max-height: 430px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.snippet-row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 7px 9px;
  border: 0;
  border-bottom: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-size: 12px;
}

.snippet-row:hover,
.snippet-row.selected {
  background: var(--accent-soft);
  color: var(--text);
}

.snippet-row-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.snippet-trigger {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--panel-3);
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 10px;
}

.snippet-builtin {
  color: var(--faint);
  font-size: 9px;
}

.snippet-driver-mismatch {
  color: var(--warning, #d29922);
  font-size: 9px;
}

.snippet-row-meta {
  color: var(--faint);
  font-size: 9px;
}

.snippet-preview {
  max-height: 430px;
  overflow: auto;
  padding: 10px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.snippet-preview-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.snippet-preview h3 {
  margin: 0;
  font-size: 13px;
}

.snippet-preview-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 3px;
  color: var(--faint);
  font-size: 10px;
}

.snippet-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 8px 0;
}

.snippet-tags span {
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--panel-3);
  color: var(--muted);
  font-size: 9px;
}

.snippet-mismatch-note {
  margin: 8px 0;
  padding: 6px 8px;
  border-radius: 6px;
  background: rgba(210, 153, 34, 0.1);
  color: var(--warning, #d29922);
  font-size: 10px;
}

.snippet-body {
  margin: 8px 0;
  padding: 10px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.snippet-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.snippet-foot {
  margin: 4px 20px 14px;
}

.snippet-editor-backdrop {
  position: absolute;
  inset: 0;
  z-index: 6;
  display: grid;
  place-items: center;
  padding: 18px;
  border-radius: inherit;
  background: rgba(0, 0, 0, 0.55);
}

.snippet-editor {
  width: min(720px, 100%);
  max-height: calc(100vh - 100px);
  overflow: auto;
  padding-bottom: 14px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--panel);
  box-shadow: var(--shadow-overlay);
}

.snippet-form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 9px 12px;
  padding: 2px 20px 8px;
}

.snippet-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--text-secondary);
  font-size: 11px;
}

.snippet-field.wide {
  grid-column: 1 / -1;
}

.snippet-field input,
.snippet-field select,
.snippet-field textarea {
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 12px;
}

.snippet-field textarea {
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
  resize: vertical;
}

.snippet-field small {
  color: var(--danger);
  font-size: 10px;
}

.snippet-editor .message,
.snippet-editor .confirm-actions {
  margin: 8px 20px;
}
</style>