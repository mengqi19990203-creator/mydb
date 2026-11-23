<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  title: string;
  message: string;
  value?: string;
  placeholder?: string;
  confirmText?: string;
  checkboxLabel?: string;
  checkboxDefault?: boolean;
}>();

const emit = defineEmits<{
  confirm: [value: string, checked: boolean];
  cancel: [];
}>();

const draft = ref("");
const checked = ref(false);
const error = ref("");
const inputRef = ref<HTMLInputElement | null>(null);

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("cancel");
  }
}

function submit() {
  const value = draft.value.trim();
  if (!value) {
    error.value = "名称不能为空";
    return;
  }
  error.value = "";
  emit("confirm", value, checked.value);
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      draft.value = props.value ?? "";
      checked.value = props.checkboxDefault ?? true;
      error.value = "";
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      inputRef.value?.focus();
      inputRef.value?.select();
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
      class="modal confirm-modal prompt-modal"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
    >
      <h2>{{ title }}</h2>
      <p class="confirm-message">{{ message }}</p>
      <input
        ref="inputRef"
        v-model="draft"
        class="prompt-input"
        :placeholder="placeholder ?? '输入新名称'"
        @keydown.enter.prevent="submit"
      />
      <p v-if="error" class="message error">{{ error }}</p>
      <label v-if="checkboxLabel" class="prompt-checkbox">
        <input v-model="checked" type="checkbox" />
        <span>{{ checkboxLabel }}</span>
      </label>
      <div class="confirm-actions">
        <button class="button ghost" @click="emit('cancel')">取消</button>
        <button class="button primary" @click="submit">
          {{ confirmText ?? "确定" }}
        </button>
      </div>
    </div>
  </div>
</template>