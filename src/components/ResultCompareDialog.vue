<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import { formatGridCell } from "../lib/grid-search";
import {
  compareResultSets,
  compareRowsTsv,
  compareStatusLabel,
  compareSummaryLines,
  compareSummaryMarkdown,
  chooseDefaultKey,
  type CompareOutcome,
  type CompareSource,
} from "../lib/result-compare";

const props = defineProps<{
  open: boolean;
  sources: CompareSource[];
  initialBaselineId: string;
  initialTargetId: string;
  pinnedOnly?: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const baselineId = ref("");
const targetId = ref("");
const keyMode = ref("auto");
const onlyDifferences = ref(true);
const copied = ref("");
const modalRef = ref<HTMLElement | null>(null);
let copyTimer = 0;
let previousFocus: HTMLElement | null = null;

const frozenSources = ref<CompareSource[]>([]);
const sources = computed(() =>
  frozenSources.value.length ? frozenSources.value : props.sources,
);

const baseline = computed(
  () => sources.value.find((source) => source.id === baselineId.value) ?? null,
);
const target = computed(
  () => sources.value.find((source) => source.id === targetId.value) ?? null,
);
const baselineOptions = computed(() =>
  sources.value.filter(
    (source) => !props.pinnedOnly || source.kind === "pinned",
  ),
);
const targetOptions = computed(() =>
  sources.value.filter(
    (source) =>
      source.id !== baselineId.value &&
      (!props.pinnedOnly || source.kind === "pinned"),
  ),
);
const structure = computed(() =>
  baseline.value && target.value
    ? compareResultSets(baseline.value.result, target.value.result, null).structure
    : null,
);
const autoKey = computed(() =>
  baseline.value && target.value && structure.value
    ? chooseDefaultKey(
        baseline.value.result,
        target.value.result,
        structure.value.common,
      )
    : null,
);
const effectiveKey = computed(() => {
  if (keyMode.value === "row") return null;
  if (keyMode.value.startsWith("col:")) {
    return keyMode.value.slice(4);
  }
  return autoKey.value?.key ?? null;
});
const outcome = computed<CompareOutcome | null>(() =>
  baseline.value && target.value
    ? compareResultSets(
        baseline.value.result,
        target.value.result,
        effectiveKey.value,
      )
    : null,
);
const visibleRows = computed(() => {
  if (!outcome.value) return [];
  return onlyDifferences.value
    ? outcome.value.rows.filter((row) => row.status !== "same")
    : outcome.value.rows;
});
const common = computed(() => outcome.value?.structure.common ?? []);
const targetColumnIndex = computed(() => {
  const map = new Map<string, number>();
  for (const column of outcome.value?.structure.target ?? []) {
    map.set(column.key, column.index);
  }
  return map;
});
const summary = computed(() => outcome.value?.summary ?? null);
const canCopy = computed(() =>
  Boolean(outcome.value && baseline.value && target.value),
);
const missingSource = computed(
  () => Boolean(baselineId.value && targetId.value && (!baseline.value || !target.value)),
);

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      window.removeEventListener("keydown", onKeydown);
      const focus = previousFocus;
      previousFocus = null;
      if (focus?.isConnected) void nextTick(() => focus.focus());
      return;
    }
    previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    frozenSources.value = props.sources.map((source) => ({ ...source }));
    const initial = frozenSources.value.filter(
      (source) => !props.pinnedOnly || source.kind === "pinned",
    );
    baselineId.value =
      initial.find((source) => source.id === props.initialBaselineId)?.id ??
      initial[0]?.id ??
      "";
    targetId.value =
      frozenSources.value.find((source) => source.id === props.initialTargetId)?.id ??
      initial.find((source) => source.id !== baselineId.value)?.id ??
      "";
    keyMode.value = "auto";
    onlyDifferences.value = true;
    copied.value = "";
    window.addEventListener("keydown", onKeydown);
    await nextTick();
    modalRef.value?.querySelector<HTMLButtonElement>(".compare-close")?.focus();
  },
  { immediate: true },
);

watch([baselineId, targetId], () => {
  keyMode.value = "auto";
});

watch(targetOptions, (options) => {
  if (!options.length) return;
  if (!options.some((source) => source.id === targetId.value)) {
    targetId.value = options[0].id;
  }
});

watch(
  () => props.sources,
  () => {
    if (!props.open) return;
    if (baselineId.value && !props.sources.some((item) => item.id === baselineId.value)) {
      baselineId.value = "";
      targetId.value = "";
    }
  },
);

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
    return;
  }
  if (event.key !== "Tab") return;
  const modal = modalRef.value;
  if (!modal) return;
  const items = Array.from(
    modal.querySelectorAll<HTMLElement>(
      'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  );
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

function columnLabel(key: string): string {
  return key.includes("#") ? key : key;
}

function cell(source: CompareSource, rowIndex: number | null, column: number): string {
  if (rowIndex === null) return "—";
  return formatGridCell(source.result.rows[rowIndex]?.[column]);
}

function changed(row: { changedColumns: string[] }, key: string): boolean {
  return row.changedColumns.includes(key);
}

function setCopied(text: string) {
  copied.value = text;
  window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copied.value = "";
  }, 2200);
}

async function copySummaryTsv() {
  if (!outcome.value || !baseline.value || !target.value) return;
  try {
    await writeClipboard(
      compareSummaryLines(
        outcome.value,
        baseline.value.name,
        target.value.name,
      ).join("\n"),
    );
    setCopied("已复制摘要（TSV）");
  } catch (reason) {
    setCopied(`复制失败：${reason}`);
  }
}

async function copySummaryMarkdown() {
  if (!outcome.value || !baseline.value || !target.value) return;
  try {
    await writeClipboard(
      compareSummaryMarkdown(
        outcome.value,
        baseline.value.name,
        target.value.name,
      ),
    );
    setCopied("已复制摘要（Markdown）");
  } catch (reason) {
    setCopied(`复制失败：${reason}`);
  }
}

async function copyRows() {
  if (!outcome.value || !baseline.value || !target.value) return;
  try {
    await writeClipboard(
      compareRowsTsv(
        outcome.value,
        baseline.value.result,
        target.value.result,
      ),
    );
    setCopied("已复制差异表（TSV）");
  } catch (reason) {
    setCopied(`复制失败：${reason}`);
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop compare-backdrop" @click.self="emit('close')">
    <div
      ref="modalRef"
      class="modal compare-modal"
      role="dialog"
      aria-modal="true"
      aria-label="结果对比"
    >
      <header class="compare-head">
        <div>
          <h2>结果对比</h2>
          <p class="hint">按显示值比较；NULL 与空串不同，数字与字符串显示相同则视为相同。</p>
        </div>
        <button
          class="icon-button compare-close"
          title="关闭"
          aria-label="关闭结果对比"
          @click="emit('close')"
        >
          <Icon name="close" />
        </button>
      </header>

      <div class="compare-controls">
        <label class="import-field">
          基准
          <select v-model="baselineId">
            <option
              v-for="source in baselineOptions"
              :key="source.id"
              :value="source.id"
            >
              {{ source.name }}（{{ source.result.rows.length }} 行）
            </option>
          </select>
        </label>
        <label class="import-field">
          目标
          <select v-model="targetId">
            <option
              v-for="source in targetOptions"
              :key="source.id"
              :value="source.id"
            >
              {{ source.name }}（{{ source.result.rows.length }} 行）
            </option>
          </select>
        </label>
        <label class="import-field">
          行键
          <select v-model="keyMode">
            <option value="auto">
              自动选择{{ autoKey ? `（${autoKey.name}）` : "（按行号）" }}
            </option>
            <option value="row">按行号对比（顺序敏感）</option>
            <option
              v-for="column in common"
              :key="column.key"
              :value="`col:${column.key}`"
            >
              {{ column.name }}{{ column.duplicateName ? `（${column.key}）` : "" }}
            </option>
          </select>
        </label>
        <label class="import-check">
          <input v-model="onlyDifferences" type="checkbox" />
          只看差异
        </label>
        <span class="spacer"></span>
        <span v-if="copied" class="hint">{{ copied }}</span>
      </div>

      <p v-if="missingSource" class="data-message warning compare-message">
        固定结果已取消或来源不可用，请关闭对话框后重新选择。
      </p>
      <p v-if="outcome?.keyError" class="data-message warning compare-message">
        {{ outcome.keyError }}
      </p>
      <p v-if="outcome?.rowMode === 'row'" class="data-message warning compare-message">
        当前按行号对比，结果顺序变化会产生假差异；建议选择唯一键列。
      </p>
      <p v-if="outcome?.truncated" class="data-message warning compare-message">
        任一侧超过 1,000 行，只比较前 1,000 行。
      </p>
      <p v-if="outcome?.structure.commonTruncated" class="data-message warning compare-message">
        共同列超过 200 个，只比较前 200 个共同列。
      </p>

      <div v-if="summary" class="compare-summary" aria-live="polite">
        <span>基准 {{ summary.baselineRows }} 行</span>
        <span>目标 {{ summary.targetRows }} 行</span>
        <span class="compare-added">新增 {{ summary.added }}</span>
        <span class="compare-removed">删除 {{ summary.removed }}</span>
        <span class="compare-changed">修改 {{ summary.changed }}</span>
        <span>未变 {{ summary.same }}</span>
        <span>共同列 {{ summary.commonColumns }}</span>
        <span>结构差异 {{ summary.structureDifferences }}</span>
      </div>

      <div v-if="outcome && summary" class="compare-structure">
        <span v-if="!outcome.structure.baselineOnly.length && !outcome.structure.targetOnly.length" class="hint">
          列结构一致
        </span>
        <span v-if="outcome.structure.baselineOnly.length" class="hint">
          仅基准有：{{ outcome.structure.baselineOnly.map((column) => columnLabel(column.key)).join("、") }}
        </span>
        <span v-if="outcome.structure.targetOnly.length" class="hint">
          仅目标有：{{ outcome.structure.targetOnly.map((column) => columnLabel(column.key)).join("、") }}
        </span>
      </div>

      <div v-if="outcome && common.length" class="compare-table-wrap">
        <table class="compare-table">
          <thead>
            <tr>
              <th rowspan="2">状态</th>
              <th rowspan="2">键</th>
              <th :colspan="common.length" class="compare-side-head">基准</th>
              <th :colspan="common.length" class="compare-side-head">目标</th>
            </tr>
            <tr>
              <th v-for="column in common" :key="`b-${column.key}`">
                {{ columnLabel(column.key) }}
              </th>
              <th v-for="column in common" :key="`t-${column.key}`">
                {{ columnLabel(column.key) }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="row in visibleRows"
              :key="`${row.status}:${row.key}:${row.baselineIndex}:${row.targetIndex}`"
              :class="`compare-row ${row.status}`"
            >
              <td>
                <span class="compare-status" :class="row.status">
                  {{ compareStatusLabel(row.status) }}
                </span>
              </td>
              <td class="mono">{{ row.key ?? "—" }}</td>
              <td
                v-for="column in common"
                :key="`bv-${column.key}`"
                :class="{ 'compare-cell-changed': changed(row, column.key) }"
              >
                {{ baseline ? cell(baseline, row.baselineIndex, column.index) : "—" }}
              </td>
              <td
                v-for="column in common"
                :key="`tv-${column.key}`"
                :class="{ 'compare-cell-changed': changed(row, column.key) }"
              >
                {{ target ? cell(target, row.targetIndex, targetColumnIndex.get(column.key) ?? column.index) : "—" }}
              </td>
            </tr>
          </tbody>
        </table>
        <p v-if="!visibleRows.length" class="panel-empty compact">
          {{ onlyDifferences ? "没有差异行。" : "没有可显示的行。" }}
        </p>
      </div>
      <p
        v-else-if="outcome && !common.length"
        class="panel-empty compact"
      >
        两边没有共同列，只能查看结构差异。
      </p>

      <footer class="modal-actions compare-actions">
        <button
          class="button ghost"
          :disabled="!canCopy"
          @click="copySummaryTsv"
        >
          复制摘要 TSV
        </button>
        <button
          class="button ghost"
          :disabled="!canCopy"
          @click="copySummaryMarkdown"
        >
          复制摘要 Markdown
        </button>
        <button
          class="button ghost"
          :disabled="!canCopy || !common.length"
          @click="copyRows"
        >
          复制差异表 TSV
        </button>
        <span class="spacer"></span>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>
    </div>
  </div>
</template>
