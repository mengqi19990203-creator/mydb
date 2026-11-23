<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { buildCreateIndexStatement } from "../lib/alter";
import type { ColumnInfo, Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  table: string;
  columns: ColumnInfo[];
}>();

const emit = defineEmits<{
  close: [];
  done: [message: string];
}>();

const name = ref("");
const unique = ref(false);
const selected = ref<string[]>([]);
const executing = ref(false);
const error = ref("");

const driverLabel = computed(() => {
  switch (props.driver) {
    case "mysql":
      return "MySQL / MariaDB";
    case "postgres":
      return "PostgreSQL";
    default:
      return "SQLite";
  }
});

const preview = computed(() => {
  try {
    const statement = buildCreateIndexStatement(props.driver, props.table, {
      name: name.value,
      columns: selected.value,
      unique: unique.value,
    });
    return { statement, error: "" };
  } catch (reason) {
    return {
      statement: "",
      error: reason instanceof Error ? reason.message : String(reason),
    };
  }
});

const previewText = computed(
  () => preview.value.statement || `-- ${preview.value.error}`,
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const first = props.columns[0]?.name ?? "column";
    name.value = `idx_${props.table}_${first}`;
    unique.value = false;
    selected.value = [];
    executing.value = false;
    error.value = "";
  },
  { immediate: true },
);

function toggleColumn(column: string) {
  if (selected.value.includes(column)) {
    selected.value = selected.value.filter((item) => item !== column);
  } else {
    selected.value = [...selected.value, column];
  }
}

async function submit() {
  if (executing.value) return;
  if (preview.value.error) {
    error.value = preview.value.error;
    return;
  }

  executing.value = true;
  error.value = "";
  try {
    await api.executeQuery(props.connectionId, preview.value.statement, 1);
    emit("done", `索引 ${name.value.trim()} 已创建`);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    executing.value = false;
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal create-table-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="index-editor-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="index-editor-title">添加索引</h2>
          <p>{{ table }} · {{ driverLabel }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label>
            索引名
            <input v-model="name" placeholder="例如：idx_orders_user_id" />
          </label>
          <label class="checkbox-row">
            <input v-model="unique" type="checkbox" />
            唯一索引
          </label>
        </div>

        <div class="index-column-list">
          <label
            v-for="column in columns"
            :key="column.name"
            class="inline-check"
          >
            <input
              type="checkbox"
              :checked="selected.includes(column.name)"
              @change="toggleColumn(column.name)"
            />
            {{ column.name }}
            <span class="hint">{{ column.dataType }}</span>
          </label>
        </div>
        <p v-if="!columns.length" class="draft-hint">当前表没有可用字段。</p>

        <section class="ddl-section">
          <h3>SQL 预览</h3>
          <pre class="ddl-preview">{{ previewText }}</pre>
        </section>

        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button
          class="button primary"
          :disabled="executing || Boolean(preview.error)"
          @click="submit"
        >
          {{ executing ? "创建中…" : "执行创建" }}
        </button>
      </footer>
    </div>
  </div>
</template>