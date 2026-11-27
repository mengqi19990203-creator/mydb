<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import {
  columnMatches,
  type AutoHideMode,
  type ColumnVisibilityColumn,
} from "../lib/column-visibility";

const props = defineProps<{
  open: boolean;
  columns: ColumnVisibilityColumn[];
  manualHidden: string[];
  autoHidden: string[];
  autoMode: AutoHideMode;
  hasData: boolean;
  rowCount: number;
  rowLabel: string;
  autoLimitNotice?: string | null;
}>();

const emit = defineEmits<{
  close: [];
  toggle: [key: string, visible: boolean];
  showAll: [];
  setMode: [mode: AutoHideMode];
}>();

const search = ref("");
const searchInput = ref<HTMLInputElement | null>(null);
const manualSet = computed(() => new Set(props.manualHidden));
const autoSet = computed(() => new Set(props.autoHidden));
const filtered = computed(() =>
  props.columns.filter((column) => columnMatches(column, search.value)),
);
const hiddenTotal = computed(
  () =>
    new Set([...props.manualHidden, ...props.autoHidden]).size,
);
const autoCount = computed(() => props.autoHidden.length);

function isVisible(column: ColumnVisibilityColumn): boolean {
  if (manualSet.value.has(column.key)) return false;
  if (autoSet.value.has(column.key)) return false;
  return true;
}

function toggleColumn(column: ColumnVisibilityColumn) {
  const visible = isVisible(column);
  if (visible && props.columns.filter((item) => isVisible(item)).length <= 1) {
    return;
  }
  emit("toggle", column.key, !visible);
}

function setMode(mode: AutoHideMode) {
  if (!props.hasData && mode !== "off") return;
  emit("setMode", mode);
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("close");
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      search.value = "";
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      searchInput.value?.focus();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div v-if="open" class="column-visibility-backdrop" @click.self="emit('close')">
    <aside class="column-visibility-panel" role="dialog" aria-modal="true" aria-label="列显示与自动隐藏">
      <header class="column-visibility-head">
        <div>
          <strong>列</strong>
          <span class="hint">
            手动隐藏 {{ manualHidden.length }} · 自动隐藏 {{ autoCount }} · 共隐藏 {{ hiddenTotal }}
          </span>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="column-visibility-search">
        <Icon name="search" :size="12" />
        <input
          ref="searchInput"
          v-model="search"
          placeholder="搜索列名或注释"
          aria-label="搜索列"
        />
      </div>
      <p class="hint">
        匹配 {{ filtered.length }} / 共 {{ columns.length }} 列 · {{ rowLabel }}
      </p>

      <div class="column-visibility-list">
        <button
          v-for="column in filtered"
          :key="column.key"
          class="column-visibility-row"
          :class="{ hidden: !isVisible(column) }"
          @click="toggleColumn(column)"
        >
          <input
            type="checkbox"
            :checked="isVisible(column)"
            :aria-label="`显示列 ${column.name}`"
            @click.stop="toggleColumn(column)"
          />
          <span class="column-visibility-name">{{ column.name }}</span>
          <span v-if="manualSet.has(column.key)" class="column-visibility-badge">手动</span>
          <span v-else-if="autoSet.has(column.key)" class="column-visibility-badge auto">空列</span>
          <span v-if="column.comment" class="column-visibility-comment">{{ column.comment }}</span>
        </button>
        <p v-if="!filtered.length" class="hint column-visibility-empty">没有匹配的列</p>
      </div>

      <div class="column-visibility-auto">
        <span>自动隐藏空列</span>
        <label>
          <input
            type="radio"
            name="column-auto-mode"
            :checked="autoMode === 'off'"
            @change="setMode('off')"
          />
          关闭
        </label>
        <label>
          <input
            type="radio"
            name="column-auto-mode"
            :checked="autoMode === 'null'"
            :disabled="!hasData"
            @change="setMode('null')"
          />
          全 NULL
        </label>
        <label>
          <input
            type="radio"
            name="column-auto-mode"
            :checked="autoMode === 'null-empty'"
            :disabled="!hasData"
            @change="setMode('null-empty')"
          />
          全 NULL / 空串
        </label>
        <p v-if="!hasData" class="hint">当前没有数据，无法判断空列。</p>
        <p v-if="autoLimitNotice" class="hint">{{ autoLimitNotice }}</p>
      </div>

      <footer class="column-visibility-foot">
        <button class="button ghost compact" @click="emit('showAll')">全部显示</button>
        <span class="spacer"></span>
        <button class="button primary compact" @click="emit('close')">完成</button>
      </footer>
    </aside>
  </div>
</template>
