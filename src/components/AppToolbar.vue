<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import Icon from "./Icon.vue";
import {
  ACTION_GROUP_LABELS,
  ACTION_GROUP_ORDER,
  type AppAction,
} from "../lib/app-actions";

const props = defineProps<{
  actions: AppAction[];
}>();

const root = ref<HTMLElement | null>(null);
const moreButton = ref<HTMLElement | null>(null);
const containerWidth = ref(0);
const moreMenuOpen = ref(false);
const moreMenuTop = ref(0);
const moreMenuRight = ref(0);
let observer: ResizeObserver | null = null;

const primary = computed(() =>
  props.actions
    .filter((action) => (action.placement ?? "primary") === "primary")
    .sort((left, right) => left.priority - right.priority),
);
const moreBase = computed(() =>
  props.actions.filter((action) => action.placement === "more"),
);

const labelMode = computed(() => {
  const width = containerWidth.value || 900;
  const labelWidths = primary.value.reduce(
    (total, action) => total + Math.max(72, action.label.length * 13 + 40) + 4,
    0,
  );
  return labelWidths + 40 + 34 <= width;
});

const visibleIds = computed(() => {
  const width = containerWidth.value || 900;
  let used = 34 + 40;
  const visible = new Set<string>();
  for (const action of primary.value) {
    const labeled = labelMode.value || action.priority <= 3;
    const actionWidth = labeled
      ? Math.max(72, action.label.length * 13 + 40) + 4
      : 36;
    if (used + actionWidth <= width) {
      visible.add(action.id);
      used += actionWidth;
    }
  }
  return visible;
});

function showLabel(action: AppAction): boolean {
  return labelMode.value || action.priority <= 3;
}

const renderGroups = computed(() =>
  ACTION_GROUP_ORDER.map((group) => ({
    group,
    actions: primary.value.filter(
      (action) => action.group === group && visibleIds.value.has(action.id),
    ),
  })).filter((entry) => entry.actions.length > 0),
);

const moreActions = computed(() => [
  ...moreBase.value,
  ...primary.value.filter((action) => !visibleIds.value.has(action.id)),
]);

const moreGroups = computed(() =>
  ACTION_GROUP_ORDER.map((group) => ({
    group,
    actions: moreActions.value.filter((action) => action.group === group),
  })).filter((entry) => entry.actions.length > 0),
);

function actionTitle(action: AppAction): string {
  const parts = [action.label];
  if (action.shortcut) parts.push(`（${action.shortcut}）`);
  if (!action.enabled && action.disabledReason) {
    parts.push(` · ${action.disabledReason}`);
  }
  return parts.join("");
}

function runAction(action: AppAction) {
  if (!action.enabled) return;
  moreMenuOpen.value = false;
  void action.run();
}

function toggleMoreMenu() {
  if (moreMenuOpen.value) {
    moreMenuOpen.value = false;
    return;
  }
  const rect = moreButton.value?.getBoundingClientRect();
  if (!rect) return;
  moreMenuTop.value = rect.bottom + 6;
  moreMenuRight.value = Math.max(8, window.innerWidth - rect.right);
  moreMenuOpen.value = true;
}

function closeMoreMenu() {
  moreMenuOpen.value = false;
}

function onDocumentMouseDown(event: MouseEvent) {
  if (!moreMenuOpen.value) return;
  const target = event.target as Node | null;
  if (target && root.value?.contains(target)) return;
  closeMoreMenu();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && moreMenuOpen.value) {
    event.stopPropagation();
    closeMoreMenu();
  }
}

onMounted(() => {
  observer = new ResizeObserver((entries) => {
    const entry = entries[0];
    if (entry) containerWidth.value = entry.contentRect.width;
  });
  if (root.value) observer.observe(root.value);
  document.addEventListener("mousedown", onDocumentMouseDown);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", closeMoreMenu);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  document.removeEventListener("mousedown", onDocumentMouseDown);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", closeMoreMenu);
});

defineExpose({ closeMoreMenu });
</script>

<template>
  <div ref="root" class="app-toolbar" role="toolbar" aria-label="应用工具栏">
    <template
      v-for="(group, index) in renderGroups"
      :key="group.group"
    >
      <span v-if="index > 0" class="app-toolbar-separator"></span>
      <button
        v-for="action in group.actions"
        :key="action.id"
        class="toolbar-action app-toolbar-action"
        :class="{ 'icon-only': !showLabel(action), active: action.active }"
        :disabled="!action.enabled"
        :title="actionTitle(action)"
        :aria-label="actionTitle(action)"
        @click="runAction(action)"
      >
        <Icon :name="action.icon" :size="14" />
        <span v-if="showLabel(action)" class="app-toolbar-label">
          {{ action.label }}
        </span>
      </button>
    </template>

    <span class="app-toolbar-spacer"></span>

    <button
      v-if="moreActions.length"
      ref="moreButton"
      class="toolbar-action app-toolbar-action app-toolbar-more"
      :class="{ 'icon-only': !labelMode, active: moreMenuOpen }"
      title="更多操作"
      aria-label="更多操作"
      :aria-expanded="moreMenuOpen"
      @click="toggleMoreMenu"
    >
      <Icon name="more" :size="14" />
      <span v-if="labelMode" class="app-toolbar-label">更多</span>
    </button>

    <div
      v-if="moreMenuOpen"
      class="context-menu app-toolbar-more-menu"
      :style="{
        position: 'fixed',
        top: `${moreMenuTop}px`,
        right: `${moreMenuRight}px`,
        left: 'auto',
      }"
      role="menu"
      @click.stop
    >
      <template
        v-for="(group, index) in moreGroups"
        :key="group.group"
      >
        <div class="context-menu-hint">
          {{ ACTION_GROUP_LABELS[group.group] }}
        </div>
        <button
          v-for="action in group.actions"
          :key="action.id"
          class="context-menu-item"
          role="menuitem"
          :disabled="!action.enabled"
          :title="actionTitle(action)"
          @click="runAction(action)"
        >
          <Icon :name="action.icon" :size="14" />
          <span>{{ action.label }}</span>
          <span
            v-if="action.shortcut"
            class="context-menu-shortcut"
          >
            {{ action.shortcut }}
          </span>
        </button>
        <div
          v-if="index < moreGroups.length - 1"
          class="context-separator"
        ></div>
      </template>
    </div>
  </div>
</template>