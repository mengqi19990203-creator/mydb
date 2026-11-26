<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import Icon from "./Icon.vue";

const props = defineProps<{
  query: string;
  caseSensitive: boolean;
  matchCount: number;
  rowCount: number;
  /** Zero-based index of the current match. */
  current: number;
  scopeHint: string;
  placeholder?: string;
}>();

const emit = defineEmits<{
  "update:query": [value: string];
  "update:caseSensitive": [value: boolean];
  next: [];
  prev: [];
  close: [];
}>();

const inputRef = ref<HTMLInputElement | null>(null);

const countLabel = computed(() => {
  if (!props.query.trim()) return "输入查找内容";
  if (!props.rowCount) return "当前没有可查找的数据";
  if (!props.matchCount) return "无匹配";
  return `第 ${props.current + 1} / ${props.matchCount} 个匹配（${props.rowCount} 行）`;
});

function focusInput() {
  const input = inputRef.value;
  if (!input) return;
  input.focus();
  input.select();
}

function onInput(event: Event) {
  emit("update:query", (event.target as HTMLInputElement).value);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
    return;
  }
  if (event.key === "Enter") {
    event.preventDefault();
    event.stopPropagation();
    if (event.shiftKey) emit("prev");
    else emit("next");
  }
}

function clear() {
  emit("update:query", "");
  focusInput();
}

onMounted(() => {
  void nextTick(focusInput);
});

defineExpose({ focusInput });
</script>

<template>
  <div class="grid-findbar" role="search" aria-label="在已加载数据中查找">
    <span class="grid-find-icon">
      <Icon name="search" :size="14" />
    </span>
    <input
      ref="inputRef"
      class="grid-find-input"
      :value="query"
      :placeholder="placeholder ?? '查找已加载数据…'"
      aria-label="查找内容"
      spellcheck="false"
      @input="onInput"
      @keydown="onKeydown"
    />
    <button
      class="grid-find-toggle"
      :class="{ active: caseSensitive }"
      title="区分大小写"
      :aria-pressed="caseSensitive"
      @click="emit('update:caseSensitive', !caseSensitive)"
    >
      Aa
    </button>
    <span class="grid-find-count" aria-live="polite">{{ countLabel }}</span>
    <button
      class="icon-button grid-find-nav"
      title="上一个匹配（Shift+Enter）"
      aria-label="上一个匹配"
      :disabled="!matchCount"
      @click="emit('prev')"
    >
      <Icon name="chevron-up" :size="13" />
    </button>
    <button
      class="icon-button grid-find-nav"
      title="下一个匹配（Enter）"
      aria-label="下一个匹配"
      :disabled="!matchCount"
      @click="emit('next')"
    >
      <Icon name="chevron-down" :size="13" />
    </button>
    <button
      class="icon-button grid-find-nav"
      title="清除查找"
      aria-label="清除查找"
      :disabled="!query"
      @click="clear"
    >
      <Icon name="close" :size="13" />
    </button>
    <button
      class="icon-button grid-find-nav"
      title="关闭查找（Esc）"
      aria-label="关闭查找"
      @click="emit('close')"
    >
      <Icon name="chevron-right" :size="13" />
    </button>
    <span class="grid-find-scope">{{ scopeHint }}</span>
  </div>
</template>