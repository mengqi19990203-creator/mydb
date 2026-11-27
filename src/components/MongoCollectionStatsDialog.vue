<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import * as api from "../lib/api";
import type { MongoCollectionStats } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  database: string;
  collection: string;
  kind?: string;
}>();

const emit = defineEmits<{ close: [] }>();

const stats = ref<MongoCollectionStats | null>(null);
const loading = ref(false);
const error = ref("");
const showAllIndexes = ref(false);

function formatBytes(value: number | null): string {
  if (value === null || value === undefined) return "—";
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(1)} ${units[index]}`;
}

function formatNumber(value: number | null): string {
  return value === null || value === undefined ? "—" : value.toLocaleString("zh-CN");
}

const visibleIndexes = computed(() => {
  const list = stats.value?.indexSizes ?? [];
  return showAllIndexes.value ? list : list.slice(0, 20);
});

async function load() {
  if (!props.open || !props.connectionId || !props.database || !props.collection) return;
  loading.value = true;
  error.value = "";
  try {
    stats.value = await api.mongoCollectionStats(
      props.connectionId,
      props.database,
      props.collection,
    );
    showAllIndexes.value = false;
  } catch (reason) {
    stats.value = null;
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function copyJson() {
  if (!stats.value) return;
  const text = JSON.stringify(stats.value, null, 2);
  if (text.length > 1_000_000) {
    error.value = "统计 JSON 超过 1 MB，无法复制";
    return;
  }
  try {
    await writeClipboard(text);
    error.value = "";
  } catch (reason) {
    error.value = String(reason);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("close");
}

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));

watch(
  () => [props.open, props.connectionId, props.database, props.collection],
  ([open]) => {
    if (open) {
      document.addEventListener("keydown", onKeydown, true);
      void load();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal mongo-stats-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>集合统计</h2>
          <p>{{ database }}.{{ collection }} · {{ kind ?? stats?.kind ?? "collection" }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="mongo-stats-body">
        <div v-if="loading" class="mongo-stats-loading">读取集合统计…</div>
        <div v-else-if="error" class="mongo-stats-error">
          <p class="data-message warning">{{ error }}</p>
          <p class="hint">
            视图 / 时间序列集合可能不支持 collStats；请刷新对象列表或检查当前用户的权限。
          </p>
        </div>
        <template v-else-if="stats">
          <p class="hint">
            namespace：<code>{{ stats.namespace }}</code>
            <template v-if="stats.capped"> · capped collection</template>
            <template v-if="stats.readOnly"> · 服务器标记为 readOnly</template>
          </p>
          <div class="mongo-stats-grid">
            <div>
              <span>文档数（服务器统计）</span>
              <strong>{{ formatNumber(stats.count) }}</strong>
            </div>
            <div>
              <span>数据大小</span>
              <strong>{{ formatBytes(stats.size) }}</strong>
              <small v-if="stats.size !== null">{{ formatNumber(stats.size) }} B</small>
            </div>
            <div>
              <span>存储大小</span>
              <strong>{{ formatBytes(stats.storageSize) }}</strong>
              <small v-if="stats.storageSize !== null">{{ formatNumber(stats.storageSize) }} B</small>
            </div>
            <div>
              <span>平均文档大小</span>
              <strong>{{ stats.avgObjSize === null ? "—" : `${stats.avgObjSize.toFixed(1)} B` }}</strong>
            </div>
            <div>
              <span>索引数量</span>
              <strong>{{ formatNumber(stats.nindexes) }}</strong>
            </div>
            <div>
              <span>索引总大小</span>
              <strong>{{ formatBytes(stats.totalIndexSize) }}</strong>
              <small v-if="stats.totalIndexSize !== null">{{ formatNumber(stats.totalIndexSize) }} B</small>
            </div>
          </div>

          <section v-if="stats.timeseries" class="mongo-stats-section">
            <h3>时间序列</h3>
            <dl class="mongo-stats-dl">
              <dt>timeField</dt><dd>{{ stats.timeseries.timeField ?? "—" }}</dd>
              <dt>metaField</dt><dd>{{ stats.timeseries.metaField ?? "—" }}</dd>
              <dt>granularity</dt><dd>{{ stats.timeseries.granularity ?? "—" }}</dd>
            </dl>
          </section>

          <section class="mongo-stats-section">
            <h3>索引大小（{{ stats.indexSizes.length }}）</h3>
            <p v-if="!stats.indexSizes.length" class="hint">服务器未返回索引大小。</p>
            <table v-else class="mongo-stats-table">
              <thead><tr><th>索引</th><th>大小</th><th>原始字节</th></tr></thead>
              <tbody>
                <tr v-for="index in visibleIndexes" :key="index.name">
                  <td><code>{{ index.name }}</code></td>
                  <td>{{ formatBytes(index.size) }}</td>
                  <td>{{ formatNumber(index.size) }}</td>
                </tr>
              </tbody>
            </table>
            <button
              v-if="stats.indexSizes.length > 20 && !showAllIndexes"
              class="button ghost compact"
              @click="showAllIndexes = true"
            >
              显示全部 {{ stats.indexSizes.length }} 个索引
            </button>
          </section>

          <section v-if="stats.validator" class="mongo-stats-section">
            <h3>validator</h3>
            <pre class="mongo-stats-json">{{ JSON.stringify(stats.validator, null, 2) }}</pre>
          </section>
        </template>
        <p v-else class="hint">暂无统计数据。</p>
      </div>

      <footer class="modal-actions">
        <button class="button ghost" :disabled="loading" @click="load">
          <Icon name="refresh" :size="13" /> 刷新
        </button>
        <button class="button ghost" :disabled="!stats" @click="copyJson">
          复制 JSON
        </button>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>
    </div>
  </div>
</template>
