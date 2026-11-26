import type { ColumnInfo } from "./types";

export const CLIPBOARD_TEXT_LIMIT = 5 * 1024 * 1024;
export const CLIPBOARD_ROW_LIMIT = 5_000;
export const CLIPBOARD_HARD_ROW_LIMIT = 20_000;

export interface ParsedClipboardTable {
  columns: string[];
  rows: string[][];
  delimiter: string;
  headerDetected: boolean;
  rowCount: number;
}

export interface ClipboardErrorRow {
  row: number;
  column: string;
  message: string;
}

export interface ClipboardConvertResult {
  drafts: Array<Array<unknown | undefined>>;
  errors: ClipboardErrorRow[];
  skipped: number;
}

function firstNonEmptyLine(text: string): string {
  return text.split(/\r\n|\n|\r/).find((line) => line.trim().length > 0) ?? "";
}

export function detectDelimiter(text: string): string {
  const line = firstNonEmptyLine(text);
  if (line.includes("\t")) return "\t";
  if (line.includes(",")) return ",";
  if (line.includes(";")) return ";";
  return "\t";
}

/** RFC4180-ish parser: quotes, escaped quotes, embedded newlines, CRLF/LF/CR. */
export function parseDelimitedText(
  text: string,
  delimiter: string,
): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let index = 0;
  while (index < text.length) {
    const character = text[index];
    if (quoted) {
      if (character === '"') {
        if (text[index + 1] === '"') {
          field += '"';
          index += 2;
          continue;
        }
        quoted = false;
        index += 1;
        continue;
      }
      field += character;
      index += 1;
      continue;
    }
    if (character === '"' && field.length === 0) {
      quoted = true;
      index += 1;
      continue;
    }
    if (character === delimiter) {
      row.push(field);
      field = "";
      index += 1;
      continue;
    }
    if (character === "\r" || character === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      if (character === "\r" && text[index + 1] === "\n") index += 2;
      else index += 1;
      continue;
    }
    field += character;
    index += 1;
  }
  row.push(field);
  rows.push(row);
  while (rows.length && rows[rows.length - 1].every((cell) => cell === "")) {
    rows.pop();
  }
  return rows;
}

function uniqueColumns(values: string[], width: number): string[] {
  const columns: string[] = [];
  const push = (base: string) => {
    const trimmed = base.trim();
    const fallback = `列 ${columns.length + 1}`;
    let candidate = trimmed || fallback;
    let suffix = 2;
    while (columns.includes(candidate)) {
      candidate = `${trimmed || fallback} (${suffix})`;
      suffix += 1;
    }
    columns.push(candidate);
  };
  values.forEach(push);
  while (columns.length < width) push("");
  return columns;
}

export function headerLooksLikeColumns(
  firstRow: readonly string[],
  columns: readonly ColumnInfo[],
): boolean {
  if (!firstRow.length || !columns.length) return false;
  const names = new Set(columns.map((column) => column.name.toLowerCase()));
  return firstRow.every((cell) => names.has(cell.trim().toLowerCase()));
}

export function buildClipboardTable(
  text: string,
  delimiter: string,
  hasHeader: boolean,
  tableColumns: readonly ColumnInfo[],
): ParsedClipboardTable {
  const raw = parseDelimitedText(text, delimiter);
  const width = raw.reduce((max, row) => Math.max(max, row.length), 0);
  if (!raw.length || !width) {
    return {
      columns: [],
      rows: [],
      delimiter,
      headerDetected: false,
      rowCount: 0,
    };
  }
  const data = raw.map((row) => {
    const copy = [...row];
    while (copy.length < width) copy.push("");
    return copy;
  });
  const headerDetected = headerLooksLikeColumns(data[0], tableColumns);
  const header = hasHeader ? data.shift() ?? [] : [];
  const columns = uniqueColumns(header, width);
  return {
    columns,
    rows: data,
    delimiter,
    headerDetected,
    rowCount: data.length,
  };
}

export function autoMapColumns(
  source: readonly string[],
  columns: readonly ColumnInfo[],
): Record<string, string> {
  const mapping: Record<string, string> = {};
  const used = new Set<string>();
  const byName = new Map(
    columns.map((column) => [column.name.toLowerCase(), column]),
  );
  for (const name of source) {
    const match = byName.get(name.toLowerCase());
    if (!match || match.autoIncrement || used.has(match.name)) {
      mapping[name] = "";
      continue;
    }
    mapping[name] = match.name;
    used.add(match.name);
  }
  return mapping;
}

function isBooleanType(type: string): boolean {
  const upper = type.toUpperCase();
  return (
    upper.includes("BOOL") ||
    upper === "BIT" ||
    /^TINYINT\s*\(\s*1\s*\)/.test(upper)
  );
}

function isNumericType(type: string): boolean {
  const upper = type.toUpperCase();
  return (
    upper.includes("INT") ||
    upper.includes("DECIMAL") ||
    upper.includes("NUMERIC") ||
    upper.includes("FLOAT") ||
    upper.includes("DOUBLE") ||
    upper.includes("REAL") ||
    upper.includes("NUMBER")
  );
}

export function parseClipboardValue(
  raw: string,
  column: ColumnInfo,
  emptyAsNull: boolean,
): unknown {
  if (raw === "") {
    if (emptyAsNull) return null;
    if (isNumericType(column.dataType) || isBooleanType(column.dataType)) {
      return raw;
    }
    return "";
  }
  if (raw === "NULL" && emptyAsNull) return null;
  if (isBooleanType(column.dataType)) {
    const lower = raw.trim().toLowerCase();
    if (["true", "1"].includes(lower)) return true;
    if (["false", "0"].includes(lower)) return false;
    throw new Error("布尔列只接受 true / false / 1 / 0");
  }
  if (isNumericType(column.dataType)) {
    const number = Number(raw);
    if (!Number.isFinite(number)) {
      throw new Error("无法转换为数字");
    }
    return number;
  }
  return raw;
}

export function convertPasteRows(
  table: ParsedClipboardTable,
  mapping: Record<string, string>,
  columns: readonly ColumnInfo[],
  emptyAsNull: boolean,
): ClipboardConvertResult {
  const targetIndex = new Map(
    columns.map((column, index) => [column.name, index]),
  );
  const drafts: Array<Array<unknown | undefined>> = [];
  const errors: ClipboardErrorRow[] = [];
  for (let rowIndex = 0; rowIndex < table.rows.length; rowIndex += 1) {
    const row = table.rows[rowIndex];
    const values: Array<unknown | undefined> = columns.map(() => undefined);
    let rowFailed = false;
    table.columns.forEach((source, sourceIndex) => {
      const target = mapping[source];
      if (!target) return;
      const column = columns[targetIndex.get(target) ?? -1];
      if (!column) return;
      try {
        values[targetIndex.get(target)!] = parseClipboardValue(
          row[sourceIndex] ?? "",
          column,
          emptyAsNull,
        );
      } catch (reason) {
        rowFailed = true;
        errors.push({
          row: rowIndex + 1,
          column: target,
          message: String(reason),
        });
      }
    });
    if (!rowFailed) drafts.push(values);
  }
  return {
    drafts,
    errors,
    skipped: table.rows.length - drafts.length,
  };
}