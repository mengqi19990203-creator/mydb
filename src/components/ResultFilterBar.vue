<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import {
  RESULT_FILTER_LIMIT,
  RESULT_FILTER_OPERATORS,
  isFilterConditionActive,
  type ResultFilterCondition,
} from "../lib/result-filter";

const props = defineProps<{
  conditions: ResultFilterCondition[];
  columns: Array<{ name: string; hidden: boolean }>;
  caseSensitive: boolean;
  filteredCount: number;
  totalCount: number;
  truncated: boolean;
}>();

const emit = defineEmits<{
  "update:condition": [
    index: number,
    patch: Partial<ResultFilterCondition>,
  ];
  add: [];
  remove: [index: number];
  clear: [];
  close: [];
  "update:caseSensitive": [value: boolean];
}>();

const activeCount = computed(
  () => props.conditions.filter(isFilterConditionActive).length,
);
const filteredOut = computed(() =>
  Math.max(0, props.totalCount - props.filteredCount),
);

function needsValue(operator: string): boolean {
  return (
    RESULT_FILTER_OPERATORS.find((item) => item.value === operator)
      ?.needsValue ?? true
  );
}

function isNumericOperator(operator: string): boolean {
  return Boolean(
    RESULT_FILTER_OPERATORS.find((item) => item.value === operator)?.numeric,
  );
}
</script>

<template>
  <div class="result-filterbar" role="region" aria-label="查询结果筛选">
    <div class="result-filter-head">
      <span class="result-filter-title">
        <Icon name="filter" :size="13" />
        只筛选当前已加载结果 · 基于原始值，不受显示格式影响
        <template v-if="truncated">（结果已截断）</template>
      </span>
      <span class="result-filter-count" aria-live="polite">
        显示 {{ filteredCount }} / 共 {{ totalCount }} 行（筛掉 {{ filteredOut }} 行）
      </span>
      <label class="result-filter-case">
        <input
          type="checkbox"
          :checked="caseSensitive"
          @change="
            emit(
              'update:caseSensitive',
              ($event.target as HTMLInputElement).checked,
            )
          "
        />
        区分大小写
      </label>
      <button
        class="button ghost compact"
        :disabled="!activeCount"
        @click="emit('clear')"
      >
        清除筛选
      </button>
      <button class="button ghost compact" @click="emit('close')">关闭</button>
    </div>

    <div class="result-filter-conditions">
      <div
        v-for="(condition, index) in conditions"
        :key="condition.id"
        class="result-filter-condition"
      >
        <select
          :value="condition.columnIndex"
          aria-label="筛选列"
          @change="
            emit('update:condition', index, {
              columnIndex: Number(($event.target as HTMLSelectElement).value),
            })
          "
        >
          <option
            v-for="(column, columnIndex) in columns"
            :key="columnIndex"
            :value="columnIndex"
          >
            {{ column.name }}{{ column.hidden ? "（已隐藏）" : "" }}
          </option>
        </select>
        <select
          :value="condition.operator"
          aria-label="筛选操作符"
          @change="
            emit('update:condition', index, {
              operator: ($event.target as HTMLSelectElement)
                .value as ResultFilterCondition['operator'],
            })
          "
        >
          <option
            v-for="operator in RESULT_FILTER_OPERATORS"
            :key="operator.value"
            :value="operator.value"
          >
            {{ operator.label }}
          </option>
        </select>
        <input
          v-if="needsValue(condition.operator)"
          :value="condition.value"
          class="filter-value-input"
          placeholder="输入值…"
          aria-label="筛选值"
          spellcheck="false"
          @input="
            emit('update:condition', index, {
              value: ($event.target as HTMLInputElement).value,
            })
          "
        />
        <span v-else class="filter-value-hint">无需输入值</span>
        <button
          class="icon-button"
          title="删除此条件"
          aria-label="删除此条件"
          @click="emit('remove', index)"
        >
          <Icon name="close" :size="12" />
        </button>
        <span
          v-if="
            isNumericOperator(condition.operator) &&
            condition.value.trim() &&
            !Number.isFinite(Number(condition.value))
          "
          class="filter-warning"
        >
          数值比较仅适用于数字值
        </span>
      </div>
    </div>

    <div class="result-filter-actions">
      <button
        class="button ghost compact"
        :disabled="conditions.length >= RESULT_FILTER_LIMIT"
        :title="
          conditions.length >= RESULT_FILTER_LIMIT
            ? '最多 5 个条件'
            : '添加筛选条件'
        "
        @click="emit('add')"
      >
        <Icon name="plus" :size="12" />
        添加条件
      </button>
      <span v-if="conditions.length >= RESULT_FILTER_LIMIT" class="hint">
        最多 5 个条件
      </span>
    </div>
  </div>
</template>