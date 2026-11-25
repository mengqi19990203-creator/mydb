<script setup lang="ts">
type IconName =
  | "database"
  | "table"
  | "view"
  | "plus"
  | "refresh"
  | "history"
  | "play"
  | "close"
  | "chevron-down"
  | "chevron-up"
  | "chevron-right"
  | "chevron-left"
  | "search"
  | "structure"
  | "trash"
  | "download"
  | "upload"
  | "key"
  | "more"
  | "edit"
  | "check"
  | "warning"
  | "copy"
  | "filter"
  | "code"
  | "bolt"
  | "pin"
  | "clock"
  | "list"
  | "hash"
  | "minus"
  | "undo"
  | "redo"
  | "cut"
  | "paste"
  | "select-all"
  | "diagram"
  | "sync"
  | "target"
  | "collapse"
  | "external";

defineProps<{
  name: IconName;
  size?: number;
}>();
</script>

<template>
  <svg
    :width="size ?? 16"
    :height="size ?? 16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'database'">
      <ellipse cx="12" cy="5" rx="8" ry="3" />
      <path d="M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5" />
      <path d="M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6" />
    </template>
    <template v-else-if="name === 'table'">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M3 9h18M9 9v11" />
    </template>
    <template v-else-if="name === 'code'">
      <path d="m8 8-4 4 4 4M16 8l4 4-4 4M13.5 5l-3 14" />
    </template>
    <template v-else-if="name === 'bolt'">
      <path d="M13 2 4 14h6l-1 8 9-12h-6l1-8Z" />
    </template>
    <template v-else-if="name === 'pin'">
      <path d="M9 3h6l-1 5 3 3v1h-5v6.5L12 21l-1-2.5V12H6v-1l3-3-1-5Z" />
    </template>
    <template v-else-if="name === 'view'">
      <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z" />
      <circle cx="12" cy="12" r="2.5" />
    </template>
    <template v-else-if="name === 'plus'">
      <path d="M12 5v14M5 12h14" />
    </template>
    <template v-else-if="name === 'refresh'">
      <path d="M20 11a8 8 0 1 0-2.3 5.7" />
      <path d="M20 4v7h-7" />
    </template>
    <template v-else-if="name === 'history'">
      <path d="M3 12a9 9 0 1 0 3-6.7" />
      <path d="M3 4v5h5" />
      <path d="M12 7v5l3 2" />
    </template>
    <template v-else-if="name === 'play'">
      <path d="M7 4l13 8-13 8V4Z" />
    </template>
    <template v-else-if="name === 'close'">
      <path d="M6 6l12 12M18 6L6 18" />
    </template>
    <template v-else-if="name === 'chevron-down'">
      <path d="M6 9l6 6 6-6" />
    </template>
    <template v-else-if="name === 'chevron-up'">
      <path d="M6 15l6-6 6 6" />
    </template>
    <template v-else-if="name === 'chevron-right'">
      <path d="M9 6l6 6-6 6" />
    </template>
    <template v-else-if="name === 'chevron-left'">
      <path d="M15 6l-6 6 6 6" />
    </template>
    <template v-else-if="name === 'search'">
      <circle cx="11" cy="11" r="7" />
      <path d="M20 20l-4-4" />
    </template>
    <template v-else-if="name === 'structure'">
      <path d="M4 5h16M4 12h16M4 19h16" />
      <circle cx="7" cy="5" r="1" />
      <circle cx="7" cy="12" r="1" />
      <circle cx="7" cy="19" r="1" />
    </template>
    <template v-else-if="name === 'trash'">
      <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />
    </template>
    <template v-else-if="name === 'download'">
      <path d="M12 3v12M7 10l5 5 5-5M4 21h16" />
    </template>
    <template v-else-if="name === 'upload'">
      <path d="M12 21V9M7 14l5-5 5 5M4 3h16" />
    </template>
    <template v-else-if="name === 'key'">
      <circle cx="8" cy="15" r="4" />
      <path d="M11 12l8-8M16 4l4 4M14 6l4 4" />
    </template>
    <template v-else-if="name === 'more'">
      <circle cx="5" cy="12" r="1" />
      <circle cx="12" cy="12" r="1" />
      <circle cx="19" cy="12" r="1" />
    </template>
    <template v-else-if="name === 'edit'">
      <path d="M4 20h4L19 9l-4-4L4 16v4Z" />
      <path d="M13.5 6.5l4 4" />
    </template>
    <template v-else-if="name === 'check'">
      <path d="M5 12l5 5L20 7" />
    </template>
    <template v-else-if="name === 'warning'">
      <path d="M12 3L2 21h20L12 3Z" />
      <path d="M12 9v5M12 17h.01" />
    </template>
    <template v-else-if="name === 'copy'">
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15V5a2 2 0 0 1 2-2h10" />
    </template>
    <template v-else-if="name === 'filter'">
      <path d="M3 4h18l-7 8v6l-4 2v-8L3 4Z" />
    </template>
    <template v-else-if="name === 'clock'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </template>
    <template v-else-if="name === 'list'">
      <path d="M8 6h13M8 12h13M8 18h13" />
      <path d="M3 6h.01M3 12h.01M3 18h.01" />
    </template>
    <template v-else-if="name === 'hash'">
      <path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18" />
    </template>
    <template v-else-if="name === 'minus'">
      <path d="M5 12h14" />
    </template>
    <template v-else-if="name === 'undo'">
      <path d="M9 14 4 9l5-5" />
      <path d="M4 9h9a7 7 0 0 1 7 7v1" />
    </template>
    <template v-else-if="name === 'redo'">
      <path d="m15 14 5-5-5-5" />
      <path d="M20 9h-9a7 7 0 0 0-7 7v1" />
    </template>
    <template v-else-if="name === 'cut'">
      <circle cx="6" cy="6" r="2.5" />
      <circle cx="6" cy="18" r="2.5" />
      <path d="M8 7.5 20 19M8 16.5 20 5" />
    </template>
    <template v-else-if="name === 'paste'">
      <rect x="5" y="4" width="14" height="17" rx="2" />
      <path d="M9 4V3h6v1M9 10h6M9 14h6" />
    </template>
    <template v-else-if="name === 'select-all'">
      <path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3" />
      <path d="m9 12 2 2 4-4" />
    </template>
    <template v-else-if="name === 'diagram'">
      <rect x="3" y="4" width="6" height="5" rx="1" />
      <rect x="15" y="4" width="6" height="5" rx="1" />
      <rect x="9" y="15" width="6" height="5" rx="1" />
      <path d="M6 9v3h12V9M12 12v3" />
    </template>
    <template v-else-if="name === 'sync'">
      <path d="M4 9a7 7 0 0 1 12-4l3 3" />
      <path d="M20 4v4h-4" />
      <path d="M20 15a7 7 0 0 1-12 4l-3-3" />
      <path d="M4 20v-4h4" />
    </template>
    <template v-else-if="name === 'target'">
      <circle cx="12" cy="12" r="7" />
      <circle cx="12" cy="12" r="2.5" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3" />
    </template>
    <template v-else-if="name === 'collapse'">
      <path d="M8 4l4 4 4-4M8 20l4-4 4 4" />
      <path d="M4 12h16" />
    </template>
    <template v-else-if="name === 'external'">
      <path d="M14 4h6v6M20 4l-9 9" />
      <path d="M18 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h5" />
    </template>
  </svg>
</template>
