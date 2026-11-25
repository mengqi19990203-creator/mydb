<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { Driver, ObjectVariant } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  kind: string;
  name: string;
  readOnly: boolean;
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const variants = ref<ObjectVariant[]>([]);
const identity = ref("");
const ddl = ref("");
const loading = ref(false);
const error = ref("");
const search = ref("");
const matchIndex = ref(0);
const editing = ref(false);
const previewing = ref(false);
const editText = ref("");
const saving = ref(false);
const saveError = ref("");
const notice = ref("");

const kindLabels: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
  procedure: "存储过程",
  function: "函数",
  trigger: "触发器",
  event: "事件",
  index: "索引",
  sequence: "序列",
};

const kindLabel = computed(() => kindLabels[props.kind] ?? props.kind);
const byteLength = computed(() => new TextEncoder().encode(ddl.value).length);
const tooLarge = computed(() => byteLength.value > 2 * 1024 * 1024);

const editable = computed(() => {
  if (props.readOnly || tooLarge.value || loading.value || !ddl.value) return false;
  if (props.driver === "mysql") return props.kind === "view";
  if (props.driver === "postgres") {
    return ["view", "function", "procedure"].includes(props.kind);
  }
  if (props.driver === "sqlite") {
    return ["view", "trigger"].includes(props.kind);
  }
  return false;
});

const editDisabledReason = computed(() => {
  if (props.readOnly) return "当前连接为只读";
  if (tooLarge.value) return "定义超过 2 MB，只读展示";
  if (props.driver === "mysql") {
    return "MySQL 存储过程 / 函数 / 触发器 / 事件暂不支持应用内编辑";
  }
  if (!editable.value) return "该对象类型不支持应用内编辑";
  return "";
});

const needsVariant = computed(
  () => variants.value.length > 1 && !identity.value,
);

const dropNotice = computed(() => {
  if (props.driver !== "sqlite") return "";
  return props.kind === "view"
    ? "保存将在一个事务内执行 DROP VIEW + CREATE VIEW；失败会回滚。"
    : "保存将在一个事务内执行 DROP TRIGGER + CREATE TRIGGER；失败会回滚。";
});

const searchMatched = computed(() => {
  const term = search.value.trim().toLowerCase();
  if (!term) return 0;
  return ddl.value.toLowerCase().split(term).length - 1;
});

const lines = computed(() => ddl.value.split("\n"));

function segments(line: string, lineIndex: number) {
  const term = search.value.trim();
  if (!term) {
    return [{ text: line || " ", match: false, key: `${lineIndex}-plain` }];
  }
  const lower = line.toLowerCase();
  const needle = term.toLowerCase();
  const parts: { text: string; match: boolean }[] = [];
  let cursor = 0;
  while (cursor <= line.length) {
    const index = lower.indexOf(needle, cursor);
    if (index < 0) {
      if (cursor < line.length) parts.push({ text: line.slice(cursor), match: false });
      break;
    }
    if (index > cursor) parts.push({ text: line.slice(cursor, index), match: false });
    parts.push({ text: line.slice(index, index + term.length), match: true });
    cursor = index + term.length;
  }
  if (!parts.length) parts.push({ text: " ", match: false });
  return parts.map((part) => ({
    ...part,
    key: `${lineIndex}-${part.text}-${part.match}`,
  }));
}

function selectVariant() {
  if (!identity.value) return;
  void loadDdl();
}

async function loadDdl() {
  loading.value = true;
  error.value = "";
  notice.value = "";
  try {
    ddl.value = await api.getObjectDdl(
      props.connectionId,
      props.kind,
      props.name,
      identity.value || null,
    );
    matchIndex.value = 0;
  } catch (reason) {
    ddl.value = "";
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function load() {
  variants.value = [];
  identity.value = "";
  ddl.value = "";
  error.value = "";
  editing.value = false;
  previewing.value = false;
  saveError.value = "";
  search.value = "";
  notice.value = "";
  loading.value = true;
  try {
    variants.value = await api.listObjectVariants(
      props.connectionId,
      props.kind,
      props.name,
    );
    if (variants.value.length > 1) {
      loading.value = false;
      return;
    }
    identity.value = variants.value[0]?.id ?? "";
    loading.value = false;
    await loadDdl();
  } catch (reason) {
    error.value = String(reason);
    loading.value = false;
  }
}

async function copyDdl() {
  try {
    await writeClipboard(ddl.value);
    notice.value = "已复制定义";
  } catch {
    notice.value = "复制失败";
  }
}

async function saveAs() {
  const path = await saveDialog({
    defaultPath: `${props.name}.sql`,
    filters: [{ name: "SQL", extensions: ["sql"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, ddl.value);
    notice.value = `已保存到 ${path}`;
  } catch (reason) {
    error.value = String(reason);
  }
}

function startEdit() {
  if (!editable.value) return;
  editText.value = ddl.value;
  editing.value = true;
  previewing.value = false;
  saveError.value = "";
}

function preview() {
  if (!editText.value.trim()) {
    saveError.value = "定义不能为空";
    return;
  }
  previewing.value = true;
  saveError.value = "";
}

async function save() {
  if (saving.value) return;
  saving.value = true;
  saveError.value = "";
  try {
    await api.saveObjectDefinition(
      props.connectionId,
      props.kind,
      props.name,
      editText.value,
    );
    editing.value = false;
    previewing.value = false;
    notice.value = "定义已保存";
    emit("saved");
    await load();
  } catch (reason) {
    saveError.value = String(reason);
  } finally {
    saving.value = false;
  }
}

function nextMatch() {
  if (!searchMatched.value) return;
  const matches = Array.from(
    document.querySelectorAll<HTMLElement>(".source-match"),
  );
  if (!matches.length) return;
  const index = (matchIndex.value + 1) % matches.length;
  matchIndex.value = index;
  void nextTick(() => matches[index]?.scrollIntoView({ block: "center" }));
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    if (previewing.value) {
      event.preventDefault();
      previewing.value = false;
      return;
    }
    if (editing.value) {
      event.preventDefault();
      editing.value = false;
      return;
    }
    if (!muteClose.value) {
      event.preventDefault();
      emit("close");
    }
  }
}

// The close button / backdrop always work; Escape is suppressed while a
// nested confirm-like panel is open, handled separately above.
const muteClose = ref(false);

watch(
  () => props.open,
  (open) => {
    if (open) {
      document.addEventListener("keydown", onKeydown, true);
      void load();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

watch(
  () => [props.connectionId, props.kind, props.name],
  () => {
    if (props.open) void load();
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal source-modal"
      role="dialog"
      aria-modal="true"
      :aria-label="`查看定义：${name}`"
    >
      <header class="modal-header">
        <div>
          <h2>查看定义 · {{ kindLabel }}「{{ name }}」</h2>
          <p>只读查看；保存前会先预览完整 SQL。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="source-toolbar">
        <select
          v-if="variants.length > 1"
          v-model="identity"
          aria-label="选择重载签名"
          @change="selectVariant"
        >
          <option value="">请选择函数 / 存储过程签名…</option>
          <option v-for="variant in variants" :key="variant.id" :value="variant.id">
            {{ variant.label }}
          </option>
        </select>
        <div class="source-search">
          <Icon name="search" :size="14" />
          <input v-model="search" placeholder="在定义中搜索" spellcheck="false" />
          <span class="hint">
            {{ searchMatched ? `${searchMatched} 处` : "" }}
          </span>
          <button
            class="button ghost compact"
            :disabled="!searchMatched"
            @click="nextMatch"
          >
            下一个
          </button>
        </div>
        <span class="spacer"></span>
        <button class="button ghost compact" :disabled="!ddl" @click="copyDdl">
          <Icon name="copy" :size="14" />
          复制
        </button>
        <button class="button ghost compact" :disabled="!ddl" @click="saveAs">
          <Icon name="download" :size="14" />
          另存为…
        </button>
        <button
          class="button primary compact"
          :disabled="!editable"
          :title="editDisabledReason || '编辑定义'"
          @click="startEdit"
        >
          <Icon name="code" :size="14" />
          编辑
        </button>
      </div>

      <p v-if="readOnly" class="source-notice">
        只读连接：可以查看和复制定义，保存会被后端拒绝。
      </p>
      <p v-if="tooLarge" class="source-notice">
        定义超过 2 MB，仅只读展示，不能编辑或保存。
      </p>
      <p v-if="notice" class="source-notice">{{ notice }}</p>
      <p v-if="error" class="message error source-message">{{ error }}</p>
      <p v-if="needsVariant" class="panel-empty">
        该名称存在多个重载签名，请先选择要查看 / 编辑的签名。
      </p>
      <div v-else-if="loading" class="panel-empty">正在读取定义…</div>

      <div v-else-if="ddl" class="source-body">
        <div class="source-code" tabindex="0">
          <div v-for="(line, lineIndex) in lines" :key="lineIndex" class="source-line">
            <span class="source-line-no">{{ lineIndex + 1 }}</span>
            <span class="source-line-text">
              <template v-for="segment in segments(line, lineIndex)" :key="segment.key">
                <mark v-if="segment.match" class="source-match">{{ segment.text }}</mark>
                <template v-else>{{ segment.text }}</template>
              </template>
            </span>
          </div>
        </div>
      </div>

      <div v-if="editing" class="source-editor">
        <div class="source-editor-head">
          <strong>
            {{ previewing ? "保存预览" : "编辑定义" }}
          </strong>
          <span class="hint">
            目标：{{ kindLabel }}「{{ name }}」 · {{ driver }}
          </span>
        </div>

        <template v-if="previewing">
          <p class="source-notice">
            <template v-if="driver === 'sqlite'">{{ dropNotice }}</template>
            <template v-else>
              将使用 CREATE OR REPLACE 保存；不会自动删除依赖对象或执行 CASCADE。
            </template>
          </p>
          <pre class="source-preview">{{ editText }}</pre>
          <p v-if="saveError" class="message error">{{ saveError }}</p>
          <div class="confirm-actions">
            <button class="button ghost" :disabled="saving" @click="previewing = false">
              返回编辑
            </button>
            <button class="button primary" :disabled="saving" @click="save">
              {{ saving ? "保存中…" : "确认保存" }}
            </button>
          </div>
        </template>

        <template v-else>
          <textarea v-model="editText" class="source-textarea" spellcheck="false"></textarea>
          <p class="hint">
            保存失败会保留编辑内容；原始定义不会被覆盖。只读连接会在后端被拒绝。
          </p>
          <p v-if="saveError" class="message error">{{ saveError }}</p>
          <div class="confirm-actions">
            <button class="button ghost" @click="editing = false">取消</button>
            <button class="button primary" @click="preview">预览保存</button>
          </div>
        </template>
      </div>

      <p v-if="ddl" class="source-foot hint">
        {{ lines.length }} 行 · {{ byteLength }} 字节
      </p>
    </div>
  </div>
</template>

<style scoped>
.source-modal {
  width: min(960px, calc(100vw - 48px));
  max-height: calc(100vh - 56px);
  display: flex;
  flex-direction: column;
}

.source-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.source-toolbar select {
  max-width: 340px;
}

.source-search {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  max-width: 360px;
  padding: 5px 8px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--surface-well);
}

.source-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 12px;
}

.source-notice {
  margin: 8px 20px 0;
  padding: 6px 9px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--text-secondary);
  font-size: 11px;
}

.source-message {
  margin: 8px 20px 0;
}

.source-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  margin: 10px 20px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--surface-well);
}

.source-code {
  padding: 10px 0;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.6;
}

.source-line {
  display: flex;
  min-width: max-content;
}

.source-line-no {
  width: 52px;
  flex: 0 0 52px;
  padding-right: 10px;
  color: var(--faint);
  text-align: right;
  user-select: none;
}

.source-line-text {
  padding-right: 16px;
  white-space: pre;
}

.source-match {
  padding: 0;
  border-radius: 2px;
  background: rgba(210, 153, 34, 0.45);
  color: inherit;
}

.source-editor {
  margin: 10px 20px;
  padding: 12px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  background: var(--panel-2);
}

.source-editor-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 8px;
  font-size: 12px;
}

.source-textarea {
  width: 100%;
  min-height: 220px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  resize: vertical;
}

.source-preview {
  max-height: 280px;
  overflow: auto;
  margin: 8px 0;
  padding: 10px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  white-space: pre-wrap;
}

.source-foot {
  margin: 6px 20px 14px;
}
</style>