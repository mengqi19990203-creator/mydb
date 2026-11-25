<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { Driver, ScopeSearchHit } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  scopeLabel: string;
}>();

const emit = defineEmits<{
  close: [];
  choose: [hit: ScopeSearchHit];
}>();

const query = ref("");
const results = ref<ScopeSearchHit[]>([]);
const loading = ref(false);
const error = ref("");
const selected = ref(0);
let timer = 0;
let requestToken = 0;

const kindLabels: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
  sequence: "序列",
  procedure: "存储过程",
  function: "函数",
  trigger: "触发器",
  column: "字段",
};

const grouped = computed(() => {
  const objects = results.value.filter((hit) => hit.kind !== "column");
  const columns = results.value.filter((hit) => hit.kind === "column");
  return [
    { label: "对象", items: objects },
    { label: "字段", items: columns },
  ].filter((group) => group.items.length);
});

const flat = computed(() => grouped.value.flatMap((group) => group.items));

function scopeText(hit: ScopeSearchHit): string {
  return [hit.database, hit.schema].filter(Boolean).join(" · ");
}

function scheduleSearch() {
  window.clearTimeout(timer);
  const value = query.value.trim();
  if (value.length < 2) {
    results.value = [];
    loading.value = false;
    error.value = "";
    return;
  }
  timer = window.setTimeout(() => {
    void runSearch(value);
  }, 250);
}

async function runSearch(value: string) {
  const token = ++requestToken;
  loading.value = true;
  error.value = "";
  try {
    const hits = await api.searchScopeObjects(props.connectionId, value);
    if (token !== requestToken) return;
    results.value = hits;
    selected.value = 0;
  } catch (reason) {
    if (token !== requestToken) return;
    results.value = [];
    error.value = String(reason);
  } finally {
    if (token === requestToken) loading.value = false;
  }
}

function choose(hit: ScopeSearchHit) {
  emit("choose", hit);
}

function onKeydown(event: KeyboardEvent) {
  if (!flat.value.length) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    selected.value = Math.min(flat.value.length - 1, selected.value + 1);
    document
      .querySelector<HTMLElement>(
        `[data-search-index="${selected.value}"]`,
      )
      ?.scrollIntoView({ block: "nearest" });
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    selected.value = Math.max(0, selected.value - 1);
    document
      .querySelector<HTMLElement>(
        `[data-search-index="${selected.value}"]`,
      )
      ?.scrollIntoView({ block: "nearest" });
  } else if (event.key === "Enter") {
    event.preventDefault();
    const hit = flat.value[selected.value];
    if (hit) choose(hit);
  }
}

function indexOf(hit: ScopeSearchHit): number {
  return flat.value.indexOf(hit);
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    query.value = "";
    results.value = [];
    loading.value = false;
    error.value = "";
    selected.value = 0;
  },
  { immediate: true },
);

watch(query, scheduleSearch);

onBeforeUnmount(() => {
  window.clearTimeout(timer);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal global-search-modal"
      role="dialog"
      aria-modal="true"
      aria-label="全局对象搜索"
      @keydown="onKeydown"
    >
      <header class="modal-header">
        <div>
          <h2>全局对象搜索</h2>
          <p>{{ scopeLabel }} · 当前连接跨库 / 模式搜索对象名与字段名</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>
      <div class="global-search-body">
        <div class="global-search-input">
          <Icon name="search" :size="15" />
          <input
            v-model="query"
            autofocus
            placeholder="输入至少 2 个字符，搜索表 / 视图 / 例程 / 字段…"
          />
          <span v-if="loading" class="hint">搜索中…</span>
          <span v-else-if="query.trim().length >= 2" class="hint">
            {{ results.length }} 条
          </span>
        </div>
        <p v-if="error" class="message error">{{ error }}</p>
        <div v-if="!query.trim()" class="panel-empty compact">
          输入关键字开始搜索；不会搜索表数据内容。
        </div>
        <div v-else-if="query.trim().length < 2" class="panel-empty compact">
          至少输入 2 个字符。
        </div>
        <div v-else-if="!loading && !results.length" class="panel-empty compact">
          没有匹配的对象或字段。
        </div>
        <div v-else class="global-search-results">
          <div v-for="group in grouped" :key="group.label" class="global-search-group">
            <div class="global-search-group-head">
              {{ group.label }} <span>{{ group.items.length }}</span>
            </div>
            <button
              v-for="hit in group.items"
              :key="`${hit.kind}:${hit.database}:${hit.schema}:${hit.owner}:${hit.name}`"
              class="global-search-row"
              :class="{ selected: flat[indexOf(hit)] === flat[selected] }"
              :data-search-index="indexOf(hit)"
              @mouseenter="selected = indexOf(hit)"
              @click="choose(hit)"
            >
              <Icon
                :name="hit.kind === 'column' ? 'list' : hit.kind === 'view' ? 'view' : 'table'"
                :size="13"
              />
              <span class="global-search-name">{{ hit.name }}</span>
              <span class="global-search-kind">{{ kindLabels[hit.kind] ?? hit.kind }}</span>
              <span class="global-search-scope">{{ scopeText(hit) }}</span>
              <span v-if="hit.owner" class="global-search-owner">← {{ hit.owner }}</span>
            </button>
          </div>
        </div>
        <p class="draft-hint">
          只搜索当前连接可访问的对象与字段；点击表 / 视图会切换库 / 模式并打开数据，字段会打开所属表。
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.global-search-modal {
  width: min(820px, calc(100vw - 48px));
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
}

.global-search-body {
  min-height: 0;
  padding: 0 20px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.global-search-input {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-well);
}

.global-search-input input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 14px;
}

.global-search-results {
  max-height: 460px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.global-search-group-head {
  padding: 6px 10px;
  color: var(--muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.global-search-group-head span {
  margin-left: 4px;
  color: var(--faint);
}

.global-search-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 10px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-size: 12px;
}

.global-search-row:hover,
.global-search-row.selected {
  background: var(--accent-soft);
  color: var(--text);
}

.global-search-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}

.global-search-kind {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--panel-3);
  color: var(--muted);
  font-size: 9px;
}

.global-search-scope {
  margin-left: auto;
  color: var(--muted);
  font-size: 10px;
}

.global-search-owner {
  color: var(--faint);
  font-size: 10px;
}
</style>