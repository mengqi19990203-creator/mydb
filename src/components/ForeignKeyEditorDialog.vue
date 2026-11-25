<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import type { ColumnInfo, Driver, TableInfo } from "../lib/types";

export interface ForeignKeyDraft {
  name: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
  onDelete: string;
  onUpdate: string;
}

const props = defineProps<{
  open: boolean;
  table: string;
  driver: Driver;
  columns: ColumnInfo[];
  tables: TableInfo[];
}>();

const emit = defineEmits<{
  close: [];
  apply: [draft: ForeignKeyDraft];
}>();

const ACTIONS = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL"] as const;

const name = ref("");
const localColumns = ref("");
const referencedTable = ref("");
const referencedColumns = ref("");
const onDelete = ref("NO ACTION");
const onUpdate = ref("NO ACTION");
const error = ref("");

const generatedName = computed(() => {
  const columns = splitColumns(localColumns.value)
    .join("_")
    .replace(/[^A-Za-z0-9_]/g, "_");
  return `fk_${props.table}_${columns || "ref"}`;
});

function splitColumns(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    name.value = "";
    localColumns.value = "";
    referencedTable.value = props.tables[0]?.name ?? "";
    referencedColumns.value = "";
    onDelete.value = "NO ACTION";
    onUpdate.value = "NO ACTION";
    error.value = "";
  },
  { immediate: true },
);

function submit() {
  error.value = "";
  const local = splitColumns(localColumns.value);
  const referenced = splitColumns(referencedColumns.value);
  if (!local.length || !referenced.length) {
    error.value = "本地列和引用列都不能为空";
    return;
  }
  if (local.length !== referenced.length) {
    error.value = "本地列与引用列数量必须一致";
    return;
  }
  const known = new Set(props.columns.map((column) => column.name));
  const missing = local.find((column) => !known.has(column));
  if (missing) {
    error.value = `本地列为空或不存在: ${missing}`;
    return;
  }
  if (!referencedTable.value) {
    error.value = "请选择引用表";
    return;
  }
  emit("apply", {
    name: name.value.trim() || generatedName.value,
    columns: local,
    referencedTable: referencedTable.value,
    referencedColumns: referenced,
    onDelete: onDelete.value,
    onUpdate: onUpdate.value,
  });
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal connection-modal"
      role="dialog"
      aria-modal="true"
      aria-label="添加外键"
    >
      <header class="modal-header">
        <div>
          <h2>添加外键</h2>
          <p>{{ table }} · {{ driver === "mysql" ? "MySQL" : "PostgreSQL" }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>
      <div class="connection-form">
        <div class="form-grid">
          <label class="span-2">
            约束名（可留空）
            <input v-model="name" :placeholder="generatedName" />
          </label>
          <label class="span-2">
            本地列（按顺序，逗号分隔）
            <input v-model="localColumns" placeholder="例如：org_id" />
          </label>
          <label class="span-2">
            引用表
            <select v-model="referencedTable">
              <option
                v-for="item in tables"
                :key="item.name"
                :value="item.name"
              >
                {{ item.name }}
              </option>
            </select>
          </label>
          <label class="span-2">
            引用列（按顺序，逗号分隔）
            <input v-model="referencedColumns" placeholder="例如：id" />
          </label>
          <label>
            ON DELETE
            <select v-model="onDelete">
              <option v-for="action in ACTIONS" :key="action" :value="action">
                {{ action }}
              </option>
            </select>
          </label>
          <label>
            ON UPDATE
            <select v-model="onUpdate">
              <option v-for="action in ACTIONS" :key="action" :value="action">
                {{ action }}
              </option>
            </select>
          </label>
        </div>
        <p class="draft-hint">
          MySQL / PostgreSQL 会通过 ALTER TABLE 添加；SQLite 不支持在结构页修改外键。
        </p>
        <p v-if="error" class="message error">{{ error }}</p>
      </div>
      <footer class="modal-footer">
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button class="button primary" @click="submit">添加外键</button>
      </footer>
    </div>
  </div>
</template>