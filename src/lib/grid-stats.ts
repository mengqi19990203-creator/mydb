import { formatGridCell } from "./grid-search";

export interface GridStats {
  cells: number;
  nonNull: number;
  nulls: number;
  distinct: number;
  numericCount: number;
  sum: number;
  average: number | null;
  min: number | null;
  max: number | null;
}

export function isNumericCell(value: unknown): boolean {
  if (typeof value === "number") return Number.isFinite(value);
  if (typeof value === "string") {
    if (!value.trim()) return false;
    return Number.isFinite(Number(value));
  }
  return false;
}

export function summarizeCells(values: readonly unknown[]): GridStats {
  const seen = new Set<string>();
  let nonNull = 0;
  let nulls = 0;
  let numericCount = 0;
  let sum = 0;
  let min: number | null = null;
  let max: number | null = null;
  for (const value of values) {
    if (value === null || value === undefined) {
      nulls += 1;
      seen.add("<NULL>");
    } else {
      nonNull += 1;
      seen.add(formatGridCell(value));
    }
    if (isNumericCell(value)) {
      const number = typeof value === "number" ? value : Number(value);
      numericCount += 1;
      sum += number;
      min = min === null ? number : Math.min(min, number);
      max = max === null ? number : Math.max(max, number);
    }
  }
  return {
    cells: values.length,
    nonNull,
    nulls,
    distinct: seen.size,
    numericCount,
    sum,
    average: numericCount ? sum / numericCount : null,
    min,
    max,
  };
}

export function formatStatValue(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  if (Number.isInteger(value)) return value.toLocaleString("zh-CN");
  return Number(value.toFixed(6)).toLocaleString("zh-CN", {
    maximumFractionDigits: 6,
  });
}
