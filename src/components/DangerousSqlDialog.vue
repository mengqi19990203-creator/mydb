<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";

const props = defineProps<{
  open: boolean;
  sql: string;
  message: string;
  connectionName: string;
  scope: string;
  highRisk: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const riskAccepted = ref(false);

function reset() {
  riskAccepted.value = false;
}

function cancel() {
  emit("cancel");
}

watch(
  () => props.open,
  (open) => {
    if (open) reset();
  },
  { immediate: true },
);

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    cancel();
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) document.addEventListener("keydown", onKeydown, true);
    else document.removeEventListener("keydown", onKeydown, true);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="cancel">
    <div class="modal confirm-modal dangerous-sql-modal" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>危险 SQL 确认</h2>
          <p>{{ connectionName }} · {{ scope }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="cancel">
          <Icon name="close" />
        </button>
      </header>

      <p class="dangerous-sql-warning">{{ message }}</p>
      <pre class="dangerous-sql-body">{{ sql.slice(0, 4000) }}{{ sql.length > 4000 ? "\n…（已截断）" : "" }}</pre>

      <label v-if="highRisk" class="export-check">
        <input v-model="riskAccepted" type="checkbox" />
        <span>我了解此操作不可恢复，并已确认连接与对象无误。</span>
      </label>
      <p class="hint">
        这是启发式提醒，用于减少误操作，不替代备份、权限控制和事务确认。
      </p>

      <div class="confirm-actions">
        <button class="button ghost" @click="cancel">取消</button>
        <button
          class="button danger"
          :disabled="highRisk && !riskAccepted"
          @click="emit('confirm')"
        >
          仍然执行
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dangerous-sql-modal {
  width: min(720px, calc(100vw - 48px));
}

.dangerous-sql-warning {
  margin: 0 20px 8px;
  padding: 8px 10px;
  border: 1px solid var(--danger-border, rgba(248, 81, 73, 0.4));
  border-radius: 7px;
  background: var(--danger-soft);
  color: var(--danger);
  font-size: 12px;
}

.dangerous-sql-body {
  max-height: 260px;
  overflow: auto;
  margin: 0 20px 10px;
  padding: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  white-space: pre-wrap;
}
</style>