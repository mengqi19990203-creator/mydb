<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { ExportFormat } from "../lib/api";

const props = defineProps<{
  open: boolean;
  kind: string;
  name: string;
}>();

const emit = defineEmits<{
  confirm: [format: ExportFormat, limit: number | null, withHeader: boolean];
  cancel: [];
}>();

const format = ref<ExportFormat>("csv");
const scope = ref<"all" | "custom">("all");
const customLimit = ref(1000);
const withHeader = ref(true);
const error = ref("");

const isDdl = computed(() => format.value === "ddl");
const kindLabel = computed(() => (props.kind === "view" ? "视图" : "表"));

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("cancel");
  }
}

function submit() {
  if (isDdl.value || scope.value === "all") {
    emit("confirm", format.value, null, withHeader.value);
    return;
  }
  const value = Math.floor(Number(customLimit.value));
  if (!Number.isFinite(value) || value < 1) {
    error.value = "行数必须大于 0";
    return;
  }
  error.value = "";
  emit("confirm", format.value, value, withHeader.value);
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      format.value = "csv";
      scope.value = "all";
      customLimit.value = 1000;
      withHeader.value = true;
      error.value = "";
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('cancel')">
    <div
      class="modal confirm-modal prompt-modal export-modal"
      role="dialog"
      aria-modal="true"
      :aria-label="`导出${kindLabel}`"
    >
      <h2>导出{{ kindLabel }}「{{ name }}」</h2>

      <div class="export-field">
        <span class="export-label">格式</span>
        <select v-model="format">
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
          <option value="sql">SQL INSERT</option>
          <option value="ddl">结构 DDL</option>
          <option value="xlsx">Excel 工作簿（XLSX）</option>
        </select>
      </div>

      <div v-if="!isDdl" class="export-field">
        <span class="export-label">行范围</span>
        <div class="export-scope">
          <label>
            <input v-model="scope" type="radio" value="all" />
            <span>全部行</span>
          </label>
          <label>
            <input v-model="scope" type="radio" value="custom" />
            <span>指定行数</span>
          </label>
          <input
            v-model.number="customLimit"
            class="export-limit"
            type="number"
            min="1"
            step="1"
            :disabled="scope !== 'custom'"
          />
          <span class="export-unit">行</span>
        </div>
      </div>

      <div v-if="format === 'csv' || format === 'xlsx'" class="export-field">
        <span class="export-label">表头</span>
        <label class="export-check">
          <input v-model="withHeader" type="checkbox" />
          <span>包含表头行</span>
        </label>
      </div>

      <p v-if="error" class="message error">{{ error }}</p>

      <div class="confirm-actions">
        <button class="button ghost" @click="emit('cancel')">取消</button>
        <button class="button primary" @click="submit">导出…</button>
      </div>
    </div>
  </div>
</template>