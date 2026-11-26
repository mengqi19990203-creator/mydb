<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { readClipboard } from "../lib/clipboard";
import {
  CLIPBOARD_HARD_ROW_LIMIT,
  CLIPBOARD_ROW_LIMIT,
  CLIPBOARD_TEXT_LIMIT,
  autoMapColumns,
  buildClipboardTable,
  convertPasteRows,
  headerLooksLikeColumns,
  parseDelimitedText,
  type ParsedClipboardTable,
} from "../lib/clipboard-table";
import type { ColumnInfo } from "../lib/types";

const props = defineProps<{
  open: boolean;
  table: string;
  columns: ColumnInfo[];
  disabledReason: string;
  pendingInserts: number;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [
    drafts: Array<Array<unknown | undefined>>,
    skipped: number,
  ];
}>();

const loading = ref(false);
const error = ref("");
const text = ref("");
const delimiter = ref("\t");
const hasHeader = ref(true);
const emptyAsNull = ref(true);
const skipErrors = ref(false);
const mapping = ref<Record<string, string>>({});
const parsed = ref<ParsedClipboardTable | null>(null);

const previewRows = computed(() => parsed.value?.rows.slice(0, 20) ?? []);
const conversions = computed(() => {
  if (!parsed.value) {
    return { drafts: [], errors: [], skipped: 0 };
  }
  return convertPasteRows(
    parsed.value,
    mapping.value,
    props.columns,
    emptyAsNull.value,
  );
});
const newPendingCount = computed(
  () => props.pendingInserts + conversions.value.drafts.length,
);
const duplicateTargets = computed(() => {
  const counts = new Map<string, number>();
  for (const target of Object.values(mapping.value)) {
    if (target) counts.set(target, (counts.get(target) ?? 0) + 1);
  }
  return [...counts.entries()]
    .filter(([, count]) => count > 1)
    .map(([name]) => name);
});
const missingRequired = computed(() => {
  const mapped = new Set(Object.values(mapping.value).filter(Boolean));
  return props.columns
    .filter(
      (column) =>
        !column.autoIncrement &&
        !column.nullable &&
        column.defaultValue === null &&
        !mapped.has(column.name),
    )
    .map((column) => column.name);
});
const confirmDisabled = computed(
  () =>
    Boolean(props.disabledReason) ||
    !parsed.value ||
    !conversions.value.drafts.length ||
    duplicateTargets.value.length > 0 ||
    missingRequired.value.length > 0 ||
    (!skipErrors.value && conversions.value.errors.length > 0) ||
    newPendingCount.value > 10_000,
);
const confirmReason = computed(() => {
  if (props.disabledReason) return props.disabledReason;
  if (!parsed.value) return "没有可粘贴的数据";
  if (duplicateTargets.value.length) {
    return `字段被重复映射：${duplicateTargets.value.join("、")}`;
  }
  if (missingRequired.value.length) {
    return `必填字段未映射：${missingRequired.value.join("、")}`;
  }
  if (conversions.value.errors.length && !skipErrors.value) {
    return "存在无法转换的行，请勾选「跳过错误行」或修正剪贴板";
  }
  if (newPendingCount.value > 10_000) {
    return "待提交新增行超过 10,000 行";
  }
  if (!conversions.value.drafts.length) return "没有可加入的完整行";
  return "";
});

function reparse(detectHeader = false) {
  if (!text.value) return;
  try {
    error.value = "";
    const raw = parseDelimitedText(text.value, delimiter.value);
    if (raw.length > CLIPBOARD_HARD_ROW_LIMIT) {
      throw new Error("剪贴板超过 20,000 行，请改用文件导入");
    }
    const detected = headerLooksLikeColumns(raw[0] ?? [], props.columns);
    if (detectHeader) hasHeader.value = detected;
    const next = buildClipboardTable(
      text.value,
      delimiter.value,
      hasHeader.value,
      props.columns,
    );
    if (next.rowCount > CLIPBOARD_ROW_LIMIT) {
      throw new Error("剪贴板超过 5,000 行，请改用文件导入");
    }
    parsed.value = next;
    mapping.value = autoMapColumns(next.columns, props.columns);
    skipErrors.value = false;
  } catch (reason) {
    parsed.value = null;
    mapping.value = {};
    error.value = String(reason);
  }
}

async function readText() {
  loading.value = true;
  error.value = "";
  parsed.value = null;
  mapping.value = {};
  skipErrors.value = false;
  text.value = "";
  try {
    const content = await readClipboard();
    if (!content.trim()) {
      throw new Error("剪贴板没有可粘贴的文本");
    }
    if (content.length > CLIPBOARD_TEXT_LIMIT) {
      throw new Error("剪贴板超过 5 MB，请改用文件导入");
    }
    text.value = content;
    delimiter.value = content.includes("\t") ? "\t" : detectFallback(content);
    hasHeader.value = false;
    reparse(true);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function detectFallback(content: string): string {
  const line = content.split(/\r\n|\n|\r/)[0] ?? "";
  if (line.includes(",")) return ",";
  if (line.includes(";")) return ";";
  return "\t";
}

function close() {
  emit("close");
}

function confirm() {
  if (confirmDisabled.value) return;
  emit("confirm", conversions.value.drafts, conversions.value.skipped);
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
      void readText();
    } else {
      text.value = "";
      parsed.value = null;
      error.value = "";
    }
  },
  { immediate: true },
);

onMounted(() => {
  document.addEventListener("keydown", onKeydown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});

</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="close">
    <div
      class="modal prompt-modal paste-rows-modal"
      role="dialog"
      aria-modal="true"
      aria-label="粘贴新增行"
    >
      <header class="paste-rows-head">
        <div>
          <h2>粘贴新增行 · {{ table }}</h2>
          <p>
            从 Excel / 文本粘贴多行，先进入待提交；点击「提交」前不会写入数据库。
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="close">
          <Icon name="close" />
        </button>
      </header>

      <p v-if="disabledReason" class="data-message error import-message">
        {{ disabledReason }}
      </p>
      <p v-if="error" class="data-message error import-message">{{ error }}</p>
      <div v-if="loading" class="panel-empty">读取剪贴板…</div>

      <template v-else-if="parsed">
        <div class="paste-rows-toolbar">
          <label class="import-field">
            分隔符
            <select v-model="delimiter" @change="reparse()">
              <option :value="'\t'">制表符 Tab</option>
              <option value=",">逗号 ,</option>
              <option value=";">分号 ;</option>
            </select>
          </label>
          <label class="import-check">
            <input v-model="hasHeader" type="checkbox" @change="reparse()" />
            首行为表头
          </label>
          <label class="import-check">
            <input v-model="emptyAsNull" type="checkbox" />
            空单元格写入 NULL
          </label>
          <label class="import-check">
            <input v-model="skipErrors" type="checkbox" />
            跳过错误行
          </label>
          <span class="spacer"></span>
          <span class="hint">
            {{ parsed.rowCount }} 行 · {{ parsed.columns.length }} 列
            <template v-if="parsed.headerDetected">· 识别到表头</template>
          </span>
        </div>

        <div class="paste-rows-body">
          <section class="paste-rows-preview">
            <h3>预览（前 20 行）</h3>
            <div class="paste-preview-scroll">
              <table class="paste-preview-table">
                <thead>
                  <tr>
                    <th
                      v-for="column in parsed.columns"
                      :key="column"
                      :title="column"
                    >
                      {{ column }}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(row, index) in previewRows" :key="index">
                    <td
                      v-for="(cell, cellIndex) in row"
                      :key="cellIndex"
                      :title="cell"
                    >
                      {{ cell }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>

          <section class="paste-rows-mapping">
            <h3>列映射</h3>
            <div class="paste-map-list">
              <div
                v-for="source in parsed.columns"
                :key="source"
                class="paste-map-row"
              >
                <code>{{ source }}</code>
                <span>→</span>
                <select v-model="mapping[source]">
                  <option value="">忽略此列</option>
                  <option
                    v-for="column in columns"
                    :key="column.name"
                    :value="column.name"
                    :disabled="
                      column.autoIncrement ||
                      /(^|[^A-Z])(BLOB|BINARY|BYTEA|IMAGE)([^A-Z]|$)/i.test(
                        column.dataType,
                      )
                    "
                  >
                    {{ column.name }} · {{ column.dataType }}
                    <template v-if="column.autoIncrement">
                      · 自增（默认忽略）
                    </template>
                    <template v-else-if="column.nullable"> · 可空</template>
                  </option>
                </select>
              </div>
            </div>
            <p v-if="duplicateTargets.length" class="import-warning">
              字段被重复映射：{{ duplicateTargets.join("、") }}
            </p>
            <p v-if="missingRequired.length" class="import-warning">
              必填字段尚未映射：{{ missingRequired.join("、") }}
            </p>
          </section>
        </div>

        <div v-if="conversions.errors.length" class="paste-errors">
          <strong>无法转换的行（最多显示 20 条）</strong>
          <ul>
            <li
              v-for="item in conversions.errors.slice(0, 20)"
              :key="`${item.row}-${item.column}`"
            >
              第 {{ item.row }} 行 · {{ item.column }} · {{ item.message }}
            </li>
          </ul>
          <span v-if="conversions.errors.length > 20" class="hint">
            等共 {{ conversions.errors.length }} 条
          </span>
        </div>

        <div class="paste-rows-summary">
          <span>将新增 {{ conversions.drafts.length }} 行到待提交</span>
          <span v-if="conversions.skipped">，跳过 {{ conversions.skipped }} 行</span>
          <span class="hint">
            · 当前待提交新增 {{ pendingInserts }} 行
          </span>
        </div>
      </template>

      <footer class="modal-actions paste-rows-actions">
        <span v-if="confirmReason" class="hint">{{ confirmReason }}</span>
        <span class="spacer"></span>
        <button class="button ghost" @click="close">取消</button>
        <button
          class="button primary"
          :disabled="confirmDisabled"
          :title="confirmReason || undefined"
          @click="confirm"
        >
          加入待提交
        </button>
      </footer>
    </div>
  </div>
</template>