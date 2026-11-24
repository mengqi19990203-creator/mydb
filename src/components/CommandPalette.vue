<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import Icon from "./Icon.vue";
import {
  filterPaletteItems,
  type PaletteItem,
  type PaletteRecentEntry,
} from "../lib/command-palette";

const props = defineProps<{
  open: boolean;
  connections: PaletteItem[];
  objects: PaletteItem[];
  tabs: PaletteItem[];
  commands: PaletteItem[];
  recent: PaletteRecentEntry[];
  objectsLoading: boolean;
  objectStatus: string;
  hasConnections: boolean;
  commandKey: string;
}>();

const emit = defineEmits<{
  close: [];
  execute: [item: PaletteItem, alt: boolean];
  "clear-recent": [];
}>();

const query = ref("");
const composing = ref(false);
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const isMac = navigator.platform.toUpperCase().includes("MAC");

interface PaletteGroupView {
  key: string;
  label: string;
  items: PaletteItem[];
  total: number;
  note?: string;
  canClear?: boolean;
}

function recentItems(): PaletteItem[] {
  const pool = [...props.connections, ...props.commands];
  return props.recent
    .map((entry) =>
      pool.find(
        (item) =>
          item.kind === entry.kind && item.id === entry.id,
      ),
    )
    .filter((item): item is PaletteItem => Boolean(item));
}

function clip(items: PaletteItem[], total: number): PaletteItem[] {
  return items.slice(0, Math.min(6, total));
}

const groups = computed<PaletteGroupView[]>(() => {
  const term = query.value.trim();
  if (!term) {
    const result: PaletteGroupView[] = [];
    const recent = recentItems();
    if (recent.length) {
      result.push({
        key: "recent",
        label: "最近",
        items: clip(recent, recent.length),
        total: recent.length,
        canClear: true,
      });
    }
    if (props.connections.length) {
      result.push({
        key: "connections",
        label: "连接",
        items: clip(props.connections, props.connections.length),
        total: props.connections.length,
      });
    }
    if (props.commands.length) {
      result.push({
        key: "commands",
        label: "命令",
        items: clip(props.commands, props.commands.length),
        total: props.commands.length,
      });
    }
    return result;
  }

  const result: PaletteGroupView[] = [];
  const connections = filterPaletteItems(props.connections, term);
  const objects = filterPaletteItems(props.objects, term);
  const tabs = filterPaletteItems(props.tabs, term);
  const commands = filterPaletteItems(props.commands, term);

  if (connections.length) {
    result.push({
      key: "connections",
      label: "连接",
      items: clip(connections, connections.length),
      total: connections.length,
    });
  }

  const objectNote = props.objectsLoading
    ? "对象加载中…"
    : props.objectStatus && !objects.length
      ? props.objectStatus
      : "";
  if (objects.length || objectNote) {
    result.push({
      key: "objects",
      label: "对象",
      items: clip(objects, objects.length),
      total: objects.length,
      note: objectNote || undefined,
    });
  }

  if (tabs.length) {
    result.push({
      key: "tabs",
      label: "标签",
      items: clip(tabs, tabs.length),
      total: tabs.length,
    });
  }

  if (commands.length) {
    result.push({
      key: "commands",
      label: "命令",
      items: clip(commands, commands.length),
      total: commands.length,
    });
  }

  return result;
});

const flatItems = computed(() =>
  groups.value.flatMap((group) => group.items),
);

const activeItem = computed<PaletteItem | null>(
  () => flatItems.value[activeIndex.value] ?? null,
);

function scrollActiveIntoView() {
  void nextTick(() => {
    panelRef.value
      ?.querySelector<HTMLElement>(
        `[data-palette-index="${activeIndex.value}"]`,
      )
      ?.scrollIntoView({ block: "nearest" });
  });
}

function moveActive(delta: number) {
  const total = flatItems.value.length;
  if (!total) return;
  activeIndex.value =
    (activeIndex.value + delta + total) % total;
  scrollActiveIntoView();
}

function setActive(index: number) {
  if (!flatItems.value.length) return;
  activeIndex.value = Math.max(
    0,
    Math.min(index, flatItems.value.length - 1),
  );
  scrollActiveIntoView();
}

function rowIndex(groupIndex: number, itemIndex: number): number {
  let index = itemIndex;
  for (let i = 0; i < groupIndex; i += 1) {
    index += groups.value[i]?.items.length ?? 0;
  }
  return index;
}

function execute(item: PaletteItem | null, alt: boolean) {
  if (!item || item.disabledReason) return;
  emit("execute", item, alt);
}

function onInputKeydown(event: KeyboardEvent) {
  if (composing.value || event.isComposing || event.keyCode === 229) {
    return;
  }
  const mod = isMac ? event.metaKey : event.ctrlKey;
  if (
    mod &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === "k"
  ) {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
    return;
  }
  if (event.ctrlKey && event.key === "Tab") {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  switch (event.key) {
    case "Escape":
      event.preventDefault();
      event.stopPropagation();
      emit("close");
      break;
    case "ArrowDown":
      event.preventDefault();
      moveActive(1);
      break;
    case "ArrowUp":
      event.preventDefault();
      moveActive(-1);
      break;
    case "Home":
      event.preventDefault();
      setActive(0);
      break;
    case "End":
      event.preventDefault();
      setActive(flatItems.value.length - 1);
      break;
    case "PageDown":
      event.preventDefault();
      moveActive(5);
      break;
    case "PageUp":
      event.preventDefault();
      moveActive(-5);
      break;
    case "Enter":
      event.preventDefault();
      execute(activeItem.value, mod);
      break;
    case "Tab":
      event.preventDefault();
      moveActive(event.shiftKey ? -1 : 1);
      break;
    default:
      break;
  }
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    query.value = "";
    activeIndex.value = 0;
    composing.value = false;
    await nextTick();
    inputRef.value?.focus();
    inputRef.value?.select();
  },
);

watch(query, () => {
  activeIndex.value = 0;
  scrollActiveIntoView();
});

watch(flatItems, () => {
  if (activeIndex.value >= flatItems.value.length) {
    activeIndex.value = Math.max(0, flatItems.value.length - 1);
  }
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="command-palette-backdrop"
      @mousedown.self="emit('close')"
    >
      <div
        ref="panelRef"
        class="command-palette"
        role="dialog"
        aria-modal="true"
        aria-label="命令面板"
      >
        <div class="palette-input-row">
          <Icon name="search" :size="16" />
          <input
            ref="inputRef"
            v-model="query"
            role="combobox"
            aria-autocomplete="list"
            aria-controls="command-palette-list"
            :aria-activedescendant="
              activeItem ? `palette-option-${activeIndex}` : undefined
            "
            placeholder="搜索连接、表、标签或命令…"
            @keydown="onInputKeydown"
            @compositionstart="composing = true"
            @compositionend="composing = false"
          />
          <kbd class="palette-esc">esc</kbd>
        </div>

        <div
          id="command-palette-list"
          class="palette-results"
          role="listbox"
          aria-label="命令面板结果"
        >
          <div
            v-if="!groups.length"
            class="palette-empty"
          >
            <Icon name="search" :size="18" />
            <strong>没有匹配项</strong>
            <span>
              {{
                hasConnections
                  ? "试试连接名、表名或「新建查询」"
                  : "还没有连接，先新建一个连接"
              }}
            </span>
          </div>

          <section
            v-for="(group, groupIndex) in groups"
            :key="group.key"
            class="palette-group"
            :aria-labelledby="`palette-group-${group.key}`"
          >
            <header class="palette-group-head">
              <span :id="`palette-group-${group.key}`">
                {{ group.label }}
              </span>
              <span class="palette-group-count">{{ group.total }}</span>
              <button
                v-if="group.canClear"
                class="palette-clear"
                @mousedown.prevent
                @click.stop="emit('clear-recent')"
              >
                清除最近
              </button>
            </header>

            <div
              v-for="(item, itemIndex) in group.items"
              :id="`palette-option-${rowIndex(groupIndex, itemIndex)}`"
              :key="`${group.key}:${item.kind}:${item.id}`"
              class="palette-item"
              :class="{
                active:
                  rowIndex(groupIndex, itemIndex) === activeIndex,
                disabled: Boolean(item.disabledReason),
              }"
              :data-palette-index="rowIndex(groupIndex, itemIndex)"
              role="option"
              :aria-selected="
                rowIndex(groupIndex, itemIndex) === activeIndex
              "
              :aria-disabled="item.disabledReason ? 'true' : undefined"
              :title="item.disabledReason || undefined"
              @mouseenter="setActive(rowIndex(groupIndex, itemIndex))"
              @mousedown.prevent
              @click="execute(item, false)"
            >
              <Icon :name="item.icon" :size="16" />
              <span class="palette-item-text">
                <span class="palette-item-title">
                  {{ item.label }}
                </span>
                <span class="palette-item-detail">
                  {{ item.disabledReason || item.detail }}
                </span>
              </span>
              <span v-if="item.badge" class="palette-badge">
                {{ item.badge }}
              </span>
              <kbd v-if="item.shortcut" class="palette-shortcut">
                {{ item.shortcut }}
              </kbd>
            </div>

            <div v-if="group.note" class="palette-note">
              {{ group.note }}
            </div>
            <div
              v-if="group.total > group.items.length"
              class="palette-note"
            >
              还有 {{ group.total - group.items.length }} 项，继续输入以缩小范围
            </div>
          </section>
        </div>

        <footer class="palette-footer">
          <span>↑↓ 选择 · Enter 打开 · {{ commandKey }}↩ 新标签 · Esc 关闭</span>
          <span>{{ flatItems.length }} 个结果</span>
        </footer>
      </div>
    </div>
  </Teleport>
</template>