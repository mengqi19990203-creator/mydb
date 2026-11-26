<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import type {
  SqlCheckIssue,
  SqlCheckResult,
  SqlCheckSeverity,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  result: SqlCheckResult | null;
  checking: boolean;
  error: string;
  scopeLabel: string;
}>();

const emit = defineEmits<{
  close: [];
  recheck: [];
  jump: [issue: SqlCheckIssue];
}>();

const filter = ref<"all" | SqlCheckSeverity>("all");
const copied = ref("");
const modalRef = ref<HTMLElement | null>(null);
let copyTimer = 0;
let previousFocus: HTMLElement | null = null;

const counts = computed(() => {
  const value = { error: 0, warning: 0, info: 0 };
  for (const issue of props.result?.issues ?? []) value[issue.severity] += 1;
  return value;
});
const filtered = computed(() => {
  const issues = props.result?.issues ?? [];
  return filter.value === "all"
    ? issues
    : issues.filter((issue) => issue.severity === filter.value);
});
const hasIssues = computed(() => Boolean(props.result?.issues.length));

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      window.removeEventListener("keydown", onKeydown);
      const target = previousFocus;
      previousFocus = null;
      if (target?.isConnected) void nextTick(() => target.focus());
      return;
    }
    previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    filter.value = "all";
    copied.value = "";
    window.addEventListener("keydown", onKeydown);
    await nextTick();
    modalRef.value?.querySelector<HTMLButtonElement>(".sqlcheck-close")?.focus();
  },
  { immediate: true },
);

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
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
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

onBeforeUnmount(() => {
  window.clearTimeout(copyTimer);
  window.removeEventListener("keydown", onKeydown);
});

function severityLabel(severity: SqlCheckSeverity): string {
  return severity === "error" ? "错误" : severity === "warning" ? "警告" : "提示";
}

async function copyIssues() {
  const issues = props.result?.issues ?? [];
  if (!issues.length) return;
  const lines = issues.map(
    (issue) =>
      `${severityLabel(issue.severity)} ${issue.line}:${issue.column} [${issue.code}] ${issue.message}`,
  );
  try {
    await writeClipboard(lines.join("\n"));
    copied.value = "已复制问题";
    window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copied.value = "";
    }, 2200);
  } catch (reason) {
    copied.value = `复制失败：${reason}`;
  }
}

function jump(issue: SqlCheckIssue) {
  emit("jump", issue);
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      ref="modalRef"
      class="modal sqlcheck-modal"
      role="dialog"
      aria-modal="true"
      aria-label="SQL 静态检查结果"
    >
      <header class="sqlcheck-head">
        <div>
          <h2>SQL 检查</h2>
          <p class="hint">
            检查范围：{{ scopeLabel }}
            <span v-if="result?.tokenCount">
              · {{ result.tokenCount.toLocaleString("zh-CN") }} 个 token
            </span>
          </p>
        </div>
        <button
          class="icon-button sqlcheck-close"
          title="关闭"
          aria-label="关闭检查结果"
          @click="emit('close')"
        >
          <Icon name="close" />
        </button>
      </header>

      <div class="sqlcheck-summary" aria-live="polite">
        <span class="sqlcheck-count error">{{ counts.error }} 个错误</span>
        <span class="sqlcheck-count warning">{{ counts.warning }} 个警告</span>
        <span class="sqlcheck-count info">{{ counts.info }} 条提示</span>
        <span v-if="checking" class="hint">检查中…</span>
        <span v-else-if="copied" class="hint">{{ copied }}</span>
      </div>

      <div class="sqlcheck-filters" role="group" aria-label="按级别筛选">
        <button
          v-for="item in [
            { value: 'all', label: '全部' },
            { value: 'error', label: '错误' },
            { value: 'warning', label: '警告' },
            { value: 'info', label: '提示' },
          ] as Array<{ value: 'all' | SqlCheckSeverity; label: string }>"
          :key="item.value"
          class="segment"
          :class="{ active: filter === item.value }"
          :aria-pressed="filter === item.value"
          @click="filter = item.value"
        >
          {{ item.label }}
        </button>
      </div>

      <p v-if="error" class="data-message error sqlcheck-message">{{ error }}</p>
      <p
        v-if="result && result.objectCheck === 'unavailable'"
        class="data-message warning sqlcheck-message"
      >
        对象检查不可用：当前库 / 模式还没有表与字段元数据，本次只完成词法检查和危险语句检查。
      </p>
      <p
        v-if="result?.truncated"
        class="data-message warning sqlcheck-message"
      >
        问题过多，仅显示前 200 条；请缩小选区后重新检查。
      </p>

      <div v-if="filtered.length" class="sqlcheck-list" role="list">
        <button
          v-for="issue in filtered"
          :key="`${issue.code}:${issue.from}:${issue.message}`"
          class="sqlcheck-item"
          role="listitem"
          :title="`${issue.line}:${issue.column} · ${issue.code}`"
          @click="jump(issue)"
        >
          <span class="sqlcheck-severity" :class="issue.severity">
            {{ severityLabel(issue.severity) }}
          </span>
          <span class="sqlcheck-position">{{ issue.line }}:{{ issue.column }}</span>
          <span class="sqlcheck-text">{{ issue.message }}</span>
          <code class="sqlcheck-code">{{ issue.code }}</code>
        </button>
      </div>
      <div
        v-else-if="!error && !checking"
        class="panel-empty sqlcheck-empty"
      >
        {{
          result && result.issues.length
            ? "当前筛选级别下没有问题。"
            : "未发现高置信度问题。"
        }}
      </div>

      <p class="sqlcheck-disclaimer">
        静态检查基于当前元数据，可能漏报 / 误报，执行结果以数据库为准；检查不会执行 SQL、不会修改编辑器内容，也不会阻止运行。
      </p>

      <footer class="modal-actions">
        <button
          class="button ghost"
          :disabled="!hasIssues"
          @click="copyIssues"
        >
          <Icon name="copy" :size="13" />
          复制问题
        </button>
        <span class="spacer"></span>
        <button class="button ghost" @click="emit('close')">关闭</button>
        <button
          class="button primary"
          :disabled="checking"
          @click="emit('recheck')"
        >
          <Icon name="refresh" :size="13" />
          {{ checking ? "检查中…" : "重新检查" }}
        </button>
      </footer>
    </div>
  </div>
</template>
