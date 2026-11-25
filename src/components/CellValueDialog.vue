<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";

const MAX_PREVIEW_BYTES = 2 * 1024 * 1024;

const props = defineProps<{
  open: boolean;
  column: string;
  rowLabel?: string;
  dataType?: string;
  value: unknown;
}>();

const emit = defineEmits<{
  close: [];
}>();

const view = ref<"formatted" | "raw">("raw");
const wrap = ref(true);
const busy = ref(false);
const error = ref("");
const message = ref("");
let messageTimer = 0;

const isNull = computed(
  () => props.value === null || props.value === undefined,
);

const rawText = computed(() => {
  if (isNull.value) return "";
  return typeof props.value === "object"
    ? JSON.stringify(props.value)
    : String(props.value);
});

const parsedJson = computed<{ ok: boolean; pretty: string }>(() => {
  if (isNull.value) return { ok: false, pretty: "" };
  if (typeof props.value === "object") {
    try {
      return { ok: true, pretty: JSON.stringify(props.value, null, 2) };
    } catch {
      return { ok: false, pretty: "" };
    }
  }
  if (typeof props.value !== "string") return { ok: false, pretty: "" };
  const text = props.value.trim();
  if (!text.startsWith("{") && !text.startsWith("[")) {
    return { ok: false, pretty: "" };
  }
  try {
    return { ok: true, pretty: JSON.stringify(JSON.parse(text), null, 2) };
  } catch {
    return { ok: false, pretty: "" };
  }
});

const isJson = computed(() => parsedJson.value.ok);

const blobInfo = computed(() => {
  if (typeof props.value !== "string") return null;
  const match = /^<BLOB (\d+) bytes>$/.exec(props.value);
  return match ? Number(match[1]) : null;
});

const text = computed(() => {
  if (isNull.value) return "NULL";
  if (blobInfo.value !== null) return props.value as string;
  if (view.value === "formatted" && isJson.value) return parsedJson.value.pretty;
  return rawText.value;
});

const sizeBytes = computed(() => {
  if (isNull.value) return 0;
  if (typeof props.value === "string") {
    return new TextEncoder().encode(props.value).length;
  }
  return new TextEncoder().encode(rawText.value).length;
});

const tooLarge = computed(() => sizeBytes.value > MAX_PREVIEW_BYTES);
const charCount = computed(() => (isNull.value ? 0 : text.value.length));
const sizeLabel = computed(() => formatBytes(sizeBytes.value));
const canSave = computed(() => !isNull.value && !tooLarge.value);

const typeLabel = computed(() => props.dataType?.trim() ?? "");

const notice = computed(() => {
  if (isNull.value) return "该值为 NULL，不能另存为文件。";
  if (blobInfo.value !== null) {
    return "二进制值仅显示占位，不进行解码。";
  }
  if (tooLarge.value) {
    return "内容超过 2 MB，仅提供只读预览；请使用查询导出获取完整内容。";
  }
  if (view.value === "formatted" && !isJson.value) {
    return "不是合法 JSON，按原文显示。";
  }
  return "";
});

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function flash(text: string) {
  message.value = text;
  window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(() => {
    message.value = "";
  }, 1400);
}

async function copy() {
  if (isNull.value || tooLarge.value) return;
  try {
    await writeClipboard(text.value);
    flash("已复制当前视图");
  } catch {
    flash("复制失败");
  }
}

async function saveAs() {
  if (!canSave.value || busy.value) return;
  const extension = view.value === "formatted" && isJson.value ? "json" : "txt";
  const path = await saveDialog({
    defaultPath: `${props.column || "value"}.${extension}`,
    filters: [
      {
        name: extension === "json" ? "JSON" : "文本",
        extensions: [extension],
      },
    ],
  });
  if (typeof path !== "string") return;

  busy.value = true;
  error.value = "";
  try {
    await api.writeTextFile(path, text.value);
    flash(`已保存到 ${path}`);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    busy.value = false;
  }
}

function close() {
  if (busy.value) return;
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    close();
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      view.value = "formatted";
      wrap.value = true;
      error.value = "";
      message.value = "";
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
  <div v-if="open" class="modal-backdrop value-viewer-backdrop" @click.self="close">
    <div
      class="modal value-viewer"
      role="dialog"
      aria-modal="true"
      :aria-label="`查看值：${column}`"
    >
      <header class="modal-header value-viewer-head">
        <div class="value-viewer-title">
          <h2>{{ column || "值" }}</h2>
          <span v-if="rowLabel" class="value-viewer-row">{{ rowLabel }}</span>
          <span v-if="typeLabel" class="value-viewer-type">{{ typeLabel }}</span>
          <span class="value-viewer-meta">
            {{ charCount }} 字符 · {{ sizeLabel }}
          </span>
        </div>
        <button class="icon-button" title="关闭" @click="close">
          <Icon name="close" />
        </button>
      </header>

      <div class="value-viewer-toolbar">
        <div class="segmented">
          <button
            class="segmented-item"
            :class="{ active: view === 'formatted' }"
            :disabled="!isJson && !isNull"
            @click="view = 'formatted'"
          >
            格式化
          </button>
          <button
            class="segmented-item"
            :class="{ active: view === 'raw' || (!isJson && !isNull) }"
            @click="view = 'raw'"
          >
            原始
          </button>
        </div>
        <label class="value-viewer-toggle">
          <input v-model="wrap" type="checkbox" />
          <span>自动换行</span>
        </label>
        <div class="value-viewer-actions">
          <button
            class="button ghost compact"
            :disabled="isNull || tooLarge"
            @click="copy"
          >
            <Icon name="copy" :size="14" />
            复制
          </button>
          <button
            class="button ghost compact"
            :disabled="!canSave || busy"
            @click="saveAs"
          >
            <Icon name="download" :size="14" />
            {{ busy ? "保存中…" : "另存为…" }}
          </button>
        </div>
      </div>

      <p v-if="notice" class="value-viewer-notice">{{ notice }}</p>
      <p v-if="error" class="message error">{{ error }}</p>

      <div
        class="value-viewer-body"
        :class="{ 'no-wrap': !wrap, null: isNull || blobInfo !== null }"
        tabindex="0"
      >
        <pre>{{ text }}</pre>
      </div>

      <footer class="value-viewer-foot">
        <span v-if="message">{{ message }}</span>
        <span v-else>只读预览，不会修改单元格或数据库。</span>
        <span class="value-viewer-esc">Esc 关闭</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.value-viewer-backdrop {
  z-index: 60;
}

.value-viewer {
  width: min(860px, calc(100vw - 56px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
  padding-bottom: 0;
}

.value-viewer-head {
  align-items: flex-start;
}

.value-viewer-title {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.value-viewer-title h2 {
  margin: 0;
  font-size: 15px;
  font-family: var(--font-mono);
}

.value-viewer-row,
.value-viewer-type {
  padding: 1px 6px;
  border-radius: 5px;
  background: var(--panel-3);
  color: var(--muted);
  font-size: 10px;
}

.value-viewer-meta {
  color: var(--faint);
  font-size: 10px;
}

.value-viewer-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 20px 10px;
}

.value-viewer-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 11px;
}

.value-viewer-actions {
  margin-left: auto;
  display: flex;
  gap: 6px;
}

.value-viewer-notice {
  margin: 0 20px 8px;
  padding: 6px 9px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--text-secondary);
  font-size: 11px;
}

.value-viewer-body {
  margin: 0 20px;
  min-height: 160px;
  max-height: calc(100vh - 300px);
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--surface-well);
}

.value-viewer-body pre {
  margin: 0;
  padding: 12px 14px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  tab-size: 2;
}

.value-viewer-body.no-wrap pre {
  white-space: pre;
  overflow-wrap: normal;
}

.value-viewer-body.null pre {
  color: var(--faint);
  font-style: italic;
}

.value-viewer-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px 14px;
  color: var(--faint);
  font-size: 10px;
}

.value-viewer-esc {
  margin-left: auto;
}
</style>