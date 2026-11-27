<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  database: string;
  collectionCount: number | null;
  busy?: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  cancel: [];
  confirm: [confirmName: string];
}>();

const confirmName = ref("");
const cancelButton = ref<HTMLButtonElement | null>(null);

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") emit("cancel");
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      confirmName.value = "";
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      cancelButton.value?.focus();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('cancel')">
    <div
      class="modal confirm-modal mongo-manage-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="删除 MongoDB 数据库"
    >
      <h2>删除 MongoDB 数据库</h2>
      <p class="confirm-message">
        即将删除数据库「{{ database }}」
        <template v-if="collectionCount !== null">
          （{{ collectionCount }} 个集合）
        </template>
        <template v-else>（集合数量未知）</template>
        ，将删除该数据库下的全部集合与数据，且不可恢复。
      </p>
      <p class="hint">请输入数据库名称 <code>{{ database }}</code> 以确认：</p>
      <input
        v-model="confirmName"
        class="mongo-drop-confirm-input"
        autocomplete="off"
        spellcheck="false"
        :placeholder="database"
        aria-label="输入数据库名称确认"
      />
      <p v-if="error" class="data-message warning">{{ error }}</p>
      <div class="confirm-actions">
        <button ref="cancelButton" class="button ghost" :disabled="busy" @click="emit('cancel')">
          取消
        </button>
        <button
          class="button danger"
          :disabled="busy || confirmName !== database"
          @click="emit('confirm', confirmName)"
        >
          {{ busy ? "删除中…" : "删除数据库" }}
        </button>
      </div>
    </div>
  </div>
</template>
