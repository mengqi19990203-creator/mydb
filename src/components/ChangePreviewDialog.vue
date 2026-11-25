<script setup lang="ts">
import { computed, ref } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";

const props = defineProps<{
  open: boolean;
  table: string;
  statements: string[];
  saving: boolean;
}>();

const emit = defineEmits<{
  close: [];
  submit: [];
}>();

const copied = ref(false);

const preview = computed(() =>
  props.statements.length
    ? props.statements.join("\n")
    : "-- 没有待提交变更",
);

async function copySql() {
  try {
    await writeClipboard(preview.value);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="!saving && emit('close')">
    <div
      class="modal create-table-modal"
      role="dialog"
      aria-modal="true"
      aria-label="待提交变更 SQL"
    >
      <header class="modal-header">
        <div>
          <h2>待提交变更</h2>
          <p>{{ table }} · 删除 → 更新 → 新增，预计每条更新 / 删除影响 1 行</p>
        </div>
        <button
          class="icon-button"
          title="关闭"
          :disabled="saving"
          @click="emit('close')"
        >
          <Icon name="close" />
        </button>
      </header>
      <div class="connection-form">
        <div class="draft-preview-head">
          <strong>{{ statements.length }} 条语句</strong>
          <button class="button ghost compact" @click="copySql">
            {{ copied ? "已复制" : "复制 SQL" }}
          </button>
        </div>
        <pre class="ddl-preview change-preview">{{ preview }}</pre>
        <p class="draft-hint">
          这里只预览 SQL，不会执行；点击「提交变更」后才会在单个事务中写入数据库。
        </p>
      </div>
      <footer class="modal-footer">
        <button class="button ghost" :disabled="saving" @click="emit('close')">
          返回编辑
        </button>
        <div class="spacer"></div>
        <button
          class="button primary"
          :disabled="saving || !statements.length"
          @click="emit('submit')"
        >
          {{ saving ? "提交中…" : "提交变更" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.change-preview {
  max-height: 360px;
}

.draft-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}
</style>