<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import PromptDialog from "./PromptDialog.vue";
import {
  formatPinClock,
  MAX_PINNED_NAME_LENGTH,
  type PinnedQueryResult,
} from "../lib/pinned-results";

const props = defineProps<{
  pins: PinnedQueryResult[];
  activeId: string | null;
  liveCompareReason: string;
}>();

const emit = defineEmits<{
  select: [id: string | null];
  unpin: [id: string];
  rename: [payload: { id: string; name: string }];
  "compare-current": [id: string];
  "compare-other": [id: string];
  "compare-latest": [];
  export: [id: string];
  "copy-sql": [id: string];
}>();

const menu = ref<{ x: number; y: number; pin: PinnedQueryResult } | null>(
  null,
);
const renameTarget = ref<PinnedQueryResult | null>(null);
const compareOtherDisabled = computed(() => props.pins.length < 2);

function selectPin(pin: PinnedQueryResult) {
  emit("select", props.activeId === pin.id ? null : pin.id);
}

function openMenu(event: MouseEvent, pin: PinnedQueryResult) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 240)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 260)),
    pin,
  };
}

function closeMenu() {
  menu.value = null;
}

function withMenuPin(action: (pin: PinnedQueryResult) => void) {
  const pin = menu.value?.pin;
  closeMenu();
  if (pin) action(pin);
}

function unpinFromMenu() {
  withMenuPin((pin) => emit("unpin", pin.id));
}

function compareCurrentFromMenu() {
  withMenuPin((pin) => emit("compare-current", pin.id));
}

function compareOtherFromMenu() {
  withMenuPin((pin) => emit("compare-other", pin.id));
}

function exportFromMenu() {
  withMenuPin((pin) => emit("export", pin.id));
}

function copySqlFromMenu() {
  withMenuPin((pin) => emit("copy-sql", pin.id));
}

function startRename(pin: PinnedQueryResult) {
  closeMenu();
  renameTarget.value = pin;
}

function confirmRename(value: string) {
  const target = renameTarget.value;
  renameTarget.value = null;
  if (!target) return;
  const name = value.trim().slice(0, MAX_PINNED_NAME_LENGTH);
  if (!name) return;
  emit("rename", { id: target.id, name });
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && menu.value) {
    event.preventDefault();
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener("click", closeMenu);
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", closeMenu);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div v-if="pins.length" class="pinned-bar" role="region" aria-label="固定结果">
    <span class="pinned-bar-label">
      <Icon name="pin" :size="12" />
      固定结果
    </span>
    <div class="pinned-bar-items">
      <button
        v-for="pin in pins"
        :key="pin.id"
        class="pinned-chip"
        :class="{ active: pin.id === activeId }"
        :title="`${pin.sql.slice(0, 300)}${pin.sql.length > 300 ? '…' : ''}`"
        :aria-pressed="pin.id === activeId"
        @click="selectPin(pin)"
        @contextmenu="openMenu($event, pin)"
      >
        <Icon name="pin" :size="11" />
        <span class="pinned-chip-name">{{ pin.name }}</span>
        <span class="pinned-chip-meta">
          {{ pin.result.rows.length }} 行 · {{ formatPinClock(pin.pinnedAt) }}
          <template v-if="pin.result.truncated"> · 已截断</template>
        </span>
        <span
          class="pinned-chip-remove"
          role="button"
          tabindex="0"
          title="取消固定"
          aria-label="取消固定"
          @click.stop="emit('unpin', pin.id)"
          @keydown.enter.stop.prevent="emit('unpin', pin.id)"
          @keydown.space.stop.prevent="emit('unpin', pin.id)"
        >
          <Icon name="close" :size="10" />
        </span>
      </button>
    </div>
    <button
      v-if="pins.length >= 2"
      class="button ghost compact pinned-compare-all"
      title="用最近固定的结果与当前结果对比"
      @click="emit('compare-latest')"
    >
      <Icon name="target" :size="12" />
      对比
    </button>
    <span class="hint pinned-session-hint">仅本次会话</span>
  </div>

  <div
    v-if="menu"
    class="context-menu"
    role="menu"
    :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
    @click.stop
  >
    <button class="context-menu-item" @click="startRename(menu.pin)">
      <Icon name="edit" :size="14" />
      重命名…
    </button>
    <button
      class="context-menu-item danger"
      @click="unpinFromMenu"
    >
      <Icon name="close" :size="14" />
      取消固定
    </button>
    <div class="context-separator"></div>
    <button
      class="context-menu-item"
      :disabled="Boolean(liveCompareReason)"
      :title="liveCompareReason || '与当前结果对比'"
      @click="compareCurrentFromMenu"
    >
      <Icon name="sync" :size="14" />
      与当前结果对比…
    </button>
    <button
      class="context-menu-item"
      :disabled="compareOtherDisabled"
      :title="
        compareOtherDisabled ? '需要至少两个固定结果' : '与另一个固定结果对比'
      "
      @click="compareOtherFromMenu"
    >
      <Icon name="sync" :size="14" />
      与另一个固定结果对比…
    </button>
    <div class="context-separator"></div>
    <button
      class="context-menu-item"
      @click="exportFromMenu"
    >
      <Icon name="download" :size="14" />
      导出固定结果…
    </button>
    <button
      class="context-menu-item"
      @click="copySqlFromMenu"
    >
      <Icon name="copy" :size="14" />
      复制固定时的 SQL
    </button>
  </div>

  <PromptDialog
    :open="Boolean(renameTarget)"
    title="重命名固定结果"
    message="名称 1–60 个字符，同标签内重名会自动追加序号。"
    :value="renameTarget?.name ?? ''"
    placeholder="例如：修复前订单"
    confirm-text="重命名"
    @confirm="confirmRename"
    @cancel="renameTarget = null"
  />
</template>
