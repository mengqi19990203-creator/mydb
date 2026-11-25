<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { ErRelation, ErTable } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  scopeLabel?: string;
}>();

const emit = defineEmits<{
  select: [table: { name: string; kind: string }];
}>();

interface ErNode extends ErTable {
  x: number;
  y: number;
  width: number;
  height: number;
}

const NODE_WIDTH = 240;
const HEADER_HEIGHT = 34;
const ROW_HEIGHT = 22;
const NODE_PADDING = 10;
const LAYOUT_KEY = "mydb.erLayout.v1";

const diagram = ref<{ tables: ErTable[]; relations: ErRelation[] } | null>(
  null,
);
const nodes = ref<ErNode[]>([]);
const loading = ref(false);
const error = ref("");
const zoom = ref(1);
const zoomLabel = computed(() => `${Math.round(zoom.value * 100)}%`);
const pan = reactive({ x: 0, y: 0 });
const canvasRef = ref<HTMLElement | null>(null);
const search = ref("");

const searchMatches = computed(() => {
  const query = search.value.trim().toLowerCase();
  if (!query) return new Set<string>();
  return new Set(
    nodes.value
      .filter((node) => node.name.toLowerCase().includes(query))
      .map((node) => node.name),
  );
});

let dragging: {
  node: ErNode;
  offsetX: number;
  offsetY: number;
  startX: number;
  startY: number;
  moved: boolean;
} | null = null;
let panning: {
  startX: number;
  startY: number;
  panX: number;
  panY: number;
} | null = null;

async function load() {
  loading.value = true;
  error.value = "";
  try {
    const data = await api.getErDiagram(props.connectionId);
    diagram.value = { tables: data.tables, relations: data.relations };
    layout(data.tables, data.relations);
    await nextTick();
    fitView();
  } catch (reason) {
    diagram.value = null;
    nodes.value = [];
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

watch(() => props.connectionId, load, { immediate: true });

function layoutKey(): string {
  return `${props.connectionId}:${props.scopeLabel ?? ""}`;
}

function readSavedLayout(): Record<string, { x: number; y: number }> {
  try {
    const stored = window.localStorage.getItem(LAYOUT_KEY);
    if (!stored) return {};
    const parsed = JSON.parse(stored) as Record<
      string,
      Record<string, { x: number; y: number }>
    >;
    return parsed[layoutKey()] ?? {};
  } catch {
    return {};
  }
}

function saveLayout() {
  try {
    const stored = window.localStorage.getItem(LAYOUT_KEY);
    const parsed = stored
      ? (JSON.parse(stored) as Record<
          string,
          Record<string, { x: number; y: number }>
        >)
      : {};
    parsed[layoutKey()] = Object.fromEntries(
      nodes.value.map((node) => [node.name, { x: node.x, y: node.y }]),
    );
    window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(parsed));
  } catch {
    // Layout persistence is best-effort.
  }
}

function resetLayout() {
  try {
    const stored = window.localStorage.getItem(LAYOUT_KEY);
    if (stored) {
      const parsed = JSON.parse(stored) as Record<string, unknown>;
      delete parsed[layoutKey()];
      window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(parsed));
    }
  } catch {
    // Ignore storage failures.
  }
  if (diagram.value) {
    layout(diagram.value.tables, diagram.value.relations);
    void nextTick(fitView);
  }
}

function centerOnNode(node: ErNode) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  zoom.value = Math.max(zoom.value, 0.8);
  pan.x = canvas.clientWidth / 2 - node.x * zoom.value;
  pan.y = canvas.clientHeight / 2 - node.y * zoom.value;
}

function focusSearchMatch() {
  const match = nodes.value.find((node) => searchMatches.value.has(node.name));
  if (match) centerOnNode(match);
}

function xmlEscape(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function exportSvg() {
  if (!nodes.value.length) return;
  const margin = 40;
  const left = Math.min(
    ...nodes.value.map((node) => node.x - node.width / 2),
  ) - margin;
  const top = Math.min(
    ...nodes.value.map((node) => node.y - node.height / 2),
  ) - margin;
  const right = Math.max(
    ...nodes.value.map((node) => node.x + node.width / 2),
  ) + margin;
  const bottom = Math.max(
    ...nodes.value.map((node) => node.y + node.height / 2),
  ) + margin;
  const width = Math.max(1, right - left);
  const height = Math.max(1, bottom - top);
  const edgeMarkup = edges.value
    .map(
      (edge) =>
        `<path d="${edge.path}" fill="none" stroke="#6b7280" stroke-width="1.4" marker-end="url(#arrow)"/>`,
    )
    .join("\n");
  const nodeMarkup = nodes.value
    .map((node) => {
      const x = node.x - node.width / 2;
      const y = node.y - node.height / 2;
      const columns = node.columns
        .map((column, index) => {
          const name = xmlEscape(column.name);
          const type = xmlEscape(column.dataType);
          const pk = column.primaryKey ? "  PK" : "";
          return `<text x="${x + 10}" y="${
            y + HEADER_HEIGHT + 16 + index * ROW_HEIGHT
          }" fill="#d4d4d8" font-size="11" font-family="ui-monospace, monospace">${name}${pk}  <tspan fill="#8b8b91">${type}</tspan></text>`;
        })
        .join("\n");
      return `<g>
  <rect x="${x}" y="${y}" width="${node.width}" height="${node.height}" rx="8" fill="#1d1d21" stroke="${
    node.kind === "table" ? "#3f3f46" : "#71717a"
  }" stroke-dasharray="${node.kind === "table" ? "none" : "5 4"}"/>
  <rect x="${x}" y="${y}" width="${node.width}" height="${HEADER_HEIGHT}" rx="8" fill="#27272a"/>
  <text x="${x + 10}" y="${
    y + 22
  }" fill="#f4f4f5" font-size="12" font-weight="600" font-family="ui-sans-serif, system-ui">${xmlEscape(
    node.name,
  )}</text>
  ${columns}
</g>`;
    })
    .join("\n");
  const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${left} ${top} ${width} ${height}">
  <defs>
    <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#6b7280"/>
    </marker>
  </defs>
  <rect x="${left}" y="${top}" width="${width}" height="${height}" fill="#131316"/>
  ${edgeMarkup}
  ${nodeMarkup}
</svg>`;
  const path = await saveDialog({
    defaultPath: "er-diagram.svg",
    filters: [{ name: "SVG", extensions: ["svg"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, svg);
  } catch (reason) {
    error.value = String(reason);
  }
}

function layout(tables: ErTable[], relations: ErRelation[]) {
  const list: ErNode[] = tables.map((table) => ({
    ...table,
    x: 0,
    y: 0,
    width: NODE_WIDTH,
    height: HEADER_HEIGHT + table.columns.length * ROW_HEIGHT + NODE_PADDING,
  }));

  if (!list.length) {
    nodes.value = [];
    return;
  }

  const count = list.length;
  const radius = Math.max(240, count * 46);
  list.forEach((node, index) => {
    const angle = (2 * Math.PI * index) / count;
    node.x = Math.cos(angle) * radius;
    node.y = Math.sin(angle) * radius;
  });

  const byName = new Map(list.map((node) => [node.name, node]));
  const links = relations
    .map((relation) => ({
      from: byName.get(relation.fromTable),
      to: byName.get(relation.toTable),
    }))
    .filter(
      (link): link is { from: ErNode; to: ErNode } =>
        Boolean(link.from && link.to),
    );

  const iterations = 160;
  for (let step = 0; step < iterations; step += 1) {
    const cooling = 1 - step / iterations;

    for (let i = 0; i < list.length; i += 1) {
      for (let j = i + 1; j < list.length; j += 1) {
        const a = list[i];
        const b = list[j];
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let distance = Math.hypot(dx, dy);
        if (!distance) {
          dx = Math.random() - 0.5;
          dy = Math.random() - 0.5;
          distance = 1;
        }
        const minDistance = 300;
        if (distance < minDistance) {
          const force = ((minDistance - distance) / distance) * 0.35 * cooling;
          dx *= force;
          dy *= force;
          a.x += dx;
          a.y += dy;
          b.x -= dx;
          b.y -= dy;
        }
      }
    }

    for (const link of links) {
      const dx = link.to.x - link.from.x;
      const dy = link.to.y - link.from.y;
      const distance = Math.hypot(dx, dy) || 1;
      const target = 380;
      const force = ((distance - target) / distance) * 0.025 * cooling;
      const fx = dx * force;
      const fy = dy * force;
      link.from.x += fx;
      link.from.y += fy;
      link.to.x -= fx;
      link.to.y -= fy;
    }

    for (const node of list) {
      node.x *= 0.996;
      node.y *= 0.996;
    }
  }

  const minLeft = Math.min(...list.map((node) => node.x - node.width / 2));
  const minTop = Math.min(...list.map((node) => node.y - node.height / 2));
  for (const node of list) {
    node.x = node.x - minLeft + node.width / 2 + 60;
    node.y = node.y - minTop + node.height / 2 + 60;
  }
  const saved = readSavedLayout();
  for (const node of list) {
    const position = saved[node.name];
    if (position) {
      node.x = position.x;
      node.y = position.y;
    }
  }
  nodes.value = list;
}

function fitView() {
  const canvas = canvasRef.value;
  if (!canvas || canvas.clientWidth < 1 || !nodes.value.length) return;

  const minLeft = Math.min(...nodes.value.map((node) => node.x - node.width / 2));
  const minTop = Math.min(...nodes.value.map((node) => node.y - node.height / 2));
  const maxRight = Math.max(...nodes.value.map((node) => node.x + node.width / 2));
  const maxBottom = Math.max(
    ...nodes.value.map((node) => node.y + node.height / 2),
  );
  const width = Math.max(1, maxRight - minLeft);
  const height = Math.max(1, maxBottom - minTop);

  const scale = Math.min(
    (canvas.clientWidth - 80) / width,
    (canvas.clientHeight - 80) / height,
    1.25,
  );
  zoom.value = Math.max(0.2, Math.min(1.25, scale));
  pan.x = (canvas.clientWidth - width * zoom.value) / 2 - minLeft * zoom.value;
  pan.y = (canvas.clientHeight - height * zoom.value) / 2 - minTop * zoom.value;
}

function relationPath(from: ErNode, to: ErNode): string {
  const fromLeft = from.x - from.width / 2;
  const fromRight = from.x + from.width / 2;
  const toLeft = to.x - to.width / 2;
  const toRight = to.x + to.width / 2;
  const toTheRight = to.x >= from.x;
  const startX = toTheRight ? fromRight : fromLeft;
  const endX = toTheRight ? toLeft : toRight;
  const direction = endX >= startX ? 1 : -1;
  const curve = Math.max(50, Math.abs(endX - startX) * 0.45);
  return `M ${startX} ${from.y} C ${startX + direction * curve} ${from.y}, ${endX - direction * curve} ${to.y}, ${endX} ${to.y}`;
}

const objectCounts = computed(() => ({
  tables: nodes.value.filter((node) => node.kind === "table").length,
  views: nodes.value.filter((node) => node.kind !== "table").length,
}));

const edges = computed(() => {
  if (!diagram.value) return [];
  const byName = new Map(nodes.value.map((node) => [node.name, node]));
  return diagram.value.relations.flatMap((relation) => {
    const from = byName.get(relation.fromTable);
    const to = byName.get(relation.toTable);
    if (!from || !to) return [];
    return [
      {
        key: `${relation.name}|${relation.fromTable}|${relation.toTable}`,
        path: relationPath(from, to),
        title: `${relation.fromTable}(${relation.fromColumns.join(
          ", ",
        )}) → ${relation.toTable}(${relation.toColumns.join(", ")})`,
      },
    ];
  });
});

function onWheel(event: WheelEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const pointerX = event.clientX - rect.left;
  const pointerY = event.clientY - rect.top;
  const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
  applyZoom(factor, pointerX, pointerY);
}

function zoomBy(factor: number) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  applyZoom(factor, canvas.clientWidth / 2, canvas.clientHeight / 2);
}

function applyZoom(factor: number, anchorX: number, anchorY: number) {
  const next = Math.max(0.2, Math.min(2.5, zoom.value * factor));
  const worldX = (anchorX - pan.x) / zoom.value;
  const worldY = (anchorY - pan.y) / zoom.value;
  zoom.value = next;
  pan.x = anchorX - worldX * next;
  pan.y = anchorY - worldY * next;
}

function startPan(event: PointerEvent) {
  if (event.button !== 0) return;
  panning = {
    startX: event.clientX,
    startY: event.clientY,
    panX: pan.x,
    panY: pan.y,
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function movePan(event: PointerEvent) {
  if (!panning) return;
  pan.x = panning.panX + event.clientX - panning.startX;
  pan.y = panning.panY + event.clientY - panning.startY;
}

function endPan() {
  panning = null;
}

function startNodeDrag(event: PointerEvent, node: ErNode) {
  if (event.button !== 0) return;
  event.stopPropagation();
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const worldX = (event.clientX - rect.left - pan.x) / zoom.value;
  const worldY = (event.clientY - rect.top - pan.y) / zoom.value;
  dragging = {
    node,
    offsetX: worldX - node.x,
    offsetY: worldY - node.y,
    startX: event.clientX,
    startY: event.clientY,
    moved: false,
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function moveNode(event: PointerEvent) {
  if (!dragging) return;
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const worldX = (event.clientX - rect.left - pan.x) / zoom.value;
  const worldY = (event.clientY - rect.top - pan.y) / zoom.value;
  dragging.node.x = worldX - dragging.offsetX;
  dragging.node.y = worldY - dragging.offsetY;
  if (
    Math.abs(event.clientX - dragging.startX) +
      Math.abs(event.clientY - dragging.startY) >
    3
  ) {
    dragging.moved = true;
  }
}

function endNodeDrag(event: PointerEvent, node: ErNode) {
  const current = dragging;
  dragging = null;
  const target = event.currentTarget;
  if (
    target instanceof HTMLElement &&
    target.hasPointerCapture(event.pointerId)
  ) {
    target.releasePointerCapture(event.pointerId);
  }
  if (current && !current.moved) {
    emit("select", { name: node.name, kind: node.kind });
  } else if (current?.moved) {
    saveLayout();
  }
}
</script>

<template>
  <section class="er-workspace">
    <header class="data-toolbar">
      <div class="data-title">
        <strong>ER 图</strong>
        <span v-if="scopeLabel" class="data-context">{{ scopeLabel }}</span>
        <span class="hint">
          {{ objectCounts.tables }} 表
          <template v-if="objectCounts.views"> · {{ objectCounts.views }} 视图</template>
          · {{ edges.length }} 关系
        </span>
      </div>
      <div class="data-actions">
        <input
          v-model="search"
          class="filter-input er-search"
          placeholder="搜索表 / 视图…"
          @keydown.enter.prevent="focusSearchMatch"
        />
        <button
          class="button ghost compact"
          :disabled="!searchMatches.size"
          @click="focusSearchMatch"
        >
          定位
        </button>
        <button
          class="button ghost compact"
          :disabled="!nodes.length"
          @click="resetLayout"
        >
          重置布局
        </button>
        <button
          class="button ghost compact"
          :disabled="!nodes.length"
          @click="exportSvg"
        >
          <Icon name="download" :size="14" />
          导出 SVG
        </button>
        <button class="button ghost compact" :disabled="loading" @click="load">
          <Icon name="refresh" :size="14" />
          刷新
        </button>
        <button
          class="button ghost compact"
          :disabled="!nodes.length"
          @click="fitView"
        >
          适应窗口
        </button>
        <span class="er-zoom">{{ zoomLabel }}</span>
        <button
          class="icon-button"
          title="放大"
          aria-label="放大"
          @click="zoomBy(1.15)"
        >
          <Icon name="plus" :size="14" />
        </button>
        <button
          class="icon-button"
          title="缩小"
          aria-label="缩小"
          @click="zoomBy(1 / 1.15)"
        >
          <Icon name="minus" :size="14" />
        </button>
      </div>
    </header>

    <div ref="canvasRef" class="er-canvas" @wheel.prevent="onWheel">
      <div
        class="er-background"
        @pointerdown="startPan"
        @pointermove="movePan"
        @pointerup="endPan"
        @pointercancel="endPan"
      ></div>

      <div
        class="er-world"
        :style="{
          transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
        }"
      >
        <svg class="er-edges" aria-hidden="true">
          <defs>
            <marker
              id="er-arrow"
              viewBox="0 0 10 10"
              refX="9"
              refY="5"
              markerWidth="7"
              markerHeight="7"
              orient="auto"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" />
            </marker>
          </defs>
          <path
            v-for="edge in edges"
            :key="edge.key"
            class="er-edge"
            :d="edge.path"
            marker-end="url(#er-arrow)"
          >
            <title>{{ edge.title }}</title>
          </path>
        </svg>

        <div
          v-for="node in nodes"
          :key="node.name"
          class="er-node"
          :class="[
            `er-node-${node.kind}`,
            {
              'search-match': searchMatches.has(node.name),
              'search-dim':
                searchMatches.size > 0 && !searchMatches.has(node.name),
            },
          ]"
          :style="{
            left: `${node.x - node.width / 2}px`,
            top: `${node.y - node.height / 2}px`,
            width: `${node.width}px`,
          }"
          @pointerdown="startNodeDrag($event, node)"
          @pointermove="moveNode"
          @pointerup="endNodeDrag($event, node)"
          @pointercancel="endNodeDrag($event, node)"
        >
          <div class="er-node-head">
            <Icon
              :name="node.kind === 'table' ? 'table' : 'view'"
              :size="13"
            />
            <span class="er-node-name" :title="node.name">{{ node.name }}</span>
            <span v-if="node.kind !== 'table'" class="er-kind-badge">
              {{ node.kind === "matview" ? "物化视图" : "视图" }}
            </span>
          </div>
          <div class="er-node-columns">
            <div
              v-for="column in node.columns"
              :key="column.name"
              class="er-column"
            >
              <span class="er-column-name" :title="column.name">
                {{ column.name }}
              </span>
              <span v-if="column.primaryKey" class="pk-badge">PK</span>
              <span class="er-column-type" :title="column.dataType">
                {{ column.dataType }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div v-if="loading" class="panel-empty">正在读取表结构…</div>
      <div v-else-if="error" class="data-message error">{{ error }}</div>
      <div v-else-if="!nodes.length" class="panel-empty">
        当前范围没有可展示的表或视图
      </div>

      <div class="er-hint">
        拖拽表移动 · 点击打开数据 · 滚轮缩放 · 拖动空白平移
      </div>
    </div>
  </section>
</template>