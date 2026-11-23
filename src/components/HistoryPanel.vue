<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import type { HistoryEntry } from "../lib/types";

const props = defineProps<{
  open: boolean;
  entries: HistoryEntry[];
  loading: boolean;
}>();

const emit = defineEmits<{
  close: [];
  select: [entry: HistoryEntry];
  clear: [];
}>();

const search = ref("");

const filteredEntries = computed(() => {
  const query = search.value.trim().toLowerCase();
  if (!query) return props.entries;
  return props.entries.filter((entry) =>
    [entry.sql, entry.connectionName, entry.error ?? ""]
      .join(" ")
      .toLowerCase()
      .includes(query),
  );
});

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

function preview(sql: string): string {
  const compact = sql.replace(/\s+/g, " ").trim();
  return compact.length > 160 ? `${compact.slice(0, 160)}…` : compact;
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") {
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      document.addEventListener("keydown", onKeydown);
    } else {
      document.removeEventListener("keydown", onKeydown);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <aside v-if="open" class="history-panel">
    <header class="panel-header">
      <span>
        SQL 历史
        <span class="group-count">
          {{ filteredEntries.length }} / {{ entries.length }}
        </span>
      </span>
      <div class="panel-actions">
        <button
          class="icon-button"
          title="清空历史"
          :disabled="!entries.length"
          @click="emit('clear')"
        >
          <Icon name="trash" />
        </button>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </div>
    </header>

    <div class="sidebar-search history-search">
      <Icon name="search" :size="14" />
      <input
        v-model="search"
        placeholder="搜索 SQL / 连接…"
        @keydown.esc="search = ''"
      />
      <button
        v-if="search"
        class="icon-button search-clear"
        title="清空搜索"
        aria-label="清空搜索"
        @click="search = ''"
      >
        <Icon name="close" :size="12" />
      </button>
    </div>

    <div class="history-panel-list">
      <div v-if="loading" class="panel-empty">
        <span class="spinner"></span>
        加载中…
      </div>
      <div v-else-if="!filteredEntries.length" class="panel-empty">
        {{ entries.length ? "没有匹配的历史" : "还没有执行记录" }}
      </div>

      <button
        v-for="entry in filteredEntries"
        :key="entry.id"
        class="history-panel-item"
        :class="{ failed: !entry.success }"
        @click="emit('select', entry)"
      >
        <div class="history-item-head">
          <span class="history-connection" :title="entry.connectionName">
            {{ entry.connectionName || "未知连接" }}
          </span>
          <span class="history-time">{{ formatTime(entry.executedAt) }}</span>
        </div>
        <code class="history-sql" :title="entry.sql">{{ preview(entry.sql) }}</code>
        <div class="history-panel-meta">
          <span class="history-status" :class="{ failed: !entry.success }">
            {{ entry.success ? "成功" : "失败" }}
          </span>
          <span v-if="entry.success">{{ entry.elapsedMs }} ms</span>
          <span v-else class="history-error-inline">{{ entry.error }}</span>
        </div>
      </button>
    </div>
  </aside>
</template>
