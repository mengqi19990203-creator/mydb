<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import type { ColumnInfo } from "../lib/types";

const props = defineProps<{
  open: boolean;
  columns: ColumnInfo[];
  selectedCount: number;
}>();

const emit = defineEmits<{
  close: [];
  apply: [column: string, value: string, useNull: boolean];
}>();

const columnName = ref("");
const value = ref("");
const useNull = ref(false);

const selectedColumn = computed(() =>
  props.columns.find((column) => column.name === columnName.value),
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    columnName.value = props.columns[0]?.name ?? "";
    value.value = "";
    useNull.value = false;
  },
  { immediate: true },
);

function submit() {
  if (!columnName.value) return;
  emit("apply", columnName.value, value.value, useNull.value);
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal connection-modal"
      role="dialog"
      aria-modal="true"
      aria-label="批量设置值"
    >
      <header class="modal-header">
        <div>
          <h2>批量设置值</h2>
          <p>将应用到当前选中的 {{ selectedCount }} 行，先进入待提交缓冲区</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>
      <div class="connection-form">
        <div class="form-grid">
          <label class="span-2">
            字段
            <select v-model="columnName">
              <option
                v-for="column in columns"
                :key="column.name"
                :value="column.name"
              >
                {{ column.name }} · {{ column.dataType }}
              </option>
            </select>
          </label>
          <label class="checkbox-row span-2">
            <input v-model="useNull" type="checkbox" />
            设置为 NULL
          </label>
          <label class="span-2">
            值
            <input
              v-model="value"
              :disabled="useNull"
              :placeholder="
                selectedColumn?.nullable
                  ? '留空表示 NULL；输入 NULL 也表示 NULL'
                  : '必填'
              "
              @keydown.enter.prevent="submit"
            />
          </label>
        </div>
        <p class="draft-hint">
          值会按字段类型解析（数字、NULL、字符串）；二进制字段不提供批量设置。
        </p>
      </div>
      <footer class="modal-footer">
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button
          class="button primary"
          :disabled="!columnName"
          @click="submit"
        >
          加入待提交
        </button>
      </footer>
    </div>
  </div>
</template>