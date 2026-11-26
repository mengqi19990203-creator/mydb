<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { formatGridCell } from "../lib/grid-search";
import type { RowDetailField } from "../lib/types";
import Icon from "./Icon.vue";

const props = defineProps<{
  open: boolean;
  title: string;
  rowLabel: string;
  fields: RowDetailField[];
  editable: boolean;
  readOnlyReason: string;
  rowState?: "clean" | "update" | "delete" | "insert";
  canPrevious: boolean;
  canNext: boolean;
}>();

const emit = defineEmits<{
  close: [];
  previous: [];
  next: [];
  edit: [index: number, value: string];
  "set-null": [index: number];
  "view-value": [index: number];
  "copy-value": [index: number];
  "copy-row-json": [];
}>();

const editingIndex = ref<number | null>(null);
const draft = ref("");
const rootRef = ref<HTMLElement | null>(null);

const stateLabel = computed(() => {
  if (props.rowState === "delete") return "已标记删除";
  if (props.rowState === "insert") return "新增草稿";
  if (props.rowState === "update") return "待提交修改";
  return "";
});

const valueText = (field: RowDetailField): string =>
  formatGridCell(field.value);

const canSetNull = (field: RowDetailField): boolean =>
  Boolean(field.editable && field.nullable && field.value !== null);

const nullReason = (field: RowDetailField): string => {
  if (!field.nullable) return "字段不允许 NULL";
  if (!field.editable) return field.disabledReason || "当前不可编辑";
  return "";
};

async function startEdit(index: number) {
  const field = props.fields[index];
  if (!field?.editable) return;
  editingIndex.value = index;
  draft.value =
    field.value === null || field.value === undefined
      ? ""
      : typeof field.value === "object"
        ? JSON.stringify(field.value)
        : String(field.value);
  await nextTick();
  const input = rootRef.value?.querySelector<HTMLInputElement>(
    ".row-detail-input",
  );
  input?.focus();
  input?.select();
}

function cancelEdit() {
  editingIndex.value = null;
  draft.value = "";
}

function commitEdit() {
  const index = editingIndex.value;
  if (index === null) return;
  const value = draft.value;
  editingIndex.value = null;
  draft.value = "";
  emit("edit", index, value);
}

watch(
  () => props.open,
  (open) => {
    if (!open) cancelEdit();
  },
);

watch(
  () => [props.fields.map((field) => field.name).join("|"), props.rowLabel],
  () => cancelEdit(),
);

function close() {
  cancelEdit();
  emit("close");
}

function onEditorKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    cancelEdit();
    return;
  }
  if (event.key === "Enter") {
    event.preventDefault();
    event.stopPropagation();
    commitEdit();
  }
}
</script>

<template>
  <aside
    v-if="open"
    ref="rootRef"
    class="row-detail-panel"
    role="complementary"
    aria-label="行详情"
  >
    <header class="row-detail-header">
      <div class="row-detail-heading">
        <strong>{{ title }}</strong>
        <span class="hint">{{ rowLabel }}</span>
      </div>
      <div class="row-detail-actions">
        <button
          class="icon-button"
          title="上一行"
          aria-label="上一行"
          :disabled="!canPrevious"
          @click="emit('previous')"
        >
          <Icon name="chevron-up" :size="14" />
        </button>
        <button
          class="icon-button"
          title="下一行"
          aria-label="下一行"
          :disabled="!canNext"
          @click="emit('next')"
        >
          <Icon name="chevron-down" :size="14" />
        </button>
        <button
          class="icon-button"
          title="关闭行详情（Esc）"
          aria-label="关闭行详情"
          @click="close"
        >
          <Icon name="close" :size="14" />
        </button>
      </div>
    </header>

    <div class="row-detail-meta">
      <span v-if="stateLabel" class="row-detail-state">{{ stateLabel }}</span>
      <span v-if="!editable" class="row-detail-lock">
        <Icon name="key" :size="12" />
        {{ readOnlyReason || "只读" }}
      </span>
      <button
        class="link-button"
        title="复制整行 JSON（全部列）"
        @click="emit('copy-row-json')"
      >
        复制整行 JSON
      </button>
    </div>

    <div class="row-detail-fields">
      <div
        v-for="(field, index) in fields"
        :key="`${field.name}-${index}`"
        class="row-detail-field"
        :class="{ modified: field.modified }"
      >
        <div class="row-detail-field-head">
          <div class="row-detail-field-name">
            <span>{{ field.name }}</span>
            <span v-if="field.type" class="row-detail-type">{{ field.type }}</span>
            <span v-if="field.primaryKey" class="row-detail-badge">PK</span>
            <span v-if="field.autoIncrement" class="row-detail-badge">AI</span>
            <span v-if="field.nullable" class="row-detail-badge muted">NULL</span>
            <span v-if="field.hidden" class="row-detail-badge muted">已隐藏</span>
            <span v-if="field.modified" class="row-detail-badge accent">已修改</span>
          </div>
          <div class="row-detail-field-actions">
            <button
              v-if="field.editable && editingIndex !== index"
              class="icon-button"
              title="编辑此字段"
              :aria-label="`编辑 ${field.name}`"
              @click="startEdit(index)"
            >
              <Icon name="edit" :size="12" />
            </button>
            <button
              v-if="field.editable && editingIndex !== index"
              class="icon-button"
              :title="nullReason(field) || '设为 NULL'"
              :aria-label="`将 ${field.name} 设为 NULL`"
              :disabled="!canSetNull(field)"
              @click="emit('set-null', index)"
            >
              <span class="row-detail-null-btn">NULL</span>
            </button>
            <button
              class="icon-button"
              title="查看值…"
              :aria-label="`查看 ${field.name} 的值`"
              @click="emit('view-value', index)"
            >
              <Icon name="view" :size="12" />
            </button>
            <button
              class="icon-button"
              title="复制值"
              :aria-label="`复制 ${field.name} 的值`"
              @click="emit('copy-value', index)"
            >
              <Icon name="copy" :size="12" />
            </button>
          </div>
        </div>
        <div
          class="row-detail-value"
          :class="{ null: field.value === null || field.value === undefined }"
          :title="valueText(field)"
          @dblclick="startEdit(index)"
        >
          <input
            v-if="editingIndex === index"
            v-model="draft"
            class="row-detail-input"
            :aria-label="`编辑 ${field.name}`"
            @keydown="onEditorKeydown"
            @blur="commitEdit"
          />
          <template v-else>
            {{ field.binary ? "<BLOB>" : valueText(field) }}
          </template>
        </div>
        <div v-if="field.comment" class="row-detail-comment">
          {{ field.comment }}
        </div>
        <div
          v-if="field.editable === false && field.disabledReason"
          class="row-detail-comment"
        >
          {{ field.disabledReason }}
        </div>
      </div>
    </div>
  </aside>
</template>