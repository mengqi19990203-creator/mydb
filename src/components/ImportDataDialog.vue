<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { TableInfo } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  tables: TableInfo[];
  initialTable?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  imported: [table: string, count: number];
}>();

const table = ref("");
const path = ref("");
const format = ref<"csv" | "json">("csv");
const hasHeader = ref(true);
const importing = ref(false);
const error = ref("");
const message = ref("");

const tableOptions = computed(() =>
  props.tables.filter((item) => item.kind === "table"),
);

const fileName = computed(() => {
  const parts = path.value.split(/[\\/]/);
  return parts[parts.length - 1] || "";
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const preferred = props.initialTable
      ? tableOptions.value.find((item) => item.name === props.initialTable)
      : null;
    table.value = preferred?.name ?? tableOptions.value[0]?.name ?? "";
    path.value = "";
    format.value = "csv";
    hasHeader.value = true;
    importing.value = false;
    error.value = "";
    message.value = "";
  },
  { immediate: true },
);

async function chooseFile() {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "CSV / JSON", extensions: ["csv", "json"] }],
  });
  if (typeof selected !== "string") return;
  path.value = selected;
  format.value = selected.toLowerCase().endsWith(".json") ? "json" : "csv";
  error.value = "";
  message.value = "";
}

async function importData() {
  if (importing.value) return;
  error.value = "";
  message.value = "";
  if (props.readOnly) {
    error.value = "当前连接为只读模式，已阻止写入";
    return;
  }
  if (!table.value) {
    error.value = "请选择目标表";
    return;
  }
  if (!path.value) {
    error.value = "请选择要导入的文件";
    return;
  }

  importing.value = true;
  try {
    const count =
      format.value === "json"
        ? await api.importJson(props.connectionId, table.value, path.value)
        : await api.importCsv(
            props.connectionId,
            table.value,
            path.value,
            hasHeader.value,
          );
    message.value = `已导入 ${count} 行到 ${table.value}`;
    emit("imported", table.value, count);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    importing.value = false;
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal import-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="import-data-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="import-data-title">导入数据</h2>
          <p>CSV 按表头匹配列名；JSON 支持对象数组或 {"rows": [...]}。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label>
            目标表
            <select v-model="table" :disabled="!tableOptions.length">
              <option
                v-for="item in tableOptions"
                :key="item.name"
                :value="item.name"
              >
                {{ item.name }}
              </option>
            </select>
          </label>
          <label>
            文件
            <div class="import-file">
              <input
                :value="fileName"
                readonly
                placeholder="选择 CSV / JSON 文件"
              />
              <button class="button ghost compact" @click="chooseFile">
                选择…
              </button>
            </div>
          </label>
          <label v-if="format === 'csv'" class="checkbox-row span-2">
            <input v-model="hasHeader" type="checkbox" />
            第一行是表头
          </label>
        </div>

        <p class="draft-hint">
          导入逐行执行，不会自动开启事务；空单元格写入 NULL，JSON 中不存在的字段使用默认值。只读连接会被后端拒绝。
        </p>
        <p v-if="message" class="message success">{{ message }}</p>
        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" @click="emit('close')">关闭</button>
        <div class="spacer"></div>
        <button
          class="button primary"
          :disabled="
            importing ||
            readOnly ||
            !tableOptions.length ||
            !path
          "
          @click="importData"
        >
          {{ importing ? "导入中…" : "开始导入" }}
        </button>
      </footer>
    </div>
  </div>
</template>