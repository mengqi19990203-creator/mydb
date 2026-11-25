<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import {
  defaultExpandedIds,
  flattenExplain,
  parseExplainResult,
  type ExplainNode,
} from "../lib/explain-plan";
import type { ExplainResult } from "../lib/types";

const props = defineProps<{
  result: ExplainResult | null;
  error: string | null;
  loading: boolean;
}>();

const view = ref<"tree" | "raw">("tree");
const expanded = ref<Set<string>>(new Set());
const selectedId = ref("");
const notice = ref("");

const parsed = computed(() =>
  props.result ? parseExplainResult(props.result) : null,
);
const flatNodes = computed(() =>
  parsed.value ? flattenExplain(parsed.value.roots, expanded.value) : [],
);
const selectedNode = computed<ExplainNode | null>(() => {
  const find = (nodes: ExplainNode[]): ExplainNode | null => {
    for (const node of nodes) {
      if (node.id === selectedId.value) return node;
      const child = find(node.children);
      if (child) return child;
    }
    return null;
  };
  return parsed.value ? find(parsed.value.roots) : null;
});

watch(
  parsed,
  (value) => {
    if (!value) {
      expanded.value = new Set();
      selectedId.value = "";
      return;
    }
    expanded.value = defaultExpandedIds(value.roots);
    selectedId.value =
      value.roots[0]?.id ?? "";
    view.value = "tree";
  },
  { immediate: true },
);

function toggle(node: ExplainNode) {
  const next = new Set(expanded.value);
  if (next.has(node.id)) next.delete(node.id);
  else next.add(node.id);
  expanded.value = next;
  selectedId.value = node.id;
}

function selectNode(node: ExplainNode) {
  selectedId.value = node.id;
}

function moveSelection(delta: number) {
  const list = flatNodes.value;
  if (!list.length) return;
  const index = list.findIndex((item) => item.node.id === selectedId.value);
  const next = Math.max(0, Math.min(list.length - 1, index + delta));
  selectedId.value = list[next].node.id;
}

function expandSelection(expand: boolean) {
  const node = selectedNode.value;
  if (!node) return;
  const next = new Set(expanded.value);
  if (expand) next.add(node.id);
  else next.delete(node.id);
  expanded.value = next;
}

function onKeydown(event: KeyboardEvent) {
  if (view.value !== "tree") return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    expandSelection(true);
  } else if (event.key === "ArrowLeft") {
    event.preventDefault();
    expandSelection(false);
  }
}

async function copyRaw() {
  if (!parsed.value) return;
  try {
    await writeClipboard(parsed.value.raw);
    notice.value = "已复制原始计划";
    window.setTimeout(() => (notice.value = ""), 1800);
  } catch (reason) {
    notice.value = String(reason);
  }
}

async function copyDetail() {
  const node = selectedNode.value;
  if (!node) return;
  const text = node.detail.map((item) => `${item.key}: ${item.value}`).join("\n");
  try {
    await writeClipboard(text);
    notice.value = "已复制节点详情";
    window.setTimeout(() => (notice.value = ""), 1800);
  } catch (reason) {
    notice.value = String(reason);
  }
}
</script>

<template>
  <div class="explain-viewer" tabindex="0" @keydown="onKeydown">
    <div v-if="loading" class="panel-empty">正在生成执行计划…</div>
    <div v-else-if="error" class="data-message error">
      {{ error }}
      <span v-if="result" class="hint">（以下保留上一次计划）</span>
    </div>

    <template v-if="parsed">
      <div class="explain-summary">
        <span class="explain-chip">{{ result?.driver }}</span>
        <span class="explain-chip">{{ parsed.format }}</span>
        <span class="explain-chip">{{ parsed.summary.nodeCount }} 节点</span>
        <span class="explain-chip">最高成本 {{ parsed.summary.topCost }}</span>
        <span class="explain-chip">最高行数 {{ parsed.summary.topRows }}</span>
        <span v-if="parsed.note" class="hint">{{ parsed.note }}</span>
      </div>
      <div class="explain-toolbar">
        <div class="segmented">
          <button
            class="segment"
            :class="{ active: view === 'tree' }"
            @click="view = 'tree'"
          >
            计划树
          </button>
          <button
            class="segment"
            :class="{ active: view === 'raw' }"
            @click="view = 'raw'"
          >
            原始计划
          </button>
        </div>
        <button class="button ghost compact" @click="copyRaw">
          <Icon name="copy" :size="13" />
          复制原始计划
        </button>
        <span v-if="notice" class="hint">{{ notice }}</span>
      </div>

      <div v-if="view === 'tree'" class="explain-main">
        <div class="explain-tree">
          <div
            v-for="item in flatNodes"
            :key="item.node.id"
            class="explain-node"
            :class="{ selected: item.node.id === selectedId }"
            :style="{ paddingLeft: `${8 + item.depth * 16}px` }"
            @click="selectNode(item.node)"
            @dblclick="toggle(item.node)"
          >
            <button
              v-if="item.node.children.length"
              class="explain-toggle"
              :aria-label="expanded.has(item.node.id) ? '折叠' : '展开'"
              @click.stop="toggle(item.node)"
            >
              <Icon
                :name="expanded.has(item.node.id) ? 'chevron-down' : 'chevron-right'"
                :size="11"
              />
            </button>
            <span v-else class="explain-toggle-placeholder"></span>
            <span class="explain-node-label">{{ item.node.label }}</span>
            <span v-if="item.node.index" class="explain-badge">
              {{ item.node.index }}
            </span>
            <span class="explain-metrics">
              <span v-if="item.node.cost !== '—'">成本 {{ item.node.cost }}</span>
              <span v-if="item.node.rows !== '—'">行 {{ item.node.rows }}</span>
            </span>
          </div>
        </div>
        <div class="explain-detail">
          <div class="explain-detail-head">
            <strong>{{ selectedNode?.operation ?? "节点详情" }}</strong>
            <button
              class="button ghost compact"
              :disabled="!selectedNode"
              @click="copyDetail"
            >
              复制详情
            </button>
          </div>
          <dl v-if="selectedNode" class="explain-detail-list">
            <template v-for="item in selectedNode.detail" :key="item.key">
              <dt>{{ item.key }}</dt>
              <dd>{{ item.value }}</dd>
            </template>
          </dl>
          <div v-else class="panel-empty compact">选择左侧节点查看详情</div>
        </div>
      </div>
      <pre v-else class="ddl-preview explain-raw">{{ parsed.raw || "（空计划）" }}</pre>
    </template>
  </div>
</template>

<style scoped>
.explain-viewer {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  outline: none;
}

.explain-summary,
.explain-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-subtle);
}

.explain-chip,
.explain-badge {
  padding: 2px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  color: var(--muted);
  font-size: 10px;
}

.explain-main {
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(260px, 0.85fr);
  min-height: 0;
  flex: 1;
}

.explain-tree {
  overflow: auto;
  border-right: 1px solid var(--border-subtle);
  padding: 6px 0;
}

.explain-node {
  display: flex;
  align-items: center;
  gap: 5px;
  min-height: 26px;
  padding-right: 10px;
  cursor: default;
  font-size: 11px;
}

.explain-node:hover {
  background: var(--hover);
}

.explain-node.selected {
  background: var(--accent-soft);
}

.explain-toggle {
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--muted);
}

.explain-toggle-placeholder {
  width: 16px;
}

.explain-node-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

.explain-metrics {
  margin-left: auto;
  display: flex;
  gap: 8px;
  color: var(--muted);
  font-size: 10px;
  white-space: nowrap;
}

.explain-detail {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: auto;
  padding: 8px 10px;
}

.explain-detail-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.explain-detail-list {
  display: grid;
  grid-template-columns: minmax(110px, 0.5fr) minmax(0, 1.5fr);
  gap: 4px 10px;
  margin: 0;
  font-size: 11px;
}

.explain-detail-list dt {
  color: var(--muted);
}

.explain-detail-list dd {
  margin: 0;
  overflow-wrap: anywhere;
  font-family: var(--font-mono);
}

.explain-raw {
  flex: 1;
  min-height: 0;
  margin: 0;
  border-radius: 0;
  overflow: auto;
}
</style>