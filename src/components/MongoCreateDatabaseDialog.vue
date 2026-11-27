<script setup lang="ts">
import { nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  busy?: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  cancel: [];
  confirm: [database: string, firstCollection: string];
}>();

const form = reactive({ database: "", firstCollection: "seed" });
const localError = ref("");
const databaseInput = ref<HTMLInputElement | null>(null);

function reset() {
  form.database = "";
  form.firstCollection = "seed";
  localError.value = "";
}

function validate(): boolean {
  const database = form.database.trim();
  const collection = form.firstCollection.trim();
  if (!database) {
    localError.value = "请输入数据库名称";
    return false;
  }
  if (database.length > 64) {
    localError.value = "数据库名称不能超过 64 个字符";
    return false;
  }
  if (/[\0/\\.\"$*<>:|?]/.test(database)) {
    localError.value = "数据库名称不能包含 / \\ . \" $ * < > : | ? 等字符";
    return false;
  }
  if (["admin", "local", "config"].includes(database)) {
    localError.value = "不能使用系统数据库名称 admin / local / config";
    return false;
  }
  if (!collection) {
    localError.value = "请输入首个集合名称";
    return false;
  }
  if (collection.length > 120) {
    localError.value = "集合名称不能超过 120 个字符";
    return false;
  }
  if (collection.includes("$") || collection.startsWith("system.")) {
    localError.value = "集合名称不能包含 $，也不能以 system. 开头";
    return false;
  }
  return true;
}

function submit() {
  if (props.busy || !validate()) return;
  emit("confirm", form.database.trim(), form.firstCollection.trim());
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") emit("cancel");
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      reset();
      document.addEventListener("keydown", onKeydown, true);
      await nextTick();
      databaseInput.value?.focus();
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
      aria-label="新建 MongoDB 数据库"
    >
      <header class="modal-header">
        <div>
          <h2>新建 MongoDB 数据库</h2>
          <p>MongoDB 会在创建首个集合时创建数据库；空数据库不会保留。</p>
        </div>
      </header>
      <div class="mongo-manage-form">
        <label>
          数据库名称
          <input
            ref="databaseInput"
            v-model="form.database"
            placeholder="例如：mydb_i03"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
        <label>
          首个集合名称
          <input
            v-model="form.firstCollection"
            placeholder="例如：seed"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
        <p class="hint">
          数据库名不能包含 / \ . " $ * &lt; &gt; : | ?，集合名不能包含 $ 或以 system. 开头；最终以服务器校验为准。
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
          {{ busy ? "创建中…" : "创建数据库" }}
        </button>
      </footer>
    </div>
  </div>
</template>
