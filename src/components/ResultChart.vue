<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import {
  CHART_COLORS,
  buildChartSeries,
  chartAriaLabel,
  formatChartValue,
  niceTicksRange,
  numericColumns,
  type ChartAggregate,
  type ChartConfig,
  type ChartData,
  type ChartSort,
  type ChartType,
} from "../lib/result-chart";

const props = defineProps<{
  data: ChartData;
  datasetKey: string;
  tabTitle: string;
  filterSummary: string | null;
  filterCount: number;
}>();

const emit = defineEmits<{
  "adjust-filter": [];
}>();

const chartRef = ref<HTMLElement | null>(null);
const svgRef = ref<SVGSVGElement | null>(null);
const width = ref(640);
const showData = ref(false);
const exporting = ref(false);
const error = ref("");
let observer: ResizeObserver | null = null;

const config = ref<ChartConfig>(defaultConfig(props.data));

function defaultConfig(data: ChartData): ChartConfig {
  const numeric = numericColumns(data);
  const x =
    data.visibleColumns.find(
      (column) => !numeric.some((item) => item.index === column.index),
    ) ?? data.visibleColumns[0];
  const y = numeric[0] ?? null;
  return {
    type: "bar",
    xIndex: x?.index ?? -1,
    yIndex: y?.index ?? null,
    aggregate: y ? "sum" : "count",
    topN: 20,
    sort: "value-desc",
    showLabels: false,
    showLegend: true,
  };
}

let pendingReset = false;
let columnSignature = props.data.columns.join("\u0001");

watch(
  () => props.datasetKey,
  () => {
    // The grid publishes its new snapshot slightly after the result object
    // changes; defer the actual reset until that snapshot arrives.
    pendingReset = true;
    config.value = defaultConfig(props.data);
  },
);

watch(
  () => props.data,
  (data) => {
    const signature = data.columns.join("\u0001");
    if (pendingReset || signature !== columnSignature) {
      config.value = defaultConfig(data);
      pendingReset = false;
      columnSignature = signature;
      return;
    }
    const visible = new Set(
      data.visibleColumns.map((column) => column.index),
    );
    if (config.value.xIndex >= 0 && !visible.has(config.value.xIndex)) {
      config.value = { ...config.value, xIndex: defaultConfig(data).xIndex };
    }
    if (config.value.yIndex !== null && !visible.has(config.value.yIndex)) {
      config.value = { ...config.value, yIndex: null, aggregate: "count" };
    }
  },
);

const numericOptions = computed(() => numericColumns(props.data));
const series = computed(() => buildChartSeries(props.data, config.value));
const points = computed(() => series.value.points);
const domainMin = computed(() =>
  Math.min(0, ...points.value.map((point) => point.value)),
);
const domainMax = computed(() =>
  Math.max(0, ...points.value.map((point) => point.value)),
);
const hasNegative = computed(() =>
  points.value.some((point) => point.value < 0),
);
const ticks = computed(() => niceTicksRange(domainMin.value, domainMax.value));
const ariaLabel = computed(() =>
  chartAriaLabel(
    config.value.type,
    config.value,
    props.data,
    points.value.length,
  ),
);
const chartHeight = 300;
const margin = { top: 18, right: 18, bottom: 52, left: 58 };
const innerWidth = computed(() => Math.max(120, width.value - margin.left - margin.right));
const innerHeight = chartHeight - margin.top - margin.bottom;
const scaleMin = computed(() => Math.min(...ticks.value));
const scaleMax = computed(() =>
  Math.max(...ticks.value) === scaleMin.value
    ? scaleMin.value + 1
    : Math.max(...ticks.value),
);
const yFor = (value: number) =>
  margin.top +
  innerHeight -
  ((value - scaleMin.value) / (scaleMax.value - scaleMin.value)) *
    innerHeight;
const baselineY = computed(() => yFor(0));
const barWidth = computed(() =>
  points.value.length
    ? Math.max(3, (innerWidth.value / points.value.length) * 0.66)
    : 0,
);
const xFor = (index: number) =>
  margin.left +
  (points.value.length
    ? ((index + 0.5) * innerWidth.value) / points.value.length
    : innerWidth.value / 2);
const linePoints = computed(() =>
  points.value
    .map((point, index) => `${xFor(index)},${yFor(point.value)}`)
    .join(" "),
);
const pieSlices = computed(() => {
  const total = points.value.reduce(
    (sum, point) => sum + Math.abs(point.value),
    0,
  );
  let start = -Math.PI / 2;
  return points.value.map((point, index) => {
    const angle = total ? (Math.abs(point.value) / total) * Math.PI * 2 : 0;
    const end = start + angle;
    const cx = width.value / 2;
    const cy = margin.top + innerHeight / 2;
    const radius = Math.min(innerWidth.value, innerHeight) / 2 - 8;
    const large = angle > Math.PI ? 1 : 0;
    const path =
      angle >= Math.PI * 2 - 0.0001
        ? `M ${cx - radius} ${cy} A ${radius} ${radius} 0 1 0 ${cx + radius} ${cy} A ${radius} ${radius} 0 1 0 ${cx - radius} ${cy}`
        : `M ${cx} ${cy} L ${cx + radius * Math.cos(start)} ${
            cy + radius * Math.sin(start)
          } A ${radius} ${radius} 0 ${large} 1 ${
            cx + radius * Math.cos(end)
          } ${cy + radius * Math.sin(end)} Z`;
    const mid = (start + end) / 2;
    start = end;
    return {
      point,
      path,
      mid,
      color: CHART_COLORS[index % CHART_COLORS.length],
    };
  });
});
const showEmpty = computed(
  () =>
    !props.data.rows.length ||
    !props.data.visibleColumns.length ||
    !points.value.length ||
    (config.value.aggregate !== "count" && config.value.yIndex === null),
);

function setType(type: ChartType) {
  config.value = {
    ...config.value,
    type,
    sort: type === "line" ? "source" : "value-desc",
  };
}

function setY(value: string) {
  const index = Number(value);
  if (index < 0) {
    config.value = {
      ...config.value,
      yIndex: null,
      aggregate: "count",
    };
  } else {
    config.value = {
      ...config.value,
      yIndex: index,
      aggregate:
        config.value.aggregate === "count" ? "sum" : config.value.aggregate,
    };
  }
}

function update<K extends keyof ChartConfig>(key: K, value: ChartConfig[K]) {
  config.value = { ...config.value, [key]: value };
}

function labelFor(pointLabel: string): string {
  return pointLabel.length > 12 ? `${pointLabel.slice(0, 11)}…` : pointLabel;
}

function exportFileStem(): string {
  const raw = (props.tabTitle || "查询结果")
    .replace(/[\\/:*?"<>|]+/g, " ")
    .trim()
    .slice(0, 40);
  const stamp = new Date()
    .toISOString()
    .replace(/[:T]/g, "-")
    .replace(/\..*$/, "");
  return `mydb-${raw || "查询结果"}-${stamp}`;
}

function themeColor(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

async function exportSvg() {
  if (!svgRef.value || exporting.value) return;
  const path = await saveDialog({
    defaultPath: `${exportFileStem()}.svg`,
    filters: [{ name: "SVG", extensions: ["svg"] }],
  });
  if (typeof path !== "string") return;
  exporting.value = true;
  error.value = "";
  try {
    // Clone the live SVG and embed the theme colors it currently inherits so
    // the exported file is readable outside the app (no external CSS).
    const clone = svgRef.value.cloneNode(true) as SVGSVGElement;
    clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    const panel = themeColor("--panel", "#1c1c1f");
    const border = themeColor("--border-subtle", "#28282c");
    const faint = themeColor("--faint", "#7c7c81");
    const text = themeColor("--text", "#e3e3e5");
    const accent = themeColor("--accent", "#7aa7ff");
    const style = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "style",
    );
    style.textContent = `
      svg { background: ${panel}; }
      .chart-grid line { stroke: ${border}; stroke-width: 1; }
      .chart-grid text, .chart-axis-label { fill: ${faint}; font-size: 10px; }
      .chart-value-label { fill: ${text}; font-size: 10px; }
      .chart-line { fill: none; stroke: ${accent}; stroke-width: 2; }
      .chart-pie-center { fill: ${text}; font-size: 14px; font-weight: 600; }
    `;
    clone.insertBefore(style, clone.firstChild);
    const background = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "rect",
    );
    background.setAttribute("width", "100%");
    background.setAttribute("height", "100%");
    background.setAttribute("fill", panel);
    clone.insertBefore(background, style.nextSibling);
    const xml = `<?xml version="1.0" encoding="UTF-8"?>\n${clone.outerHTML}`;
    await api.writeTextFile(path, xml);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    exporting.value = false;
  }
}

async function copyChartData() {
  const header = ["分类", "值", "行数", "占比"];
  const lines = points.value.map((point) =>
    [
      point.label,
      formatChartValue(point.value),
      String(point.count),
      `${point.percent.toFixed(1)}%`,
    ].join("\t"),
  );
  try {
    await writeClipboard([header.join("\t"), ...lines].join("\n"));
  } catch {
    error.value = "复制失败，请检查剪贴板权限";
  }
}

onMounted(() => {
  const element = chartRef.value;
  if (!element) return;
  width.value = Math.max(360, element.clientWidth);
  if (typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(() => {
      if (!chartRef.value) return;
      width.value = Math.max(360, chartRef.value.clientWidth);
    });
    observer.observe(element);
  }
});

onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div ref="chartRef" class="result-chart">
    <div class="chart-toolbar">
      <div class="segmented compact-segmented">
        <button
          v-for="type in (['bar', 'line', 'pie'] as ChartType[])"
          :key="type"
          class="segment"
          :class="{ active: config.type === type }"
          @click="setType(type)"
        >
          {{ type === "bar" ? "柱状图" : type === "line" ? "折线图" : "饼图" }}
        </button>
      </div>
      <span v-if="filterCount" class="chart-filter-badge">
        筛选 {{ filterCount }} 个条件
        <button class="link-button" @click="emit('adjust-filter')">调整筛选</button>
      </span>
      <span class="spacer"></span>
      <button
        class="button ghost compact"
        :disabled="exporting"
        @click="exportSvg"
      >
        <Icon name="download" :size="13" />
        导出 SVG…
      </button>
    </div>

    <div class="chart-config">
      <label>
        X 轴
        <select
          :value="config.xIndex"
          @change="update('xIndex', Number(($event.target as HTMLSelectElement).value))"
        >
          <option :value="-1">行号</option>
          <option
            v-for="column in data.visibleColumns"
            :key="column.index"
            :value="column.index"
          >
            {{ column.name }}
          </option>
        </select>
      </label>
      <label>
        Y 轴
        <select
          :value="config.yIndex ?? -1"
          @change="setY(($event.target as HTMLSelectElement).value)"
        >
          <option :value="-1">计数</option>
          <option
            v-for="column in numericOptions"
            :key="column.index"
            :value="column.index"
          >
            {{ column.name }}
          </option>
        </select>
      </label>
      <label>
        聚合
        <select
          :value="config.aggregate"
          :disabled="config.yIndex === null"
          @change="
            update(
              'aggregate',
              ($event.target as HTMLSelectElement).value as ChartAggregate,
            )
          "
        >
          <option value="sum">求和</option>
          <option value="avg">平均</option>
          <option value="count">计数</option>
          <option value="min">最小</option>
          <option value="max">最大</option>
          <option value="none">不聚合</option>
        </select>
      </label>
      <label>
        Top N
        <select
          :value="config.topN"
          @change="update('topN', Number(($event.target as HTMLSelectElement).value))"
        >
          <option :value="5">5</option>
          <option :value="10">10</option>
          <option :value="20">20</option>
          <option :value="50">50</option>
          <option :value="1000">全部（最多 50）</option>
        </select>
      </label>
      <label>
        排序
        <select
          :value="config.sort"
          @change="
            update(
              'sort',
              ($event.target as HTMLSelectElement).value as ChartSort,
            )
          "
        >
          <option value="value-desc">按值降序</option>
          <option value="value-asc">按值升序</option>
          <option value="category-asc">分类升序</option>
          <option value="category-desc">分类降序</option>
          <option value="source">原始顺序</option>
        </select>
      </label>
      <label class="import-check">
        <input
          :checked="config.showLabels"
          type="checkbox"
          @change="update('showLabels', ($event.target as HTMLInputElement).checked)"
        />
        数值标签
      </label>
      <label class="import-check">
        <input
          :checked="config.showLegend"
          type="checkbox"
          @change="update('showLegend', ($event.target as HTMLInputElement).checked)"
        />
        图例
      </label>
      <button class="button ghost compact" @click="showData = !showData">
        {{ showData ? "隐藏图表数据" : "查看图表数据" }}
      </button>
    </div>

    <p v-if="error" class="data-message error">{{ error }}</p>
    <div
      v-if="showEmpty"
      class="panel-empty chart-empty"
      role="img"
      :aria-label="ariaLabel"
    >
      当前结果没有可绘制的数据。请选择有数值的 Y 轴，或检查筛选条件。
    </div>
    <div v-else class="chart-canvas" role="img" :aria-label="ariaLabel">
      <svg
        ref="svgRef"
        :width="width"
        :height="chartHeight"
        :viewBox="`0 0 ${width} ${chartHeight}`"
        xmlns="http://www.w3.org/2000/svg"
      >
        <title>{{ ariaLabel }}</title>
        <template v-if="config.type !== 'pie'">
          <g class="chart-grid">
            <line
              v-for="tick in ticks"
              :key="tick"
              :x1="margin.left"
              :x2="width - margin.right"
              :y1="yFor(tick)"
              :y2="yFor(tick)"
            />
            <text
              v-for="tick in ticks"
              :key="`label-${tick}`"
              :x="margin.left - 6"
              :y="yFor(tick) + 3"
              text-anchor="end"
            >
              {{ formatChartValue(tick) }}
            </text>
          </g>
          <g v-if="config.type === 'bar'">
            <g v-for="(point, index) in points" :key="point.label">
              <rect
                :x="xFor(index) - barWidth / 2"
                :y="Math.min(yFor(point.value), baselineY)"
                :width="barWidth"
                :height="Math.max(1, Math.abs(baselineY - yFor(point.value)))"
                :fill="CHART_COLORS[index % CHART_COLORS.length]"
              >
                <title>{{ point.label }}：{{ formatChartValue(point.value) }}（{{ point.count }} 行）</title>
              </rect>
              <text
                v-if="config.showLabels"
                :x="xFor(index)"
                :y="yFor(point.value) - 4"
                text-anchor="middle"
                class="chart-value-label"
              >
                {{ formatChartValue(point.value) }}
              </text>
              <text
                v-if="index % Math.ceil(points.length / 12) === 0 || points.length <= 12"
                :x="xFor(index)"
                :y="chartHeight - margin.bottom + 16"
                text-anchor="middle"
                class="chart-axis-label"
              >
                <title>{{ point.label }}</title>
                {{ labelFor(point.label) }}
              </text>
            </g>
          </g>
          <g v-else>
            <polyline :points="linePoints" class="chart-line" />
            <circle
              v-for="(point, index) in points"
              :key="point.label"
              :cx="xFor(index)"
              :cy="yFor(point.value)"
              r="3.5"
              :fill="CHART_COLORS[index % CHART_COLORS.length]"
            >
              <title>{{ point.label }}：{{ formatChartValue(point.value) }}（{{ point.count }} 行）</title>
            </circle>
            <template
              v-for="(point, index) in points"
              :key="`line-label-${point.label}`"
            >
              <text
                v-if="config.showLabels"
                :x="xFor(index)"
                :y="yFor(point.value) - 8"
                text-anchor="middle"
                class="chart-value-label"
              >
                {{ formatChartValue(point.value) }}
              </text>
            </template>
            <text
              v-for="(point, index) in points"
              v-show="index % Math.ceil(points.length / 12) === 0 || points.length <= 12"
              :key="`line-x-${point.label}`"
              :x="xFor(index)"
              :y="chartHeight - margin.bottom + 16"
              text-anchor="middle"
              class="chart-axis-label"
            >
              <title>{{ point.label }}</title>
              {{ labelFor(point.label) }}
            </text>
          </g>
        </template>
        <g v-else>
          <path
            v-for="slice in pieSlices"
            :key="slice.point.label"
            :d="slice.path"
            :fill="slice.color"
          >
            <title>{{ slice.point.label }}：{{ formatChartValue(slice.point.value) }}（{{ slice.point.percent.toFixed(1) }}%）</title>
          </path>
          <text
            v-for="slice in pieSlices"
            v-show="config.showLabels && slice.point.percent >= 4"
            :key="`pie-label-${slice.point.label}`"
            :x="
              width / 2 +
              (Math.min(innerWidth, innerHeight) / 2 - 34) *
                Math.cos(slice.mid)
            "
            :y="
              margin.top +
              innerHeight / 2 +
              (Math.min(innerWidth, innerHeight) / 2 - 34) *
                Math.sin(slice.mid)
            "
            text-anchor="middle"
            class="chart-value-label"
          >
            {{ slice.point.percent.toFixed(1) }}%
          </text>
          <text
            v-if="!config.showLabels"
            :x="width / 2"
            :y="margin.top + innerHeight / 2"
            text-anchor="middle"
            class="chart-pie-center"
          >
            {{ formatChartValue(points.reduce((sum, point) => sum + Math.abs(point.value), 0)) }}
          </text>
        </g>
        <g v-if="config.showLegend" class="chart-legends">
          <g
            v-for="(point, index) in points.slice(0, 8)"
            :key="`legend-${point.label}`"
            :transform="`translate(${margin.left + index * 90}, ${chartHeight - 14})`"
          >
            <rect width="8" height="8" :fill="CHART_COLORS[index % CHART_COLORS.length]" />
            <text x="12" y="8" class="chart-axis-label">{{ labelFor(point.label) }}</text>
          </g>
        </g>
      </svg>
    </div>

    <p class="chart-notes">
      <span v-if="series.ignored">Y 为空的 {{ series.ignored }} 行已忽略；</span>
      <span v-if="series.rowTruncated">结果超过 10,000 行，仅分析前 10,000 行；</span>
      <span v-if="series.truncated">
        共 {{ series.categories }} 个分类，仅显示前
        {{ config.topN >= 1000 ? 50 : config.topN }} 个<span
          v-if="config.type !== 'line'"
          >，其余聚合为「其他」</span
        >；
      </span>
      <span v-if="hasNegative && config.type === 'pie'">含负值，饼图按绝对值计算占比；</span>
      <span>{{ config.aggregate === "count" ? "计数包含 NULL 行" : "聚合忽略 NULL 与非数值" }}</span>
    </p>

    <div v-if="showData" class="chart-data-table-wrap">
      <div class="chart-data-actions">
        <span>图表数据（{{ points.length }} 行）</span>
        <button class="button ghost compact" @click="copyChartData">
          复制数据表
        </button>
      </div>
      <div class="chart-data-table" tabindex="0">
      <table>
        <thead>
          <tr><th>分类</th><th>值</th><th>行数</th><th>占比</th></tr>
        </thead>
        <tbody>
          <tr v-for="point in points" :key="point.label">
            <td>{{ point.label }}</td>
            <td>{{ formatChartValue(point.value) }}</td>
            <td>{{ point.count }}</td>
            <td>{{ point.percent.toFixed(1) }}%</td>
          </tr>
        </tbody>
      </table>
      </div>
    </div>
  </div>
</template>
