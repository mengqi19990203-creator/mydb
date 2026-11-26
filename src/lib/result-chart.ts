import { isNumericCell } from "./grid-stats";
import { formatGridCell } from "./grid-search";

export interface ChartColumn {
  name: string;
  index: number;
}

export interface ChartData {
  columns: string[];
  visibleColumns: ChartColumn[];
  rows: unknown[][];
  truncated: boolean;
}

export type ChartType = "bar" | "line" | "pie";
export type ChartAggregate = "sum" | "avg" | "count" | "min" | "max" | "none";
export type ChartSort =
  | "category-asc"
  | "category-desc"
  | "value-asc"
  | "value-desc"
  | "source";

export interface ChartConfig {
  type: ChartType;
  xIndex: number;
  yIndex: number | null;
  aggregate: ChartAggregate;
  topN: number;
  sort: ChartSort;
  showLabels: boolean;
  showLegend: boolean;
}

export interface ChartPoint {
  label: string;
  value: number;
  count: number;
  percent: number;
}

export const CHART_COLORS = [
  "#4f8cff",
  "#3fb950",
  "#d29922",
  "#f4676f",
  "#a371f7",
  "#39c5cf",
  "#db6d28",
  "#8b949e",
];

export function isChartNumericColumn(data: ChartData, index: number): boolean {
  let seen = 0;
  let numeric = 0;
  for (const row of data.rows.slice(0, 200)) {
    const value = row[index];
    if (value === null || value === undefined) continue;
    seen += 1;
    if (isNumericCell(value)) numeric += 1;
  }
  return seen > 0 && numeric === seen;
}

export function numericColumns(data: ChartData): ChartColumn[] {
  return data.visibleColumns.filter((column) =>
    isChartNumericColumn(data, column.index),
  );
}

function xLabel(row: unknown[], config: ChartConfig): string {
  if (config.xIndex < 0) return "";
  const value = row[config.xIndex];
  if (value === null || value === undefined) return "(NULL)";
  return formatGridCell(value);
}

function yValue(row: unknown[], config: ChartConfig): number | null {
  if (config.yIndex === null) return null;
  const value = row[config.yIndex];
  return isNumericCell(value)
    ? typeof value === "number"
      ? value
      : Number(value)
    : null;
}

export function buildChartSeries(
  data: ChartData,
  config: ChartConfig,
): {
  points: ChartPoint[];
  ignored: number;
  categories: number;
  truncated: boolean;
  rowTruncated: boolean;
} {
  const source = data.rows.slice(0, 10_000);
  const sourceTruncated = data.rows.length > 10_000;
  // Only "不聚合" draws one point per row. Counting rows per X still needs
  // grouping, even when no numeric Y column is selected.
  const noAggregate = config.aggregate === "none";
  const counts = new Map<string, { sum: number; count: number; numeric: number; min: number; max: number }>();
  const labels = new Set<string>();
  let ignored = 0;
  for (let index = 0; index < source.length; index += 1) {
    const row = source[index];
    let label = xLabel(row, config);
    if (config.xIndex < 0) label = `行 ${index + 1}`;
    if (noAggregate) {
      let unique = label;
      let suffix = 2;
      while (labels.has(unique)) {
        unique = `${label} (行 ${suffix})`;
        suffix += 1;
      }
      labels.add(unique);
      const value = yValue(row, config);
      if (config.aggregate !== "count" && value === null) {
        ignored += 1;
        continue;
      }
      counts.set(unique, {
        sum: value ?? 0,
        count: 1,
        numeric: value === null ? 0 : 1,
        min: value ?? 0,
        max: value ?? 0,
      });
      continue;
    }
    const value = yValue(row, config);
    if (config.aggregate !== "count" && value === null) {
      ignored += 1;
      continue;
    }
    const entry = counts.get(label) ?? {
      sum: 0,
      count: 0,
      numeric: 0,
      min: Number.POSITIVE_INFINITY,
      max: Number.NEGATIVE_INFINITY,
    };
    entry.count += 1;
    if (value !== null) {
      entry.sum += value;
      entry.numeric += 1;
      entry.min = Math.min(entry.min, value);
      entry.max = Math.max(entry.max, value);
    }
    counts.set(label, entry);
  }
  let points: ChartPoint[] = [...counts.entries()].map(([label, entry]) => ({
    label,
    value:
      config.aggregate === "count"
        ? entry.count
        : config.aggregate === "avg"
          ? entry.numeric
            ? entry.sum / entry.numeric
            : 0
          : config.aggregate === "min"
            ? entry.numeric
              ? entry.min
              : 0
            : config.aggregate === "max"
              ? entry.numeric
                ? entry.max
                : 0
              : entry.sum,
    count: entry.count,
    percent: 0,
  }));
  const categories = points.length;
  if (config.type === "line") {
    if (config.sort === "value-asc") points.sort((a, b) => a.value - b.value);
    else if (config.sort === "value-desc") points.sort((a, b) => b.value - a.value);
    else if (config.sort === "category-desc") points.sort((a, b) => b.label.localeCompare(a.label, "zh-CN", { numeric: true }));
    else if (config.sort === "category-asc") points.sort((a, b) => a.label.localeCompare(b.label, "zh-CN", { numeric: true }));
  } else {
    if (config.sort === "value-asc") points.sort((a, b) => a.value - b.value);
    else if (config.sort === "category-asc") points.sort((a, b) => a.label.localeCompare(b.label, "zh-CN", { numeric: true }));
    else if (config.sort === "category-desc") points.sort((a, b) => b.label.localeCompare(a.label, "zh-CN", { numeric: true }));
    else points.sort((a, b) => b.value - a.value);
  }
  const limit = Math.max(1, Math.min(50, config.topN || 20));
  const truncated = points.length > limit;
  if (truncated) {
    if (config.type === "line") {
      points = points.slice(0, limit);
    } else {
      const kept = points.slice(0, limit);
      const rest = points.slice(limit);
      const other = rest.reduce(
        (total, point) => total + point.value,
        0,
      );
      const otherCount = rest.reduce((total, point) => total + point.count, 0);
      kept.push({ label: "其他", value: other, count: otherCount, percent: 0 });
      points = kept;
    }
  }
  const total = points.reduce((sum, point) => sum + Math.abs(point.value), 0);
  for (const point of points) {
    point.percent = total ? (Math.abs(point.value) / total) * 100 : 0;
  }
  return {
    points,
    ignored,
    categories,
    truncated,
    rowTruncated: sourceTruncated,
  };
}

export function niceTicks(max: number, count = 5): number[] {
  return niceTicksRange(0, Math.max(0, max), count);
}

export function niceTicksRange(min: number, max: number, count = 5): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, 1];
  if (min === max) {
    if (min === 0) return [0, 1];
    return min > 0 ? [0, min] : [min, 0];
  }
  const rough = (max - min) / count;
  const power = Math.pow(10, Math.floor(Math.log10(rough)));
  const step = Math.ceil(rough / power) * power;
  const start = Math.floor(min / step) * step;
  const end = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let value = start; value <= end + step * 0.001; value += step) {
    ticks.push(Math.round(value * 1e10) / 1e10);
  }
  return ticks.length ? ticks : [0, 1];
}

export function formatChartValue(value: number): string {
  if (!Number.isFinite(value)) return "—";
  if (Number.isInteger(value)) return value.toLocaleString("zh-CN");
  return Number(value.toFixed(3)).toLocaleString("zh-CN", {
    maximumFractionDigits: 3,
  });
}

export function chartAriaLabel(
  type: ChartType,
  config: ChartConfig,
  data: ChartData,
  pointCount: number,
): string {
  const typeLabel = type === "bar" ? "柱状图" : type === "line" ? "折线图" : "饼图";
  const x = config.xIndex >= 0 ? data.columns[config.xIndex] : "行号";
  const y =
    config.aggregate === "count"
      ? "行数"
      : config.yIndex !== null
        ? data.columns[config.yIndex]
        : "";
  return `${typeLabel}：按 ${x} ${config.aggregate === "count" ? "计数" : `统计 ${y}`}，共 ${pointCount} 个分类`;
}
