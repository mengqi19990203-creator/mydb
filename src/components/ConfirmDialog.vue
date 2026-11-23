<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  title: string;
  message: string;
  detail?: string | null;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const modalRef = ref<HTMLElement | null>(null);
const cancelRef = ref<HTMLButtonElement | null>(null);

function focusables(): HTMLElement[] {
  const modal = modalRef.value;
  if (!modal) return [];
  return Array.from(
    modal.querySelectorAll<HTMLElement>(
      'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  );
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("cancel");
    return;
  }
  if (event.key !== "Tab") return;

  const items = focusables();
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
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      cancelRef.value?.focus();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('cancel')">
    <div
      ref="modalRef"
      class="modal confirm-modal"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
    >
      <h2>{{ title }}</h2>
      <p class="confirm-message">{{ message }}</p>
      <pre v-if="detail" class="confirm-detail">{{ detail }}</pre>
      <div class="confirm-actions">
        <button
          ref="cancelRef"
          class="button ghost"
          @click="emit('cancel')"
        >
          {{ cancelText ?? "取消" }}
        </button>
        <button
          class="button"
          :class="danger === false ? 'primary' : 'danger'"
          @click="emit('confirm')"
        >
          {{ confirmText ?? "确认" }}
        </button>
      </div>
    </div>
  </div>
</template>