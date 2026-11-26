<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type {
  ColumnInfo,
  Driver,
  ImportOptions,
  ImportPreview,
  ImportProgress,
  ImportSummary,
  TableInfo,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  initialTable?: string;
  tables?: TableInfo[];
  readOnly: boolean;
}>();

const emit = defineEmits<{
  close: [];
  imported: [table: string, count: number];
}>();

const table = ref("");
const columns = ref<ColumnInfo[]>([]);
const loadingTable = ref(false);
const path = ref("");
const format = ref<"csv" | "json" | "xlsx">("csv");
const preview = ref<ImportPreview | null>(null);
const previewing = ref(false);
const targets = ref<Record<string, string>>({});
const options = ref<ImportOptions>({
  delimiter: null,
  hasHeader: true,
  errorMode: "rollback",
  emptyAsNull: true,
  sheet: null,
});
const error = ref("");
const executing = ref(false);
const operationId = ref("");
const progress = ref<ImportProgress | null>(null);
const summary = ref<ImportSummary | null>(null);
const cancelling = ref(false);
let unlisten: UnlistenFn | undefined;

const tableOptions = computed(() =>
  (props.tables ?? []).filter((item) => item.kind === "table"),
);
const fileName = computed(() => path.value.split(/[\\/]/).pop() ?? "");
const duplicateTargets = computed(() => {
  const counts = new Map<string, number>();
  for (const target of Object.values(targets.value)) {
    if (target) counts.set(target, (counts.get(target) ?? 0) + 1);
  }
  return [...counts.entries()]
    .filter(([, count]) => count > 1)
    .map(([target]) => target);
});
const requiredUnmapped = computed(() =>
  columns.value
    .filter(
      (column) =>
        !column.autoIncrement &&
        !column.nullable &&
        column.defaultValue === null,
    )
    .filter(
      (column) => !Object.values(targets.value).includes(column.name),
    )
    .map((column) => column.name),
);
const activeMappingCount = computed(
  () => Object.values(targets.value).filter(Boolean).length,
);
const canImport = computed(
  () =>
    Boolean(table.value) &&
    Boolean(preview.value) &&
    !props.readOnly &&
    !executing.value &&
    activeMappingCount.value > 0 &&
    !duplicateTargets.value.length &&
    !requiredUnmapped.value.length,
);
const progressPercent = computed(() => {
  const current = progress.value;
  if (!current || !current.total) return 0;
  return Math.min(100, Math.round((current.processed / current.total) * 100));
});

function resetMapping() {
  const next: Record<string, string> = {};
  const names = new Set(columns.value.map((column) => column.name));
  for (const source of preview.value?.columns ?? []) {
    const match =
      [...names].find((name) => name === source) ??
      [...names].find((name) => name.toLowerCase() === source.toLowerCase()) ??
      "";
    next[source] = match;
  }
  targets.value = next;
}

async function loadTable() {
  if (!table.value) return;
  loadingTable.value = true;
  error.value = "";
  try {
    const structure = await api.getTableStructure(
      props.connectionId,
      table.value,
    );
    columns.value = structure.columns;
    resetMapping();
  } catch (reason) {
    error.value = String(reason);
    columns.value = [];
  } finally {
    loadingTable.value = false;
  }
}

async function chooseFile() {
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [
      { name: "CSV / JSON / Excel", extensions: ["csv", "json", "xlsx"] },
    ],
  });
  if (typeof selected !== "string") return;
  path.value = selected;
  const lower = selected.toLowerCase();
  format.value = lower.endsWith(".xlsx")
    ? "xlsx"
    : lower.endsWith(".json")
      ? "json"
      : "csv";
  options.value.sheet = null;
  summary.value = null;
  await runPreview();
}

async function runPreview() {
  if (!path.value || previewing.value || executing.value) return;
  previewing.value = true;
  error.value = "";
  try {
    preview.value = await api.previewImportFile(
      path.value,
      format.value,
      options.value,
    );
    if (format.value === "xlsx") {
      options.value.sheet = preview.value.sheet ?? null;
    }
    if (format.value === "csv" && !options.value.delimiter) {
      const detected = preview.value.detectedDelimiter;
      if ([",", ";", "\\t", "|"].includes(detected)) {
        options.value.delimiter = detected === "\\t" ? "tab" : detected;
      }
    }
    resetMapping();
  } catch (reason) {
    preview.value = null;
    error.value = String(reason);
  } finally {
    previewing.value = false;
  }
}

async function changeOption() {
  if (!path.value) return;
  await runPreview();
}

async function execute() {
  if (!canImport.value) return;
  executing.value = true;
  error.value = "";
  summary.value = null;
  cancelling.value = false;
  progress.value = null;
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `import-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  operationId.value = id;
  try {
    const result = await api.executeImportFile(
      props.connectionId,
      table.value,
      path.value,
      format.value,
      options.value,
      (preview.value?.columns ?? []).map((source) => ({
        source,
        target: targets.value[source] || null,
      })),
      id,
    );
    summary.value = result;
    if (result.inserted > 0) {
      emit("imported", table.value, result.inserted);
    }
  } catch (reason) {
    error.value = String(reason);
  } finally {
    executing.value = false;
    operationId.value = "";
    cancelling.value = false;
  }
}

async function cancel() {
  if (!operationId.value || cancelling.value) return;
  cancelling.value = true;
  try {
    await api.cancelImport(operationId.value);
  } catch (reason) {
    error.value = String(reason);
    cancelling.value = false;
  }
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape" && !executing.value) {
    event.preventDefault();
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      table.value =
        props.initialTable ??
        tableOptions.value[0]?.name ??
        "";
      columns.value = [];
      path.value = "";
      format.value = "csv";
      preview.value = null;
      targets.value = {};
      options.value = {
        delimiter: null,
        hasHeader: true,
        errorMode: "rollback",
        emptyAsNull: true,
        sheet: null,
      };
      error.value = "";
      executing.value = false;
      progress.value = null;
      summary.value = null;
      cancelling.value = false;
      void loadTable();
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

watch(table, () => {
  preview.value = null;
  targets.value = {};
  void loadTable();
});

onMounted(async () => {
  unlisten = await listen<ImportProgress>(
    "import:progress",
    (event) => {
      if (!operationId.value) return;
      if (event.payload.operationId !== operationId.value) return;
      progress.value = event.payload;
    },
  );
});

onBeforeUnmount(() => {
  unlisten?.();
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="!executing && emit('close')">
    <div class="modal import-wizard" role="dialog" aria-modal="true" aria-label="导入数据">
      <header class="modal-header">
        <div>
          <h2>导入数据{{ table ? ` · ${table}` : "" }}</h2>
          <p>
            预览 → 列映射 → 选项 → 执行；CSV 仅支持 UTF-8，Excel
            读取缓存值且不执行公式，导入过程按行写入。
          </p>
        </div>
        <button class="icon-button" title="关闭" :disabled="executing" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="import-toolbar">
        <label v-if="tableOptions.length" class="import-field">
          目标表
          <select v-model="table" :disabled="executing">
            <option v-for="item in tableOptions" :key="item.name" :value="item.name">
              {{ item.name }}
            </option>
          </select>
        </label>
        <span class="import-field">
          文件
          <button class="button ghost compact" :disabled="executing" @click="chooseFile">
            <Icon name="upload" :size="13" />
            {{ fileName || "选择 CSV / JSON / Excel…" }}
          </button>
        </span>
        <label
          v-if="format === 'xlsx' && preview?.sheets?.length"
          class="import-field"
        >
          工作表
          <select
            v-model="options.sheet"
            :disabled="executing || previewing"
            @change="changeOption"
          >
            <option
              v-for="name in preview?.sheets ?? []"
              :key="name"
              :value="name"
            >
              {{ name }}
            </option>
          </select>
        </label>
        <label v-if="format === 'csv'" class="import-field">
          分隔符
          <select v-model="options.delimiter" :disabled="executing" @change="changeOption">
            <option :value="null">自动</option>
            <option value=",">逗号 ,</option>
            <option value=";">分号 ;</option>
            <option value="tab">制表符 Tab</option>
            <option value="|">竖线 |</option>
          </select>
        </label>
        <label v-if="format !== 'json'" class="import-check">
          <input
            v-model="options.hasHeader"
            type="checkbox"
            :disabled="executing"
            @change="changeOption"
          />
          首行是表头
        </label>
        <span class="spacer"></span>
        <button
          class="button ghost compact"
          :disabled="!path || previewing || executing"
          @click="runPreview"
        >
          {{ previewing ? "预览中…" : "重新预览" }}
        </button>
        <button class="button primary compact" :disabled="!canImport" @click="execute">
          {{ executing ? "导入中…" : "开始导入" }}
        </button>
      </div>

      <p v-if="readOnly" class="data-message error import-message">当前连接为只读，不能导入数据。</p>
      <p v-if="error" class="data-message error import-message">{{ error }}</p>

      <div v-if="loadingTable" class="panel-empty">正在读取表结构…</div>
      <div v-else-if="!table" class="panel-empty">请选择目标表。</div>
      <div v-else-if="!preview" class="panel-empty">
        选择一个 CSV / JSON / Excel 文件后显示前 20 行预览。
      </div>

      <div v-else class="import-body">
        <section class="import-mapping">
          <div class="import-section-head">
            <h3>列映射</h3>
            <span class="hint">
              文件 {{ preview.columns.length }} 列 · 映射 {{ activeMappingCount }} 列
              <template v-if="preview.format === 'csv'">
                · 检测分隔符 {{ preview.detectedDelimiter || "—" }}
              </template>
              <template v-else-if="preview.format === 'xlsx'">
                · Excel<template v-if="preview.sheet"> · {{ preview.sheet }}</template>
              </template>
            </span>
          </div>
          <div class="import-map-list">
            <div v-for="source in preview.columns" :key="source" class="import-map-row">
              <code>{{ source }}</code>
              <span>→</span>
              <select v-model="targets[source]" :disabled="executing">
                <option value="">忽略此列</option>
                <option v-for="column in columns" :key="column.name" :value="column.name">
                  {{ column.name }} · {{ column.dataType }}
                  {{ column.nullable ? "" : " · NOT NULL" }}
                  <template v-if="column.autoIncrement"> · 自增</template>
                </option>
              </select>
            </div>
          </div>
          <p v-if="duplicateTargets.length" class="import-warning">
            字段被重复映射：{{ duplicateTargets.join("、") }}
          </p>
          <p v-if="requiredUnmapped.length" class="import-warning">
            必填字段尚未映射：{{ requiredUnmapped.join("、") }}
          </p>
        </section>

        <section class="import-preview">
          <div class="import-section-head">
            <h3>文件预览（前 20 行）</h3>
          </div>
          <div class="import-preview-grid">
            <table class="structure-table">
              <thead>
                <tr>
                  <th v-for="column in preview.columns" :key="column">{{ column }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(row, rowIndex) in preview.rows" :key="rowIndex">
                  <td v-for="(value, index) in row" :key="index">{{ value }}</td>
                </tr>
              </tbody>
            </table>
          </div>

          <div class="import-options">
            <h3>导入选项</h3>
            <label class="import-check">
              <input v-model="options.emptyAsNull" type="checkbox" :disabled="executing" />
              空字符串写入 NULL（关闭则写入空字符串）
            </label>
            <div class="import-error-modes">
              <label>
                <input v-model="options.errorMode" type="radio" value="rollback" :disabled="executing" />
                全部回滚（单事务）
              </label>
              <label>
                <input v-model="options.errorMode" type="radio" value="stop" :disabled="executing" />
                遇错停止并保留已导入
              </label>
              <label>
                <input v-model="options.errorMode" type="radio" value="skip" :disabled="executing" />
                跳过错误行继续
              </label>
            </div>
            <p class="hint">
              「全部回滚」在写入前一次性校验；回滚失败不会留下部分数据。「停止 / 跳过」逐行写入。
            </p>
          </div>
        </section>
      </div>

      <div v-if="executing" class="import-progress">
        <div class="import-progress-bar"><span :style="{ width: `${progressPercent}%` }"></span></div>
        <span>
          {{ cancelling ? "正在取消…" : "正在导入" }}
          {{ progress ? `${progress.processed} / ${progress.total ?? "?"} 行` : "" }}
          · 成功 {{ progress?.inserted ?? 0 }} · 失败 {{ progress?.failed ?? 0 }}
        </span>
        <button class="button ghost compact danger" :disabled="cancelling" @click="cancel">
          {{ cancelling ? "取消中…" : "取消" }}
        </button>
      </div>

      <section v-if="summary" class="import-summary">
        <h3>导入结果</h3>
        <div class="import-summary-grid">
          <div><span>成功</span><strong>{{ summary.inserted }}</strong></div>
          <div><span>失败</span><strong>{{ summary.failed }}</strong></div>
          <div><span>跳过</span><strong>{{ summary.skipped }}</strong></div>
          <div><span>耗时</span><strong>{{ summary.elapsedMs }} ms</strong></div>
          <div><span>回滚</span><strong>{{ summary.rolledBack ? "是" : "否" }}</strong></div>
          <div><span>取消</span><strong>{{ summary.cancelled ? "是" : "否" }}</strong></div>
        </div>
        <p v-if="summary.rolledBack" class="import-warning">
          导入失败，已整体回滚；数据库没有写入本次导入的数据。
        </p>
        <div v-if="summary.failures.length" class="import-failures">
          <div v-for="failure in summary.failures" :key="`${failure.row}-${failure.message}`">
            第 {{ failure.row }} 行：{{ failure.message }}
          </div>
        </div>
        <p v-if="!summary.failures.length && summary.inserted" class="hint">
          已刷新表数据；可关闭对话框继续查看。
        </p>
      </section>

      <div class="confirm-actions">
        <button class="button ghost" :disabled="executing" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.import-wizard {
  width: min(1100px, calc(100vw - 40px));
  max-height: calc(100vh - 40px);
  display: flex;
  flex-direction: column;
}

.import-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.import-field {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 11px;
  max-width: 520px;
}

.import-field select {
  max-width: 220px;
  padding: 4px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 11px;
}

.import-check {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 11px;
}

.import-message {
  margin: 8px 20px 0;
}

.import-body {
  display: grid;
  grid-template-columns: minmax(280px, 0.8fr) minmax(0, 1.2fr);
  gap: 14px;
  min-height: 0;
  overflow: auto;
  padding: 10px 20px;
}

.import-mapping,
.import-preview {
  min-width: 0;
}

.import-section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}

.import-section-head h3,
.import-options h3 {
  margin: 0;
  font-size: 11px;
  font-weight: 600;
}

.import-map-list {
  max-height: 360px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--panel-2);
}

.import-map-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 10px;
}

.import-map-row code {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.import-map-row select {
  max-width: 60%;
  padding: 3px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.import-warning {
  margin: 6px 0 0;
  padding: 5px 7px;
  border-radius: 5px;
  background: rgba(210, 153, 34, 0.1);
  color: var(--warning, #d29922);
  font-size: 10px;
}

.import-preview-grid {
  max-height: 230px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
}

.import-preview-grid table {
  width: max-content;
  min-width: 100%;
  font-size: 10px;
}

.import-preview-grid th,
.import-preview-grid td {
  max-width: 180px;
  padding: 4px 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.import-options {
  margin-top: 12px;
  padding: 8px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--panel-2);
}

.import-options h3 {
  margin-bottom: 7px;
}

.import-error-modes {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 7px;
  color: var(--text-secondary);
  font-size: 11px;
}

.import-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 20px;
  padding: 8px 10px;
  border-radius: 7px;
  background: var(--accent-soft);
  color: var(--text-secondary);
  font-size: 11px;
}

.import-progress-bar {
  flex: 1;
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-3);
}

.import-progress-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.2s ease;
}

.import-summary {
  margin: 10px 20px 0;
  padding: 8px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--panel-2);
}

.import-summary h3 {
  margin: 0 0 6px;
  font-size: 11px;
}

.import-summary-grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 6px;
}

.import-summary-grid div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 5px 7px;
  border-radius: 5px;
  background: var(--surface-well);
}

.import-summary-grid span {
  color: var(--faint);
  font-size: 9px;
}

.import-summary-grid strong {
  font-size: 12px;
}

.import-failures {
  max-height: 150px;
  overflow: auto;
  margin-top: 6px;
  padding: 6px 8px;
  border-radius: 5px;
  background: var(--danger-soft);
  color: var(--danger);
  font-size: 10px;
  line-height: 1.5;
}
</style>