<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import type { MenuBarItem, MenuBarMenu } from "../lib/app-actions";

const props = defineProps<{
  menus: MenuBarMenu[];
}>();

const root = ref<HTMLElement | null>(null);
const openIndex = ref<number | null>(null);
const menuTop = ref(0);
const menuLeft = ref(0);

const activeMenu = computed(() =>
  openIndex.value === null ? null : props.menus[openIndex.value] ?? null,
);

function placeMenu(element: HTMLElement) {
  const rect = element.getBoundingClientRect();
  menuTop.value = rect.bottom + 4;
  menuLeft.value = Math.max(
    8,
    Math.min(rect.left, window.innerWidth - 260),
  );
}

function toggleMenu(index: number, event: MouseEvent) {
  if (openIndex.value === index) {
    openIndex.value = null;
    return;
  }
  placeMenu(event.currentTarget as HTMLElement);
  openIndex.value = index;
}

function hoverMenu(index: number, event: MouseEvent) {
  if (openIndex.value === null || openIndex.value === index) return;
  placeMenu(event.currentTarget as HTMLElement);
  openIndex.value = index;
}

function closeMenu() {
  openIndex.value = null;
}

function runItem(item: MenuBarItem) {
  if (item.separator || item.disabled || !item.action) return;
  closeMenu();
  void item.action();
}

function onDocumentMouseDown(event: MouseEvent) {
  if (openIndex.value === null) return;
  const target = event.target as Node | null;
  if (target && root.value?.contains(target)) return;
  closeMenu();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && openIndex.value !== null) {
    event.stopPropagation();
    closeMenu();
  }
}

onMounted(() => {
  document.addEventListener("mousedown", onDocumentMouseDown);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", closeMenu);
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocumentMouseDown);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", closeMenu);
});
</script>

<template>
  <nav ref="root" class="app-menubar" aria-label="应用菜单">
    <button
      v-for="(menu, index) in menus"
      :key="menu.label"
      class="app-menubar-button"
      :class="{ active: openIndex === index }"
      :aria-expanded="openIndex === index"
      aria-haspopup="menu"
      @click="toggleMenu(index, $event)"
      @mouseenter="hoverMenu(index, $event)"
    >
      {{ menu.label }}
    </button>

    <div
      v-if="activeMenu"
      class="context-menu app-menubar-menu"
      :style="{
        position: 'fixed',
        top: `${menuTop}px`,
        left: `${menuLeft}px`,
      }"
      role="menu"
      @click.stop
    >
      <template v-for="(item, index) in activeMenu.items" :key="index">
        <div v-if="item.separator" class="context-separator"></div>
        <button
          v-else
          class="context-menu-item"
          :class="{ danger: item.danger }"
          role="menuitem"
          :disabled="item.disabled"
          :title="item.disabledReason || undefined"
          @click="runItem(item)"
        >
          <span>{{ item.label }}</span>
          <span v-if="item.shortcut" class="context-menu-shortcut">
            {{ item.shortcut }}
          </span>
        </button>
      </template>
    </div>
  </nav>
</template>