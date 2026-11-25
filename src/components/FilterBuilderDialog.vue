<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import Icon from "./Icon.vue";
import { quoteIdentifier, sqlLiteral } from "../lib/sql-literal";
import type { ColumnInfo, Driver } from "../lib/types";

interface Condition {
  column: string;
  operator: string;
  value: string;
}

const props = defineProps<{
  open: boolean;
  columns: ColumnInfo[];
  driver: Driver;
}>();

const emit = defineEmits<{
  close: [];
  apply: [filter: string];
}>();

const OPERATORS = [
  "=",
  "<>",
  ">",
  ">=",
  "<",
  "<=",
  "LIKE",
  "NOT LIKE",
  "IS NULL",
  "IS NOT NULL",
] as const;

const conditions = reactive<Condition[]>([]);

const noValueOperator = (operator: string) =>
  operator === "IS NULL" || operator === "IS NOT NULL";

const preview = computed(() => {
  const parts = conditions
    .filter((condition) => condition.column && condition.operator)
    .map((condition) => {
      const identifier = quoteIdentifier(condition.column, props.driver);
      if (noValueOperator(condition.operator)) {
        return `${identifier} ${condition.operator}`;
      }
      return `${identifier} ${condition.operator} ${sqlLiteral(
        condition.value,
        props.driver,
      )}`;
    })
    .filter(Boolean);
  return parts.length ? parts.join(" AND ") : "";
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    conditions.splice(0, conditions.length);
    conditions.push({
      column: props.columns[0]?.name ?? "",
      operator: "=",
      value: "",
    });
  },
  { immediate: true },
);

function addCondition() {
  conditions.push({
    column: props.columns[0]?.name ?? "",
    operator: "=",
    value: "",
  });
}

function removeCondition(index: number) {
  conditions.splice(index, 1);
}

function apply() {
  if (!preview.value) return;
  emit("apply", preview.value);
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal create-table-modal"
      role="dialog"
      aria-modal="true"
      aria-label="筛选构建器"
    >
      <header class="modal-header">
        <div>
          <h2>筛选构建器</h2>
          <p>多个条件按 AND 组合；OR、括号和子查询请使用高级 WHERE</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>
      <div class="connection-form">
        <div class="filter-condition-list">
          <div
            v-for="(condition, index) in conditions"
            :key="index"
            class="filter-condition"
          >
            <select v-model="condition.column" aria-label="字段">
              <option
                v-for="column in columns"
                :key="column.name"
                :value="column.name"
              >
                {{ column.name }}
              </option>
            </select>
            <select v-model="condition.operator" aria-label="操作符">
              <option
                v-for="operator in OPERATORS"
                :key="operator"
                :value="operator"
              >
                {{ operator }}
              </option>
            </select>
            <input
              v-model="condition.value"
              :disabled="noValueOperator(condition.operator)"
              placeholder="值"
              aria-label="值"
            />
            <button
              class="icon-button"
              title="移除条件"
              aria-label="移除条件"
              :disabled="conditions.length === 1"
              @click="removeCondition(index)"
            >
              <Icon name="trash" :size="14" />
            </button>
          </div>
        </div>
        <button class="button ghost compact" @click="addCondition">
          <Icon name="plus" :size="14" />
          添加条件
        </button>
        <div class="filter-preview">
          <strong>生成的 WHERE</strong>
          <code>{{ preview || "（尚未填写）" }}</code>
        </div>
        <p class="draft-hint">
          值与标识符按当前数据库方言转义；LIKE 值请直接填写模式，例如 A%。
        </p>
      </div>
      <footer class="modal-footer">
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button
          class="button primary"
          :disabled="!preview"
          @click="apply"
        >
          应用筛选
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.filter-condition-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 10px;
}

.filter-condition {
  display: grid;
  grid-template-columns: minmax(100px, 1fr) 110px minmax(130px, 1.4fr) 32px;
  gap: 8px;
  align-items: center;
}

.filter-preview {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.filter-preview code {
  display: block;
  padding: 8px;
  overflow-x: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 11px;
  white-space: pre-wrap;
}
</style>