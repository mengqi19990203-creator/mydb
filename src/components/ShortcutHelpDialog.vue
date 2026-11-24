<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import {
  findDuplicateShortcuts,
  formatShortcut,
  SHORTCUTS,
  SHORTCUT_SCOPE_LABELS,
  SHORTCUT_SCOPE_ORDER,
  type ShortcutEntry,
} from "../lib/shortcuts";

const props = defineProps<{
  open: boolean;
}>();
const emit = defineEmits<{
  close: [];
}>();

const query = ref("");
const searchRef = ref<HTMLInputElement | null>(null);
const modalRef = ref<HTMLElement | null>(null);
let previousActive: HTMLElement | null = null;
const isMac = navigator.platform.toUpperCase().includes("MAC");

const groups = computed(() => {
  const term = query.value.trim().toLowerCase();
  return SHORTCUT_SCOPE_ORDER.map((scope) => {
    const entries = SHORTCUTS.filter((entry) => {
      if (entry.scope !== scope) return false;
      if (!term) return true;
      return [entry.action, entry.keys, entry.context ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(term);
    });
    return {
      scope,
      label: SHORTCUT_SCOPE_LABELS[scope],
      entries,
    };
  }).filter((group) => group.entries.length > 0);
});

const duplicateWarnings = computed(() => findDuplicateShortcuts());

function keysFor(entry: ShortcutEntry): string {
  return formatShortcut(entry.keys, isMac);
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
    return;
  }
  if (event.key !== "Tab") return;
  const modal = modalRef.value;
  if (!modal) return;
  const items = Array.from(
    modal.querySelectorAll<HTMLElement>(
      'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  );
  if (!items.length) return;
  const first = items[0];
  const last = items[items.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      previousActive =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      query.value = "";
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      searchRef.value?.focus();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
      if (previousActive?.isConnected) {
        previousActive.focus({ preventScroll: true });
      }
      previousActive = null;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      ref="modalRef"
      class="modal shortcut-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="shortcut-help-title"
    >
      <header class="shortcut-head">
        <div>
          <h2 id="shortcut-help-title">键盘快捷键</h2>
          <p class="shortcut-subtitle">
            只列出当前版本实际可用的快捷键；{{ isMac ? "⌘" : "Ctrl" }}
            表示主修饰键。
          </p>
        </div>
        <button
          class="icon-button"
          title="关闭"
          aria-label="关闭键盘快捷键"
          @click="emit('close')"
        >
          <Icon name="close" :size="14" />
        </button>
      </header>

      <label class="shortcut-search">
        <Icon name="search" :size="14" />
        <input
          ref="searchRef"
          v-model="query"
          placeholder="搜索动作、快捷键或场景…"
        />
        <button
          v-if="query"
          class="icon-button search-clear"
          title="清空搜索"
          aria-label="清空搜索"
          @click="query = ''"
        >
          <Icon name="close" :size="12" />
        </button>
      </label>

      <div class="shortcut-body">
        <div v-if="duplicateWarnings.length" class="shortcut-warning">
          检测到 {{ duplicateWarnings.length }} 组重复键位（仅开发期提示，不影响使用）。
        </div>
        <div v-if="!groups.length" class="shortcut-empty">
          没有匹配的快捷键。
        </div>
        <section
          v-for="group in groups"
          :key="group.scope"
          class="shortcut-group"
          :aria-labelledby="`shortcut-${group.scope}`"
        >
          <h3 :id="`shortcut-${group.scope}`">{{ group.label }}</h3>
          <div class="shortcut-list">
            <div
              v-for="entry in group.entries"
              :key="`${entry.scope}:${entry.action}:${entry.keys}`"
              class="shortcut-row"
            >
              <span class="shortcut-action">
                {{ entry.action }}
                <span v-if="entry.context" class="shortcut-context">
                  {{ entry.context }}
                </span>
              </span>
              <kbd class="shortcut-key">{{ keysFor(entry) }}</kbd>
            </div>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>