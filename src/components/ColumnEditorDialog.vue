<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { buildColumnStatements } from "../lib/alter";
import type { ColumnInfo, Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  table: string;
  column: ColumnInfo | null;
}>();

const emit = defineEmits<{
  close: [];
  done: [message: string];
}>();

const name = ref("");
const type = ref("");
const nullable = ref(true);
const defaultValue = ref("");
const executing = ref(false);
const error = ref("");

const isSqliteEdit = computed(
  () => props.driver === "sqlite" && props.column !== null,
);

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

const typeOptions = computed(() => {
  switch (props.driver) {
    case "mysql":
      return [
        "INT",
        "BIGINT",
        "VARCHAR(255)",
        "TEXT",
        "LONGTEXT",
        "DECIMAL(10,2)",
        "DATETIME",
        "DATE",
        "BOOLEAN",
        "JSON",
      ];
    case "postgres":
      return [
        "INTEGER",
        "BIGINT",
        "TEXT",
        "VARCHAR(255)",
        "NUMERIC(10,2)",
        "TIMESTAMP",
        "TIMESTAMPTZ",
        "DATE",
        "BOOLEAN",
        "JSONB",
        "UUID",
      ];
    default:
      return ["TEXT", "INTEGER", "REAL", "BLOB", "NUMERIC"];
  }
});

const defaultType = computed(() => typeOptions.value[0] ?? "TEXT");

const preview = computed(() => {
  try {
    const statements = buildColumnStatements(
      props.driver,
      props.table,
      props.column,
      {
        name: name.value,
        type: type.value,
        nullable: nullable.value,
        defaultValue: defaultValue.value,
      },
    );
    return { statements, error: "" };
  } catch (reason) {
    return {
      statements: [] as string[],
      error: reason instanceof Error ? reason.message : String(reason),
    };
  }
});

const previewText = computed(() => {
  if (preview.value.statements.length) {
    return preview.value.statements.join("\n");
  }
  return preview.value.error
    ? `-- ${preview.value.error}`
    : "-- 没有需要执行的修改";
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const column = props.column;
    name.value = column?.name ?? "";
    type.value = column?.dataType ?? defaultType.value;
    nullable.value = column?.nullable ?? true;
    defaultValue.value = column?.defaultValue ?? "";
    executing.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function submit() {
  if (executing.value) return;
  if (preview.value.error) {
    error.value = preview.value.error;
    return;
  }
  if (!preview.value.statements.length) {
    error.value = "没有需要执行的修改";
    return;
  }

  executing.value = true;
  error.value = "";
  let completed = 0;
  try {
    for (const statement of preview.value.statements) {
      await api.executeQuery(props.connectionId, statement, 1);
      completed += 1;
    }
    emit("done", props.column ? "字段已更新" : "字段已添加");
  } catch (reason) {
    const prefix =
      completed > 0 ? "部分语句已执行，请关闭后刷新确认。\n" : "";
    error.value = `${prefix}${String(reason)}`;
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
      aria-labelledby="column-editor-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="column-editor-title">
            {{ column ? "编辑字段" : "添加字段" }}
          </h2>
          <p>{{ table }} · {{ driverLabel }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label>
            字段名
            <input v-model="name" placeholder="例如：created_at" />
          </label>
          <label>
            类型
            <input
              v-model="type"
              list="alter-column-types"
              :disabled="isSqliteEdit"
              placeholder="例如：VARCHAR(255)"
            />
          </label>
          <label class="span-2 checkbox-row">
            <input
              v-model="nullable"
              type="checkbox"
              :disabled="isSqliteEdit"
            />
            可空
          </label>
          <label class="span-2">
            默认值
            <input
              v-model="defaultValue"
              :disabled="isSqliteEdit"
              placeholder="SQL 表达式，如 0、'abc'、CURRENT_TIMESTAMP"
            />
          </label>
        </div>

        <datalist id="alter-column-types">
          <option v-for="item in typeOptions" :key="item" :value="item" />
        </datalist>

        <p v-if="isSqliteEdit" class="draft-hint">
          SQLite 不支持修改字段类型、可空性和默认值，这里只能修改字段名。
        </p>
        <p v-else-if="driver === 'sqlite'" class="draft-hint">
          SQLite 添加字段时默认值必须是常量（不能使用 CURRENT_TIMESTAMP
          等表达式），且 NOT NULL 字段必须提供默认值。
        </p>
        <p v-else class="draft-hint">
          默认值按 SQL 原样填写；编辑时清空默认值会移除默认值。
        </p>

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
          {{ executing ? "执行中…" : "执行修改" }}
        </button>
      </footer>
    </div>
  </div>
</template>