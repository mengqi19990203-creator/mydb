import type { Driver } from "./types";

/** Quotes a single SQL identifier for the given driver. */
export function quoteIdentifier(name: string, driver: Driver): string {
  if (driver === "mysql") {
    return `\`${name.replaceAll("`", "``")}\``;
  }
  return `"${name.replaceAll('"', '""')}"`;
}

/** Quotes a `table` or `schema.table` name, one segment at a time. */
export function qualifiedIdentifier(input: string, driver: Driver): string {
  const parts = input.split(".").map((part) => part.trim());
  if (!parts.length || parts.some((part) => !part)) {
    throw new Error("目标表名格式不正确，请填写 table 或 schema.table");
  }
  return parts.map((part) => quoteIdentifier(part, driver)).join(".");
}

function quoteString(text: string, driver: Driver): string {
  if (driver === "mysql") {
    // MySQL treats backslash as an escape character even inside quotes.
    return `'${text
      .replaceAll("\\", "\\\\")
      .replaceAll("\0", "\\0")
      .replaceAll("'", "''")}'`;
  }
  // PostgreSQL and SQLite text cannot contain NUL bytes.
  return `'${text.replaceAll("\0", "").replaceAll("'", "''")}'`;
}

/** Formats a JSON value as a SQL literal for INSERT statements. */
export function sqlLiteral(value: unknown, driver: Driver): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  if (typeof value === "number") {
    return Number.isFinite(value) ? String(value) : "NULL";
  }
  if (typeof value === "bigint") return value.toString();
  if (typeof value === "object") {
    return quoteString(JSON.stringify(value), driver);
  }
  return quoteString(String(value), driver);
}