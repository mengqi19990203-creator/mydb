<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";

const MYSQL_CHARSETS = ["utf8mb4", "utf8mb3", "latin1", "gbk", "big5", "binary"];

const props = defineProps<{
  open: boolean;
  driver: "mysql" | "postgres";
  mode?: "database" | "schema";
}>();

const emit = defineEmits<{
  close: [];
  confirm: [
    payload: {
      name: string;
      charset: string | null;
      collation: string | null;
      switchAfter: boolean;
    },
  ];
}>();

const name = ref("");
const charset = ref("utf8mb4");
const collation = ref("");
const switchAfter = ref(true);
const error = ref("");
const submitting = ref(false);

const isMysql = computed(
  () => props.driver === "mysql" && props.mode !== "schema",
);
const isSchema = computed(() => props.mode === "schema");
const title = computed(() =>
  isSchema.value
    ? "新建模式"
    : isMysql.value
      ? "新建数据库"
      : "新建数据库 / 模式",
);

function validate(): string {
  const value = name.value.trim();
  if (!value) return "名称不能为空";
  if (value.includes(";")) return "名称不能包含分号";
  const limit = isMysql.value ? 64 : 63;
  if (value.length > limit) return `名称不能超过 ${limit} 个字符`;
  if (
    isMysql.value &&
    ["information_schema", "mysql", "performance_schema", "sys"].includes(
      value.toLowerCase(),
    )
  ) {
    return "不能创建系统数据库";
  }
  return "";
}

function reset() {
  name.value = "";
  charset.value = "utf8mb4";
  collation.value = "";
  switchAfter.value = true;
  error.value = "";
  submitting.value = false;
}

function submit() {
  const issue = validate();
  if (issue) {
    error.value = issue;
    return;
  }
  error.value = "";
  submitting.value = true;
  emit("confirm", {
    name: name.value.trim(),
    charset: isMysql.value ? charset.value : null,
    collation: isMysql.value ? collation.value.trim() || null : null,
    switchAfter: isMysql.value && switchAfter.value,
  });
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});

defineExpose({
  setError(message: string) {
    error.value = message;
    submitting.value = false;
  },
  done() {
    submitting.value = false;
  },
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal confirm-modal prompt-modal" role="dialog" aria-modal="true">
      <div class="modal-header">
        <h2>{{ title }}</h2>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </div>

      <label class="export-field">
        <span class="export-label">名称</span>
        <input
          v-model="name"
          autofocus
          type="text"
          placeholder="例如 mydb_demo"
          spellcheck="false"
          @keydown.enter.prevent="submit"
        />
      </label>

      <template v-if="isMysql">
        <label class="export-field">
          <span class="export-label">字符集</span>
          <select v-model="charset">
            <option v-for="value in MYSQL_CHARSETS" :key="value" :value="value">
              {{ value }}
            </option>
          </select>
        </label>
        <label class="export-field">
          <span class="export-label">排序规则</span>
          <input
            v-model="collation"
            type="text"
            placeholder="留空跟随字符集默认"
            spellcheck="false"
          />
        </label>
        <label class="export-check">
          <input v-model="switchAfter" type="checkbox" />
          <span>创建后切换到此数据库</span>
        </label>
      </template>
      <p v-else-if="isSchema" class="hint">
        将在当前 PostgreSQL 数据库中执行 CREATE SCHEMA；成功后刷新模式列表并可切换。
      </p>
      <p v-else class="hint">
        第一版只创建数据库；创建后需要刷新连接才能浏览。模式请使用对象树中的「新建模式」。
      </p>

      <p class="hint">
        名称会按驱动安全引用；不会覆盖或级联删除已有对象。
      </p>
      <p v-if="error" class="message error">{{ error }}</p>

      <div class="confirm-actions">
        <button class="button ghost" :disabled="submitting" @click="emit('close')">
          取消
        </button>
        <button class="button primary" :disabled="submitting" @click="submit">
          {{ submitting ? "创建中…" : "创建" }}
        </button>
      </div>
    </div>
  </div>
</template>