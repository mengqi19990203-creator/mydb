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
const comment = ref("");
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
  if (isSqliteEdit.value) {
    if (!name.value.trim()) {
      return { statements: [] as string[], error: "字段名不能为空", rebuild: true };
    }
    if (!type.value.trim()) {
      return { statements: [] as string[], error: "字段类型不能为空", rebuild: true };
    }
    const definition = `"${name.value.trim()}" ${type.value.trim()}${
      nullable.value ? "" : " NOT NULL"
    }${
      defaultValue.value.trim() ? ` DEFAULT ${defaultValue.value.trim()}` : ""
    }`;
    return {
      statements: [`-- SQLite 将重建表并应用字段定义：${definition}`],
      error: "",
      rebuild: true,
    };
  }
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
        comment: comment.value,
      },
    );
    return { statements, error: "", rebuild: false };
  } catch (reason) {
    return {
      statements: [] as string[],
      error: reason instanceof Error ? reason.message : String(reason),
      rebuild: false,
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
    comment.value = column?.comment ?? "";
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
    if (preview.value.rebuild && props.column) {
      await api.alterTableColumn(
        props.connectionId,
        props.table,
        props.column.name,
        name.value.trim(),
        type.value.trim(),
        nullable.value,
        defaultValue.value.trim() || null,
      );
      emit("done", "字段已更新（SQLite 已重建表）");
      return;
    }

    for (const statement of preview.value.statements) {
      await api.executeQuery(props.connectionId, statement, 1);
      completed += 1;
    }
    emit("done", props.column ? "字段已更新" : "字段已添加");
  } catch (reason) {
    const prefix =
      preview.value.rebuild
        ? "重建表失败，已回滚；原表和数据保持不变。\n"
        : completed > 0
          ? "部分语句已执行，请关闭后刷新确认。\n"
          : "";
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
              placeholder="例如：VARCHAR(255)"
            />
          </label>
          <label class="span-2 checkbox-row">
            <input
              v-model="nullable"
              type="checkbox"
            />
            可空
          </label>
          <label class="span-2">
            注释
            <input
              v-model="comment"
              :disabled="driver === 'sqlite'"
              placeholder="可选；MySQL 使用 COMMENT，PostgreSQL 使用 COMMENT ON"
            />
          </label>
          <label class="span-2">
            默认值
            <input
              v-model="defaultValue"
              placeholder="SQL 表达式，如 0、'abc'、CURRENT_TIMESTAMP"
            />
          </label>
        </div>

        <datalist id="alter-column-types">
          <option v-for="item in typeOptions" :key="item" :value="item" />
        </datalist>

        <p v-if="isSqliteEdit" class="draft-hint">
          SQLite 会重建表来修改字段类型、可空性或默认值；数据、显式索引和触发器会保留，
          已有数据违反新约束时会整体回滚。
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