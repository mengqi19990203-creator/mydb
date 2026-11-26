import { quoteIdentifier, sqlLiteral } from "./sql-literal";
import type { Driver } from "./types";

export type TableExportRange = "page" | "selected" | "all";
export type TableExportFormat = "csv" | "json" | "sql" | "xlsx";

export function tableExportExtension(format: TableExportFormat): string {
  return format === "xlsx" ? "xlsx" : format === "json" ? "json" : format === "sql" ? "sql" : "csv";
}

export function tableExportTimestamp(date = new Date()): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(
    date.getHours(),
  )}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function cellText(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

function csvField(value: unknown): string {
  const text = cellText(value);
  return /[",\r\n]/.test(text) || /^\s|\s$/.test(text)
    ? `"${text.replaceAll('"', '""')}"`
    : text;
}

export function rowsToCsv(
  rows: readonly (readonly unknown[])[],
  columns: readonly string[],
  withHeader: boolean,
): string {
  const lines: string[] = [];
  if (withHeader) lines.push(columns.map(csvField).join(","));
  for (const row of rows) {
    lines.push(row.slice(0, columns.length).map(csvField).join(","));
  }
  return lines.join("\n");
}

export function rowsToJson(
  rows: readonly (readonly unknown[])[],
  columns: readonly string[],
  table: string,
): string {
  return JSON.stringify(
    {
      table,
      columns,
      rows: rows.map((row) =>
        columns.map((_, index) => row[index] ?? null),
      ),
    },
    null,
    2,
  );
}

export function rowsToSql(
  rows: readonly (readonly unknown[])[],
  columns: readonly string[],
  table: string,
  driver: Driver,
): string {
  const reference = table
    .split(".")
    .map((part) => quoteIdentifier(part.trim(), driver))
    .join(".");
  const columnList = columns
    .map((column) => quoteIdentifier(column, driver))
    .join(", ");
  return rows
    .map(
      (row) =>
        `INSERT INTO ${reference} (${columnList}) VALUES (${columns
          .map((_, index) => sqlLiteral(row[index] ?? null, driver))
          .join(", ")});`,
    )
    .join("\n");
}
