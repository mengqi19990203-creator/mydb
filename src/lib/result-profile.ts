import { formatGridCell } from "./grid-search";
import { isNumericCell } from "./grid-stats";

export const PROFILE_COLUMN_LIMIT = 200;
export const PROFILE_TOP_LIMIT = 20;
export const PROFILE_HISTOGRAM_BINS = 10;

export type ProfileColumnType =
  | "numeric"
  | "text"
  | "boolean"
  | "date-like"
  | "json"
  | "binary-like"
  | "empty"
  | "mixed";

export interface ProfileValue {
  label: string;
  count: number;
  percent: number;
  blank?: boolean;
}

export interface ProfileHistogramBin {
  label: string;
  count: number;
}

export interface ColumnProfile {
  key: string;
  name: string;
  index: number;
  type: ProfileColumnType;
  total: number;
  nulls: number;
  nullPercent: number;
  blanks: number;
  blankPercent: number;
  distinct: number;
  unique: number;
  duplicateValues: number;
  duplicatePercent: number;
  min: number | null;
  max: number | null;
  average: number | null;
  stddev: number | null;
  minLength: number | null;
  maxLength: number | null;
  averageLength: number | null;
  dateMin: string | null;
  dateMax: string | null;
  textNumeric: boolean;
  quality: string[];
  examples: string[];
  top: ProfileValue[];
  otherValues: number;
  otherCount: number;
  histogram: ProfileHistogramBin[];
  lengthHistogram: ProfileHistogramBin[];
}

export interface ProfileDataInput {
  columns: string[];
  visibleColumns: Array<{ name: string; index: number }>;
  rows: unknown[][];
  truncated?: boolean;
  filterSummary?: string | null;
  sourceLabel?: string;
}

export interface ResultProfileResult {
  columns: ColumnProfile[];
  totalRows: number;
  visibleColumns: number;
  truncatedColumns: boolean;
}

const DATE_PATTERN =
  /^\d{4}-\d{2}-\d{2}(?:[ T]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?)?$/;
const BINARY_PATTERN = /^<BLOB \d+ bytes>$/i;

function isBooleanValue(value: unknown): boolean {
  if (typeof value === "boolean") return true;
  if (typeof value !== "string") return false;
  const text = value.trim().toLocaleLowerCase();
  return text === "true" || text === "false";
}

function isDateValue(value: unknown): boolean {
  return typeof value === "string" && DATE_PATTERN.test(value.trim());
}

function isBinaryValue(value: unknown): boolean {
  return (
    typeof value === "string" && BINARY_PATTERN.test(value.trim())
  );
}

function isObjectValue(value: unknown): boolean {
  return typeof value === "object" && value !== null;
}

function numericValue(value: unknown): number | null {
  if (!isNumericCell(value)) return null;
  const number = typeof value === "number" ? value : Number(value);
  return Number.isFinite(number) ? number : null;
}

function percent(count: number, total: number): number {
  return total ? (count / total) * 100 : 0;
}

function histogram(
  values: number[],
  bins: number,
  formatter: (value: number) => string,
): ProfileHistogramBin[] {
  if (!values.length) return [];
  const min = Math.min(...values);
  const max = Math.max(...values);
  if (min === max) {
    return [{ label: formatter(min), count: values.length }];
  }
  const width = (max - min) / bins;
  const counts = Array.from({ length: bins }, () => 0);
  for (const value of values) {
    const index = Math.min(
      bins - 1,
      Math.max(0, Math.floor((value - min) / width)),
    );
    counts[index] += 1;
  }
  return counts.map((count, index) => {
    const start = min + width * index;
    const end = index === bins - 1 ? max : min + width * (index + 1);
    return {
      label: `${formatter(start)} – ${formatter(end)}`,
      count,
    };
  });
}

function formatNumber(value: number): string {
  if (Number.isInteger(value)) return String(value);
  return String(Number(value.toFixed(3)));
}

function columnKeys(
  columns: string[],
): Array<{ key: string; name: string; index: number }> {
  const counts = new Map<string, number>();
  for (const name of columns) {
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return columns.map((name, index) => ({
    key: (counts.get(name) ?? 0) > 1 ? `${name}#${index + 1}` : name,
    name,
    index,
  }));
}

export function profileColumn(
  column: { key: string; name: string; index: number },
  rows: unknown[][],
): ColumnProfile {
  const total = rows.length;
  let nulls = 0;
  let blanks = 0;
  let numbers = 0;
  let numericStrings = 0;
  let booleans = 0;
  let texts = 0;
  let dates = 0;
  let objects = 0;
  let binaries = 0;
  let leadingTrailingSpaces = 0;
  let sum = 0;
  let sumSquares = 0;
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  const lengths: number[] = [];
  const dateValues: string[] = [];
  const numericValues: number[] = [];
  const counts = new Map<string, number>();
  const examples: string[] = [];

  for (const row of rows) {
    const value = row[column.index];
    if (value === null || value === undefined) {
      nulls += 1;
      continue;
    }
    const label = formatGridCell(value);
    counts.set(label, (counts.get(label) ?? 0) + 1);
    if (examples.length < 5 && !examples.includes(label)) {
      examples.push(label);
    }
    if (typeof value === "string") {
      if (!value.trim()) {
        blanks += 1;
      } else if (value !== value.trim()) {
        leadingTrailingSpaces += 1;
      }
      lengths.push(value.length);
    }
    if (isObjectValue(value)) {
      objects += 1;
      continue;
    }
    if (isBinaryValue(value)) {
      binaries += 1;
      continue;
    }
    const number = numericValue(value);
    if (number !== null) {
      numbers += 1;
      if (typeof value === "string") numericStrings += 1;
      numericValues.push(number);
      sum += number;
      sumSquares += number * number;
      min = Math.min(min, number);
      max = Math.max(max, number);
      continue;
    }
    if (isBooleanValue(value)) {
      booleans += 1;
      continue;
    }
    if (typeof value === "string" && isDateValue(value)) {
      dates += 1;
      dateValues.push(value.trim());
      continue;
    }
    texts += 1;
  }

  const nonNull = total - nulls;
  let type: ProfileColumnType = "empty";
  if (nonNull > 0) {
    if (numbers === nonNull) type = "numeric";
    else if (booleans === nonNull) type = "boolean";
    else if (dates === nonNull) type = "date-like";
    else if (objects === nonNull) type = "json";
    else if (binaries === nonNull) type = "binary-like";
    else if (texts === nonNull) type = "text";
    else type = "mixed";
  }

  const typeCounts = [
    numbers,
    booleans,
    dates,
    objects,
    binaries,
    texts,
  ].filter((count) => count > 0);
  const distinct = counts.size;
  const sortedValues = [...counts.entries()].sort(
    (left, right) => right[1] - left[1] || left[0].localeCompare(right[0]),
  );
  const unique = sortedValues.filter(([, count]) => count === 1).length;
  const duplicateValues = sortedValues.filter(([, count]) => count > 1).length;
  const duplicateRows = sortedValues
    .filter(([, count]) => count > 1)
    .reduce((sumValue, [, count]) => sumValue + count, 0);
  const average = numbers ? sum / numbers : null;
  const variance = numbers
    ? Math.max(0, sumSquares / numbers - (average ?? 0) ** 2)
    : 0;
  const stddev = numbers ? Math.sqrt(variance) : null;

  const nonBlankValues = sortedValues.filter(
    ([label]) => label.trim() !== "",
  );
  const topEntries = nonBlankValues.slice(0, PROFILE_TOP_LIMIT);
  const top = topEntries.map(([label, count]) => ({
    label,
    count,
    percent: percent(count, total),
    blank: false,
  }));
  const otherEntries = nonBlankValues.slice(PROFILE_TOP_LIMIT);
  const otherValues = otherEntries.length;
  const otherCount = otherEntries.reduce(
    (totalValue, [, count]) => totalValue + count,
    0,
  );

  const quality: string[] = [];
  if (nonNull === 0) quality.push("全 NULL");
  else if (nulls / total > 0.3) quality.push("高 NULL");
  if (distinct === 1 && nonNull > 0) quality.push("常量");
  if (blanks) quality.push("含空白串");
  if (duplicateRows / Math.max(1, nonNull) > 0.5) quality.push("高重复");
  if (typeCounts.length > 1) quality.push("混合类型");
  if (numericStrings) quality.push("文本数字");
  if (leadingTrailingSpaces) quality.push("首尾空白");

  const averageLength = lengths.length
    ? lengths.reduce((sumValue, length) => sumValue + length, 0) /
      lengths.length
    : null;
  const lengthHistogram = lengths.length
    ? histogram(lengths, PROFILE_HISTOGRAM_BINS, formatNumber)
    : [];

  return {
    key: column.key,
    name: column.name,
    index: column.index,
    type,
    total,
    nulls,
    nullPercent: percent(nulls, total),
    blanks,
    blankPercent: percent(blanks, total),
    distinct,
    unique,
    duplicateValues,
    duplicatePercent: percent(duplicateRows, Math.max(1, nonNull)),
    min: numbers ? min : null,
    max: numbers ? max : null,
    average,
    stddev,
    minLength: lengths.length ? Math.min(...lengths) : null,
    maxLength: lengths.length ? Math.max(...lengths) : null,
    averageLength,
    dateMin: dateValues.length ? [...dateValues].sort()[0] : null,
    dateMax: dateValues.length
      ? [...dateValues].sort()[dateValues.length - 1]
      : null,
    textNumeric: numericStrings > 0,
    quality,
    examples,
    top,
    otherValues,
    otherCount,
    histogram: numericValues.length
      ? histogram(numericValues, PROFILE_HISTOGRAM_BINS, formatNumber)
      : [],
    lengthHistogram,
  };
}

export function profileResult(
  input: ProfileDataInput,
  limit = PROFILE_COLUMN_LIMIT,
): ResultProfileResult {
  const keys = columnKeys(input.columns);
  const bounded = Math.max(0, Math.min(limit, PROFILE_COLUMN_LIMIT * 100));
  const visible = input.visibleColumns.slice(0, bounded);
  const columns = visible.map((column) => {
    const info = keys[column.index] ?? {
      key: column.name,
      name: column.name,
      index: column.index,
    };
    return profileColumn(info, input.rows);
  });
  return {
    columns,
    totalRows: input.rows.length,
    visibleColumns: input.visibleColumns.length,
    truncatedColumns: input.visibleColumns.length > bounded,
  };
}

export const PROFILE_TYPE_LABELS: Record<ProfileColumnType, string> = {
  numeric: "数值",
  text: "文本",
  boolean: "布尔",
  "date-like": "日期",
  json: "JSON / 对象",
  "binary-like": "二进制",
  empty: "全空",
  mixed: "混合",
};

function tsvCell(value: string | number): string {
  return String(value)
    .replace(/\t/g, "\\t")
    .replace(/\r?\n/g, "\\n");
}

export function profileToTsv(
  profiles: ColumnProfile[],
  sourceLabel = "",
): string {
  const header = [
    "列",
    "类型",
    "总数",
    "NULL",
    "NULL%",
    "空白串",
    "Distinct",
    "唯一值",
    "重复值",
    "最小值",
    "最大值",
    "平均值",
    "标准差",
    "最小长度",
    "最大长度",
    "平均长度",
    "质量",
    "Top 值",
  ];
  if (sourceLabel) header.push("来源");
  const lines = [header.join("\t")];
  for (const profile of profiles) {
    const cells = [
      profile.key,
      PROFILE_TYPE_LABELS[profile.type],
      profile.total,
      profile.nulls,
      `${profile.nullPercent.toFixed(1)}%`,
      profile.blanks,
      profile.distinct,
      profile.unique,
      profile.duplicateValues,
      profile.min ?? "",
      profile.max ?? "",
      profile.average === null ? "" : formatNumber(profile.average),
      profile.stddev === null ? "" : formatNumber(profile.stddev),
      profile.minLength ?? "",
      profile.maxLength ?? "",
      profile.averageLength === null
        ? ""
        : formatNumber(profile.averageLength),
      profile.quality.join(" / "),
      profile.top
        .map((value) => `${value.label} (${value.count})`)
        .join("; "),
    ];
    if (sourceLabel) cells.push(sourceLabel);
    lines.push(cells.map(tsvCell).join("\t"));
  }
  return lines.join("\n");
}

export function profileToMarkdown(
  profiles: ColumnProfile[],
  sourceLabel: string,
): string {
  const lines = [
    `### 列剖析 · ${sourceLabel}`,
    "",
    "| 列 | 类型 | NULL | NULL% | Distinct | 唯一值 | 重复值 | 最小 | 最大 | 平均 | 标准差 | 质量 |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
  ];
  const escape = (value: string) =>
    value.replace(/\|/g, "\\|").replace(/\r?\n/g, " ");
  for (const profile of profiles) {
    lines.push(
      `| ${escape(profile.key)} | ${PROFILE_TYPE_LABELS[profile.type]} | ${profile.nulls} | ${profile.nullPercent.toFixed(1)}% | ${profile.distinct} | ${profile.unique} | ${profile.duplicateValues} | ${profile.min ?? ""} | ${profile.max ?? ""} | ${profile.average === null ? "" : formatNumber(profile.average)} | ${profile.stddev === null ? "" : formatNumber(profile.stddev)} | ${escape(profile.quality.join(" / "))} |`,
    );
  }
  return lines.join("\n");
}
