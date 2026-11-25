<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  buildPreviewSql,
  defaultGenerator,
  GENERATOR_OPTIONS,
  visibleGeneratorColumns,
} from "../lib/data-generator";
import type {
  ColumnInfo,
  Driver,
  GeneratePreview,
  GenerateProgress,
  GenerateRequest,
  GeneratorColumn,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  table: string;
  tableKind?: string;
  driver: Driver;
  readOnly: boolean;
}>();

const emit = defineEmits<{
  close: [];
  generated: [count: number];
}>();

const columns = ref<ColumnInfo[]>([]);
const configs = ref<GeneratorColumn[]>([]);
const count = ref(100);
const seed = ref(1);
const batchSize = ref(500);
const loading = ref(false);
const generating = ref(false);
const previewing = ref(false);
const error = ref("");
const notice = ref("");
const preview = ref<GeneratePreview | null>(null);
const operationId = ref("");
const progress = ref<GenerateProgress | null>(null);
const cancelled = ref(false);
let unlisten: UnlistenFn | undefined;

const isTable = computed(() => (props.tableKind ?? "table") === "table");
const enabledCount = computed(
  () => configs.value.filter((config) => config.enabled).length,
);
const previewSql = computed(() => {
  if (!preview.value) return "";
  return buildPreviewSql(
    props.table,
    props.driver,
    preview.value.columns,
    preview.value.rows,
  );
});
const progressPercent = computed(() => {
  const current = progress.value;
  if (!current || !current.total) return 0;
  return Math.min(100, Math.round((current.generated / current.total) * 100));
});

function buildRequest(): GenerateRequest {
  return {
    table: props.table,
    columns: configs.value,
    count: Math.max(1, Math.min(10_000, Math.floor(count.value))),
    seed: Math.max(1, Math.floor(seed.value)),
    batchSize: Math.max(100, Math.min(1_000, Math.floor(batchSize.value))),
  };
}

async function load() {
  loading.value = true;
  error.value = "";
  notice.value = "";
  preview.value = null;
  progress.value = null;
  cancelled.value = false;
  try {
    const structure = await api.getTableStructure(
      props.connectionId,
      props.table,
    );
    columns.value = structure.columns;
    configs.value = visibleGeneratorColumns(structure.columns).map(
      defaultGenerator,
    );
    if (!configs.value.length) {
      error.value = "这张表没有可生成数据的列";
      return;
    }
    await runPreview();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function runPreview() {
  if (previewing.value || generating.value) return;
  previewing.value = true;
  error.value = "";
  try {
    preview.value = await api.previewGeneratedData(
      props.connectionId,
      buildRequest(),
    );
    notice.value = "预览不会写入数据库。";
  } catch (reason) {
    preview.value = null;
    error.value = String(reason);
  } finally {
    previewing.value = false;
  }
}

async function generate() {
  if (generating.value || props.readOnly) return;
  generating.value = true;
  error.value = "";
  notice.value = "";
  cancelled.value = false;
  progress.value = null;
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `generate-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  operationId.value = id;
  try {
    const summary = await api.generateTableData(props.connectionId, {
      ...buildRequest(),
      operationId: id,
    });
    if (summary.cancelled) {
      notice.value = `已取消：生成 ${summary.generated} 行，未写入任何数据。`;
      return;
    }
    notice.value = `已写入 ${summary.inserted} 行，耗时 ${summary.elapsedMs} ms。`;
    emit("generated", summary.inserted);
    emit("close");
  } catch (reason) {
    error.value = String(reason);
  } finally {
    generating.value = false;
    operationId.value = "";
  }
}

async function cancel() {
  if (!operationId.value || cancelled.value) return;
  cancelled.value = true;
  try {
    await api.cancelGenerate(operationId.value);
    notice.value = "正在取消，当前批次结束后回滚…";
  } catch (reason) {
    error.value = String(reason);
  }
}

function randomSeed() {
  seed.value = Math.floor(Math.random() * 1_000_000_000) + 1;
}

function setGenerator(config: GeneratorColumn, value: string) {
  config.generator = value;
  if (value === "date" && config.dateFrom.includes(" ")) {
    config.dateFrom = "2024-01-01";
    config.dateTo = "2026-12-31";
  }
  if (value === "datetime" && !config.dateFrom.includes(" ")) {
    config.dateFrom = "2024-01-01 00:00:00";
    config.dateTo = "2026-12-31 23:59:59";
  }
}

function typeOf(name: string): ColumnInfo | undefined {
  return columns.value.find((column) => column.name === name);
}

function columnLabel(config: GeneratorColumn): string {
  const column = typeOf(config.name);
  return column ? `${config.name} · ${column.dataType}` : config.name;
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      configs.value = [];
      preview.value = null;
      error.value = "";
      count.value = 100;
      seed.value = 1;
      batchSize.value = 500;
      void load();
    }
  },
  { immediate: true },
);

onMounted(async () => {
  unlisten = await listen<GenerateProgress>(
    "generate:progress",
    (event) => {
      if (!operationId.value) return;
      if (event.payload.operationId !== operationId.value) return;
      progress.value = event.payload;
    },
  );
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="!generating && emit('close')">
    <div class="modal generate-modal" role="dialog" aria-modal="true" aria-label="生成测试数据">
      <header class="modal-header">
        <div>
          <h2>生成测试数据 · {{ table }}</h2>
          <p>按列选择生成器，先预览，再在一个事务中写入；不会读取表中现有数据。</p>
        </div>
        <button class="icon-button" title="关闭" :disabled="generating" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <p v-if="readOnly || !isTable" class="data-message error">
        {{ readOnly ? "当前连接为只读，不能生成数据。" : "只有表支持生成测试数据。" }}
      </p>

      <div class="generate-toolbar">
        <label>
          行数
          <input
            v-model.number="count"
            type="number"
            min="1"
            max="10000"
            :disabled="generating"
          />
        </label>
        <label>
          种子
          <input
            v-model.number="seed"
            type="number"
            min="1"
            :disabled="generating"
          />
          <button class="button ghost compact" type="button" :disabled="generating" @click="randomSeed">
            随机
          </button>
        </label>
        <label>
          批大小
          <input
            v-model.number="batchSize"
            type="number"
            min="100"
            max="1000"
            :disabled="generating"
          />
        </label>
        <span class="hint">相同种子会生成相同数据。</span>
        <span class="spacer"></span>
        <button class="button ghost compact" :disabled="previewing || generating" @click="runPreview">
          {{ previewing ? "预览中…" : "重新预览" }}
        </button>
      </div>

      <p v-if="error" class="message error generate-message">{{ error }}</p>
      <p v-else-if="notice" class="message success generate-message">{{ notice }}</p>

      <div v-if="loading" class="panel-empty">正在读取表结构…</div>
      <div v-else class="generate-body">
        <section class="generate-columns">
          <table class="structure-table generate-table">
            <thead>
              <tr>
                <th>启用</th>
                <th>列</th>
                <th>生成器</th>
                <th>参数</th>
                <th>NULL %</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="config in configs" :key="config.name">
                <td>
                  <input v-model="config.enabled" type="checkbox" :disabled="generating" />
                </td>
                <td class="mono">
                  {{ columnLabel(config) }}
                  <span v-if="typeOf(config.name)?.primaryKey" class="pk-badge">PK</span>
                  <span v-if="!typeOf(config.name)?.nullable" class="backup-sub">NOT NULL</span>
                </td>
                <td>
                  <select
                    :value="config.generator"
                    :disabled="generating || !config.enabled"
                    @change="setGenerator(config, ($event.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="option in GENERATOR_OPTIONS"
                      :key="option.value"
                      :value="option.value"
                      :disabled="
                        (option.value === 'null' && !typeOf(config.name)?.nullable) ||
                        (['date', 'datetime'].includes(option.value) &&
                          typeOf(config.name)?.dataType.toUpperCase().includes('INT'))
                      "
                    >
                      {{ option.label }}
                    </option>
                  </select>
                </td>
                <td>
                  <div class="generate-params">
                    <template v-if="config.generator === 'sequence'">
                      <label>起始 <input v-model.number="config.sequenceStart" type="number" /></label>
                      <label>步长 <input v-model.number="config.sequenceStep" type="number" /></label>
                    </template>
                    <template v-else-if="config.generator === 'integer'">
                      <label>最小 <input v-model.number="config.minNumber" type="number" /></label>
                      <label>最大 <input v-model.number="config.maxNumber" type="number" /></label>
                    </template>
                    <template v-else-if="config.generator === 'decimal'">
                      <label>最小 <input v-model.number="config.minNumber" type="number" step="any" /></label>
                      <label>最大 <input v-model.number="config.maxNumber" type="number" step="any" /></label>
                      <label>小数位 <input v-model.number="config.decimals" type="number" min="0" max="6" /></label>
                    </template>
                    <template v-else-if="config.generator === 'text'">
                      <label>长度 <input v-model.number="config.textMin" type="number" min="0" /> – <input v-model.number="config.textMax" type="number" min="0" /></label>
                      <select v-model="config.charset">
                        <option value="alphanumeric">字母 + 数字</option>
                        <option value="alpha">字母</option>
                        <option value="numeric">数字</option>
                      </select>
                    </template>
                    <input
                      v-else-if="config.generator === 'regex'"
                      v-model="config.regex"
                      class="generate-wide-input"
                      placeholder="例如 [A-Z]{2}-\d{4}"
                    />
                    <template v-else-if="config.generator === 'date' || config.generator === 'datetime'">
                      <input v-model="config.dateFrom" :type="config.generator === 'date' ? 'date' : 'datetime-local'" />
                      <span>–</span>
                      <input v-model="config.dateTo" :type="config.generator === 'date' ? 'date' : 'datetime-local'" />
                    </template>
                    <template v-else-if="config.generator === 'boolean'">
                      <label>true 比例 <input v-model.number="config.boolPercent" type="number" min="0" max="100" /></label>
                    </template>
                    <input
                      v-else-if="config.generator === 'fixed'"
                      v-model="config.fixedValue"
                      class="generate-wide-input"
                      placeholder="固定值"
                    />
                    <span v-else class="hint">无需参数</span>
                  </div>
                </td>
                <td>
                  <input
                    v-model.number="config.nullPercent"
                    type="number"
                    min="0"
                    max="100"
                    :disabled="!typeOf(config.name)?.nullable || generating"
                  />
                </td>
              </tr>
            </tbody>
          </table>
          <p v-if="!configs.length" class="panel-empty">没有可生成的列。</p>
        </section>

        <section class="generate-preview">
          <div class="generate-preview-head">
            <h3>预览前 20 行</h3>
            <span class="hint">{{ enabledCount }} / {{ configs.length }} 列启用</span>
          </div>
          <p v-if="preview?.warnings.length" class="generate-warning">
            {{ preview.warnings.join("；") }}
          </p>
          <div v-if="preview && preview.rows.length" class="generate-preview-grid">
            <table class="structure-table">
              <thead>
                <tr>
                  <th v-for="column in preview.columns" :key="column">{{ column }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(row, rowIndex) in preview.rows" :key="rowIndex">
                  <td v-for="(value, index) in row" :key="index">
                    {{ value === null ? "NULL" : String(value) }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <p v-else class="panel-empty compact">点击「重新预览」查看前 20 行。</p>

          <h3 class="generate-sql-title">INSERT 预览</h3>
          <pre class="generate-sql">{{ previewSql || "—" }}</pre>
        </section>
      </div>

      <div v-if="generating" class="generate-progress">
        <div class="generate-progress-bar">
          <span :style="{ width: `${progressPercent}%` }"></span>
        </div>
        <span>
          {{ cancelled ? "正在取消…" : progress?.phase === "inserting" ? "正在写入…" : "正在生成…" }}
          {{ progress ? `${progress.generated} / ${progress.total} 行` : "" }}
        </span>
        <button class="button ghost compact danger" :disabled="cancelled" @click="cancel">
          {{ cancelled ? "取消中…" : "取消" }}
        </button>
      </div>

      <div class="confirm-actions">
        <button class="button ghost" :disabled="generating" @click="emit('close')">关闭</button>
        <button
          class="button primary"
          :disabled="readOnly || !isTable || generating || previewing || !enabledCount"
          @click="generate"
        >
          {{ generating ? "生成中…" : `生成 ${count} 行` }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.generate-modal {
  width: min(1180px, calc(100vw - 40px));
  max-height: calc(100vh - 40px);
  display: flex;
  flex-direction: column;
}

.generate-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.generate-toolbar label {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 11px;
}

.generate-toolbar input {
  width: 90px;
  padding: 4px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 12px;
}

.generate-message {
  margin: 8px 20px 0;
}

.generate-body {
  min-height: 0;
  overflow: auto;
  padding: 10px 20px;
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(0, 0.75fr);
  gap: 14px;
}

.generate-columns {
  min-width: 0;
  overflow: auto;
}

.generate-table {
  width: 100%;
}

.generate-table td {
  vertical-align: top;
}

.generate-params {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
  max-width: 360px;
}

.generate-params label {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--faint);
  font-size: 10px;
}

.generate-params input,
.generate-params select {
  min-width: 54px;
  max-width: 120px;
  padding: 3px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-well);
  color: var(--text);
  font-size: 10px;
}

.generate-params input[type="date"],
.generate-params input[type="datetime-local"] {
  max-width: 170px;
}

.generate-wide-input {
  min-width: 220px;
  flex: 1;
}

.generate-preview {
  min-width: 0;
  max-height: 480px;
  overflow: auto;
  padding: 8px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.generate-preview-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.generate-preview h3,
.generate-sql-title {
  margin: 0 0 6px;
  font-size: 11px;
  font-weight: 600;
}

.generate-preview-grid {
  max-height: 240px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
}

.generate-preview-grid table {
  width: max-content;
  min-width: 100%;
  font-size: 10px;
}

.generate-preview-grid th,
.generate-preview-grid td {
  max-width: 180px;
  padding: 4px 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.generate-warning {
  margin: 0 0 6px;
  color: var(--warning, #d29922);
  font-size: 10px;
}

.generate-sql {
  max-height: 200px;
  overflow: auto;
  margin: 0;
  padding: 8px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 9px;
  line-height: 1.5;
  white-space: pre-wrap;
}

.generate-progress {
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

.generate-progress-bar {
  flex: 1;
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-3);
}

.generate-progress-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.2s ease;
}
</style>