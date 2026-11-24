<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import Icon from "./Icon.vue";
import type { MenuBarItem, MenuBarMenu } from "../lib/app-actions";

const props = defineProps<{
  menus: MenuBarMenu[];
}>();

const root = ref<HTMLElement | null>(null);
const menuRef = ref<HTMLElement | null>(null);
const openIndex = ref<number | null>(null);
const focusIndex = ref(0);
const itemFocus = ref(0);
const menuTop = ref(0);
const menuLeft = ref(0);
let typeahead = "";
let typeaheadTimer = 0;

const activeMenu = computed(() =>
  openIndex.value === null ? null : props.menus[openIndex.value] ?? null,
);

function menuButtons(): HTMLButtonElement[] {
  return Array.from(
    root.value?.querySelectorAll<HTMLButtonElement>(
      ".app-menubar-button",
    ) ?? [],
  );
}

function placeMenu(index: number) {
  const element = menuButtons()[index];
  if (!element) return;
  const rect = element.getBoundingClientRect();
  menuTop.value = rect.bottom + 4;
  menuLeft.value = Math.max(
    8,
    Math.min(rect.left, window.innerWidth - 300),
  );
}

function itemIndices(): number[] {
  const items = activeMenu.value?.items ?? [];
  return items
    .map((item, index) => (item.separator ? -1 : index))
    .filter((index) => index >= 0);
}

function firstItemIndex(): number {
  return itemIndices()[0] ?? -1;
}

function menuItemElements(): HTMLButtonElement[] {
  return Array.from(
    menuRef.value?.querySelectorAll<HTMLButtonElement>(
      "[data-menu-item]",
    ) ?? [],
  );
}

function focusMenuItem(index: number) {
  itemFocus.value = index;
  void nextTick(() => {
    const element = menuItemElements().find(
      (item) => Number(item.dataset.menuItem) === index,
    );
    element?.focus();
  });
}

function focusButton(index: number) {
  focusIndex.value = index;
  menuButtons()[index]?.focus();
}

function openMenu(index: number, focusFirst = true) {
  placeMenu(index);
  openIndex.value = index;
  typeahead = "";
  const first = firstItemIndex();
  itemFocus.value = first;
  if (focusFirst && first >= 0) focusMenuItem(first);
}

function closeMenu(restoreFocus = false) {
  const index = openIndex.value;
  openIndex.value = null;
  typeahead = "";
  if (restoreFocus && index !== null) {
    void nextTick(() => menuButtons()[index]?.focus());
  }
}

function toggleMenu(index: number) {
  if (openIndex.value === index) {
    closeMenu(true);
    return;
  }
  openMenu(index);
}

function hoverMenu(index: number) {
  if (openIndex.value === null || openIndex.value === index) return;
  openMenu(index, false);
}

function runItem(item: MenuBarItem) {
  if (item.separator || item.disabled || !item.action) return;
  const index = openIndex.value;
  closeMenu(false);
  if (index !== null) menuButtons()[index]?.focus();
  void item.action();
}

function moveItem(delta: number) {
  const indices = itemIndices();
  if (!indices.length) return;
  const position = indices.indexOf(itemFocus.value);
  const nextPosition =
    position < 0
      ? delta > 0
        ? 0
        : indices.length - 1
      : (position + delta + indices.length) % indices.length;
  focusMenuItem(indices[nextPosition]);
}

function switchTopMenu(delta: number) {
  if (openIndex.value === null) return;
  const count = props.menus.length;
  const next = (openIndex.value + delta + count) % count;
  openMenu(next);
}

function onButtonKeydown(index: number, event: KeyboardEvent) {
  if (event.isComposing) return;
  const count = props.menus.length;
  switch (event.key) {
    case "ArrowRight": {
      event.preventDefault();
      const next = (index + 1) % count;
      if (openIndex.value === null) focusButton(next);
      else openMenu(next);
      break;
    }
    case "ArrowLeft": {
      event.preventDefault();
      const next = (index - 1 + count) % count;
      if (openIndex.value === null) focusButton(next);
      else openMenu(next);
      break;
    }
    case "Home":
      event.preventDefault();
      if (openIndex.value === null) focusButton(0);
      else openMenu(0);
      break;
    case "End":
      event.preventDefault();
      if (openIndex.value === null) focusButton(count - 1);
      else openMenu(count - 1);
      break;
    case "ArrowDown":
    case "Enter":
    case " ":
      event.preventDefault();
      openMenu(index);
      break;
    case "Escape":
      if (openIndex.value !== null) {
        event.preventDefault();
        closeMenu(true);
      }
      break;
  }
}

function runTypeahead(query: string) {
  const items = activeMenu.value?.items ?? [];
  const indices = itemIndices();
  if (!indices.length) return;
  const position = indices.indexOf(itemFocus.value);
  for (let step = 1; step <= indices.length; step += 1) {
    const index =
      indices[(position + step + indices.length) % indices.length];
    const label = (items[index].label ?? "").toLowerCase();
    if (label.startsWith(query)) {
      focusMenuItem(index);
      return;
    }
  }
}

function onMenuKeydown(event: KeyboardEvent) {
  if (event.isComposing) return;
  const items = activeMenu.value?.items ?? [];
  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      moveItem(1);
      break;
    case "ArrowUp":
      event.preventDefault();
      moveItem(-1);
      break;
    case "Home":
      event.preventDefault();
      if (firstItemIndex() >= 0) focusMenuItem(firstItemIndex());
      break;
    case "End": {
      event.preventDefault();
      const indices = itemIndices();
      if (indices.length) focusMenuItem(indices[indices.length - 1]);
      break;
    }
    case "Enter":
    case " ": {
      event.preventDefault();
      const item = items[itemFocus.value];
      if (item) runItem(item);
      break;
    }
    case "Escape":
      event.preventDefault();
      event.stopPropagation();
      closeMenu(true);
      break;
    case "Tab":
      {
        const index = openIndex.value;
        closeMenu(false);
        if (index !== null) menuButtons()[index]?.focus();
      }
      break;
    case "ArrowRight":
      event.preventDefault();
      switchTopMenu(1);
      break;
    case "ArrowLeft":
      event.preventDefault();
      switchTopMenu(-1);
      break;
    default: {
      if (
        event.key.length === 1 &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey
      ) {
        window.clearTimeout(typeaheadTimer);
        typeahead += event.key.toLowerCase();
        typeaheadTimer = window.setTimeout(() => {
          typeahead = "";
        }, 500);
        runTypeahead(typeahead);
        event.preventDefault();
      }
    }
  }
}

function closeOnResize() {
  closeMenu(false);
}

function onDocumentMouseDown(event: MouseEvent) {
  if (openIndex.value === null) return;
  const target = event.target as Node | null;
  if (target && root.value?.contains(target)) return;
  closeMenu(false);
}

onMounted(() => {
  document.addEventListener("mousedown", onDocumentMouseDown);
  window.addEventListener("resize", closeOnResize);
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocumentMouseDown);
  window.removeEventListener("resize", closeOnResize);
  window.clearTimeout(typeaheadTimer);
});
</script>

<template>
  <nav
    ref="root"
    class="app-menubar"
    role="menubar"
    aria-label="应用菜单"
  >
    <button
      v-for="(menu, index) in menus"
      :key="menu.label"
      class="app-menubar-button"
      :class="{ active: openIndex === index }"
      role="menuitem"
      :tabindex="openIndex === index || focusIndex === index ? 0 : -1"
      :aria-expanded="openIndex === index"
      aria-haspopup="menu"
      @focus="focusIndex = index"
      @click="toggleMenu(index)"
      @mouseenter="hoverMenu(index)"
      @keydown="onButtonKeydown(index, $event)"
    >
      {{ menu.label }}
    </button>

    <div
      v-if="activeMenu"
      ref="menuRef"
      class="context-menu app-menubar-menu"
      :style="{
        position: 'fixed',
        top: `${menuTop}px`,
        left: `${menuLeft}px`,
      }"
      role="menu"
      :aria-label="activeMenu.label"
      @click.stop
      @keydown="onMenuKeydown"
    >
      <template v-for="(item, index) in activeMenu.items" :key="index">
        <div
          v-if="item.separator"
          class="context-separator"
          role="separator"
        ></div>
        <button
          v-else
          class="context-menu-item"
          :class="{ danger: item.danger, disabled: item.disabled }"
          :role="item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'"
          :aria-checked="
            item.checked === undefined ? undefined : item.checked
          "
          :aria-disabled="item.disabled ? 'true' : undefined"
          tabindex="-1"
          :data-menu-item="index"
          :title="item.disabledReason || undefined"
          @mouseenter="itemFocus = index"
          @click="runItem(item)"
        >
          <span class="layout-check-slot" aria-hidden="true">
            <Icon v-if="item.checked" name="check" :size="13" />
          </span>
          <span>{{ item.label }}</span>
          <span v-if="item.shortcut" class="context-menu-shortcut">
            {{ item.shortcut }}
          </span>
        </button>
      </template>
    </div>
  </nav>
</template>