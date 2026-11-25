<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import {
  DEFAULT_FORMAT_OPTIONS,
  loadFormatOptions,
  saveFormatOptions,
  type SqlFormatOptions,
} from "../lib/sql-format";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const options = ref<SqlFormatOptions>({ ...DEFAULT_FORMAT_OPTIONS });

function reset() {
  options.value = { ...DEFAULT_FORMAT_OPTIONS };
}

function save() {
  saveFormatOptions({ ...options.value });
  emit("saved");
  emit("close");
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
      options.value = loadFormatOptions();
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
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal confirm-modal prompt-modal" role="dialog" aria-modal="true">
      <div class="modal-header">
        <div>
          <h2>SQL 格式化设置</h2>
          <p>只影响本机编辑器的格式化 / 压缩行为。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </div>

      <label class="export-field">
        <span class="export-label">关键字大小写</span>
        <select v-model="options.keywordCase">
          <option value="upper">大写（SELECT）</option>
          <option value="lower">小写（select）</option>
          <option value="preserve">保持原样</option>
        </select>
      </label>

      <label class="export-field">
        <span class="export-label">缩进</span>
        <select v-model.number="options.tabWidth">
          <option :value="2">2 空格</option>
          <option :value="4">4 空格</option>
        </select>
      </label>

      <label class="export-field">
        <span class="export-label">语句间空行</span>
        <select v-model.number="options.linesBetweenQueries">
          <option :value="0">0 行</option>
          <option :value="1">1 行</option>
          <option :value="2">2 行</option>
        </select>
      </label>

      <label class="export-check">
        <input v-model="options.preserveComments" type="checkbox" />
        <span>格式化时保留注释（压缩始终移除普通注释）</span>
      </label>

      <p class="hint">
        压缩会保留字符串、引号标识符、MySQL 可执行注释与优化器提示；含 DELIMITER、过程体或超过 100 KB 时会拒绝处理。
      </p>

      <div class="confirm-actions">
        <button class="button ghost" @click="reset">恢复默认</button>
        <span class="spacer"></span>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button class="button primary" @click="save">保存设置</button>
      </div>
    </div>
  </div>
</template>