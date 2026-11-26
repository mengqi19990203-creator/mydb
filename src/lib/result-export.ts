import type { Driver } from "./types";
import { qualifiedIdentifier, quoteIdentifier, sqlLiteral } from "./sql-literal";

export type ResultExportFormat = "csv" | "json" | "markdown" | "sql" | "xlsx";

export interface ResultSortInfo {
  column: string;
  direction: "asc" | "desc";
  columnHidden: boolean;
}

export interface ResultExportSnapshot {
  /** Visible column names, in grid order. */
  columns: string[];
  /** Visible values in current sort order. */
  rows: unknown[][];
  sort: ResultSortInfo | null;
  hiddenCount: number;
  sourceRowCount: number;
  truncated: boolean;
  /** Rows removed by the local result filter, when one is active. */
  filteredOut?: number;
  /** Human-readable AND summary of the active filter. */
  filterSummary?: string | null;
}

export interface ResultExportOptions {
  withHeader: boolean;
  table: string;
  driver: Driver;
}

/** JSON / SQL INSERT need unique, non-empty object keys. */
export function columnNameIssue(columns: string[]): string {
  const trimmed = columns.map((column) => column.trim());
  if (trimmed.some((column) => !column)) {
    return "结果包含空列名，JSON / SQL INSERT 需要唯一列名";
  }
  if (new Set(trimmed).size !== trimmed.length) {
    return "结果包含重复列名，JSON / SQL INSERT 需要唯一列名";
  }
  return "";
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function csvValue(value: unknown): string {
  const text = displayValue(value);
  if (/[",\r\n]/.test(text) || /^\s|\s$/.test(text)) {
    return `"${text.replaceAll('"', '""')}"`;
  }
  return text;
}

export function buildCsv(
  snapshot: ResultExportSnapshot,
  withHeader: boolean,
): string {
  const lines: string[] = [];
  if (withHeader) {
    lines.push(snapshot.columns.map(csvValue).join(","));
  }
  for (const row of snapshot.rows) {
    lines.push(row.map(csvValue).join(","));
  }
  return lines.join("\n");
}

export function buildJson(snapshot: ResultExportSnapshot): string {
  const issue = columnNameIssue(snapshot.columns);
  if (issue) throw new Error(issue);
  const payload = snapshot.rows.map((row) => {
    const object: Record<string, unknown> = {};
    snapshot.columns.forEach((column, index) => {
      object[column] = row[index] ?? null;
    });
    return object;
  });
  return JSON.stringify(payload, null, 2);
}

function markdownCell(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  const text = typeof value === "object" ? JSON.stringify(value) : String(value);
  return text
    .replaceAll("\\", "\\\\")
    .replaceAll("|", "\\|")
    .replace(/\r\n|\r|\n/g, "<br>");
}

export function buildMarkdown(snapshot: ResultExportSnapshot): string {
  const header = `| ${snapshot.columns.map(markdownCell).join(" | ")} |`;
  const separator = `| ${snapshot.columns.map(() => "---").join(" | ")} |`;
  const rows = snapshot.rows.map(
    (row) => `| ${row.map(markdownCell).join(" | ")} |`,
  );
  return [header, separator, ...rows].join("\n");
}

export function buildSqlInsert(
  snapshot: ResultExportSnapshot,
  table: string,
  driver: Driver,
): string {
  const issue = columnNameIssue(snapshot.columns);
  if (issue) throw new Error(issue);
  const target = qualifiedIdentifier(table, driver);
  const columns = snapshot.columns
    .map((column) => quoteIdentifier(column, driver))
    .join(", ");
  return snapshot.rows
    .map((row) => {
      const values = row.map((value) => sqlLiteral(value, driver)).join(", ");
      return `INSERT INTO ${target} (${columns}) VALUES (${values});`;
    })
    .join("\n");
}

export function buildExportContent(
  format: ResultExportFormat,
  snapshot: ResultExportSnapshot,
  options: ResultExportOptions,
): string {
  switch (format) {
    case "json":
      return buildJson(snapshot);
    case "markdown":
      return buildMarkdown(snapshot);
    case "sql":
      return buildSqlInsert(snapshot, options.table, options.driver);
    case "xlsx":
      throw new Error("XLSX 导出需要使用二进制导出命令");
    default:
      return buildCsv(snapshot, options.withHeader);
  }
}

export function exportExtension(format: ResultExportFormat): string {
  return format === "markdown" ? "md" : format;
}

export function exportFilename(
  format: ResultExportFormat,
  prefix = "mydb-result",
): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(
    now.getDate(),
  )}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `${prefix}-${stamp}.${exportExtension(format)}`;
}