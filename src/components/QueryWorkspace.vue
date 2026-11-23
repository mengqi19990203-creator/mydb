<script setup lang="ts">
import { ref } from "vue";
import ResultGrid from "./ResultGrid.vue";
import SqlEditor from "./SqlEditor.vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import type { QueryResult } from "../lib/types";

defineProps<{
  sql: string;
  tabId: string;
  result: QueryResult | null;
  running: boolean;
  error: string | null;
  inTransaction: boolean;
  transactionBusy: boolean;
}>();

const emit = defineEmits<{
  "update:sql": [value: string];
  selection: [value: string];
  run: [sql?: string];
  "begin-transaction": [];
  "commit-transaction": [];
  "rollback-transaction": [];
}>();

const resultGrid = ref<InstanceType<typeof ResultGrid> | null>(null);
const hiddenColumns = ref(0);

async function copyError(error: string) {
  try {
    await writeClipboard(error);
  } catch {
    // Clipboard permission is optional; the error stays visible either way.
  }
}
</script>

<template>
  <section class="query-workspace">
    <div class="editor-panel">
      <div class="panel-header">
        <span>SQL 编辑器</span>
        <div class="editor-actions">
          <span class="hint">⌘ / Ctrl + Enter 运行 · 选中时仅执行选中部分</span>
          <span v-if="inTransaction" class="transaction-badge">
            事务中 · 未提交
          </span>
          <button
            v-if="!inTransaction"
            class="button ghost compact"
            :disabled="transactionBusy || running"
            @click="emit('begin-transaction')"
          >
            开始事务
          </button>
          <template v-else>
            <button
              class="button ghost compact"
              :disabled="transactionBusy || running"
              @click="emit('commit-transaction')"
            >
              提交
            </button>
            <button
              class="button ghost compact"
              :disabled="transactionBusy || running"
              @click="emit('rollback-transaction')"
            >
              回滚
            </button>
          </template>
        </div>
      </div>
      <SqlEditor
        :model-value="sql"
        :tab-id="tabId"
        @update:model-value="emit('update:sql', $event)"
        @selection="emit('selection', $event)"
        @run="emit('run', $event)"
      />
    </div>
    <div class="result-panel">
      <div class="panel-header">
        <span>结果</span>
        <div class="result-hints">
          <span v-if="result && !error" class="hint">
            {{ result.rows.length }} 行 · {{ result.elapsedMs }} ms
          </span>
          <button
            v-if="hiddenColumns && !error"
            class="link-button result-hidden-link"
            @click="resultGrid?.showAllColumns()"
          >
            已隐藏 {{ hiddenColumns }} 列 · 显示全部
          </button>
        </div>
      </div>

      <div v-if="error" class="query-error">
        <div class="query-error-head">
          <span class="query-error-title">
            <Icon name="warning" :size="14" />
            执行失败
          </span>
          <button class="button ghost compact" @click="copyError(error)">
            复制错误
          </button>
        </div>
        <pre class="query-error-body">{{ error }}</pre>
      </div>
      <ResultGrid
        v-else
        ref="resultGrid"
        :result="result"
        :running="running"
        @hidden-change="hiddenColumns = $event"
      />
    </div>
  </section>
</template>
