<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  database: string;
  from: string;
  existing: string[];
  busy?: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  cancel: [];
  confirm: [to: string];
}>();

const to = ref("");
const localError = ref("");
const input = ref<HTMLInputElement | null>(null);

function validate(): boolean {
  const name = to.value.trim();
  if (!name) {
    localError.value = "请输入新集合名称";
    return false;
  }
  if (name.length > 120) {
    localError.value = "集合名称不能超过 120 个字符";
    return false;
  }
  if (name.includes("$") || name.startsWith("system.")) {
    localError.value = "集合名称不能包含 $，也不能以 system. 开头";
    return false;
  }
  if (name === props.from) {
    localError.value = "新集合名不能与源集合名相同";
    return false;
  }
  if (props.existing.includes(name)) {
    localError.value = "目标集合已存在，请换一个名称";
    return false;
  }
  return true;
}

function submit() {
  if (props.busy || !validate()) return;
  emit("confirm", to.value.trim());
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") emit("cancel");
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      to.value = "";
      localError.value = "";
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      input.value?.focus();
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
      class="modal mongo-manage-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="重命名 MongoDB 集合"
    >
      <header class="modal-header">
        <div>
          <h2>重命名 MongoDB 集合</h2>
          <p>从 {{ database }}.{{ from }} 到 {{ database }}.<strong>{{ to || "…" }}</strong></p>
        </div>
      </header>
      <div class="mongo-manage-form">
        <label>
          新集合名称
          <input
            ref="input"
            v-model="to"
            placeholder="例如：events_renamed"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
        <p class="hint">
          只允许在同一数据库内重命名，不覆盖已有集合，不复制数据；大量写入时可能短暂阻塞。
        </p>
        <p v-if="localError || error" class="data-message warning">
          {{ localError || error }}
        </p>
      </div>
      <footer class="modal-actions">
        <button class="button ghost" :disabled="busy" @click="emit('cancel')">
          取消
        </button>
        <button class="button primary" :disabled="busy" @click="submit">
          {{ busy ? "重命名中…" : "重命名" }}
        </button>
      </footer>
    </div>
  </div>
</template>
