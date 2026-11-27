import type { Driver, QueryResult } from "./types";

export interface DatasetSource {
  name: string;
  connectionId: string;
  connectionName: string;
  driver: Driver;
  database?: string | null;
  schema?: string | null;
  sql: string;
  result: QueryResult;
}

export interface SavedDataset {
  id: string;
  name: string;
  connectionId: string;
  connectionName: string;
  driver: Driver;
  database: string;
  schema: string;
  sql: string;
  columns: string[];
  rows: unknown[][];
  rowCount: number;
  truncated: boolean;
  savedAt: number;
}

const STORAGE_KEY = "mydb.datasets.v1";
const STORAGE_VERSION = 1;
const MAX_DATASETS = 100;
const MAX_ROWS = 2000;
const MAX_DATASET_BYTES = 2 * 1024 * 1024;

interface StoredDatasets {
  version: number;
  datasets: SavedDataset[];
}

function readStored(): SavedDataset[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Partial<StoredDatasets>;
    if (parsed.version !== STORAGE_VERSION || !Array.isArray(parsed.datasets)) {
      localStorage.removeItem(STORAGE_KEY);
      return [];
    }
    return parsed.datasets.filter(
      (dataset): dataset is SavedDataset =>
        Boolean(dataset) &&
        typeof dataset.id === "string" &&
        typeof dataset.name === "string" &&
        Array.isArray(dataset.columns) &&
        Array.isArray(dataset.rows),
    );
  } catch {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // Ignore storage failures.
    }
    return [];
  }
}

function writeStored(datasets: SavedDataset[]): void {
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ version: STORAGE_VERSION, datasets }),
    );
  } catch {
    // Keep the in-memory dialog usable even when localStorage is full.
  }
}

export function listDatasets(): SavedDataset[] {
  return readStored().sort((left, right) => right.savedAt - left.savedAt);
}

export function saveDataset(source: DatasetSource): SavedDataset {
  const existing = readStored();
  let rows = source.result.rows.slice(0, MAX_ROWS).map((row) => [...row]);
  let truncated = source.result.truncated || source.result.rows.length > MAX_ROWS;
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `dataset-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  const base: SavedDataset = {
    id,
    name: source.name.trim() || "未命名数据集",
    connectionId: source.connectionId,
    connectionName: source.connectionName,
    driver: source.driver,
    database: source.database ?? "",
    schema: source.schema ?? "",
    sql: source.sql,
    columns: [...source.result.columns],
    rows,
    rowCount: source.result.rows.length,
    truncated,
    savedAt: Date.now(),
  };
  let serialized = JSON.stringify(base);
  while (serialized.length > MAX_DATASET_BYTES && rows.length > 0) {
    rows = rows.slice(0, Math.floor(rows.length / 2));
    truncated = true;
    base.rows = rows;
    base.truncated = truncated;
    serialized = JSON.stringify(base);
  }
  const datasets = [base, ...existing.filter((item) => item.id !== id)].slice(
    0,
    MAX_DATASETS,
  );
  writeStored(datasets);
  return base;
}

export function deleteDataset(id: string): void {
  writeStored(readStored().filter((dataset) => dataset.id !== id));
}

export function datasetScopeLabel(dataset: SavedDataset): string {
  return [dataset.database, dataset.schema].filter(Boolean).join(" · ");
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function cellText(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

export function datasetHtml(dataset: SavedDataset): string {
  const generated = new Date().toLocaleString("zh-CN", { hour12: false });
  const rows = dataset.rows
    .map(
      (row) =>
        `<tr>${dataset.columns
          .map((_, index) => `<td>${escapeHtml(cellText(row[index]))}</td>`)
          .join("")}</tr>`,
    )
    .join("\n");
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>${escapeHtml(dataset.name)}</title>
<style>
  :root { color-scheme: light; }
  body { margin: 32px; color: #1f2937; font: 13px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
  h1 { margin: 0 0 6px; font-size: 24px; }
  .meta { color: #6b7280; font-size: 12px; }
  .sql { margin: 18px 0; padding: 12px; border: 1px solid #e5e7eb; border-radius: 6px; background: #f9fafb; white-space: pre-wrap; word-break: break-word; font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace; }
  table { width: 100%; border-collapse: collapse; margin-top: 18px; font-size: 12px; }
  th, td { border: 1px solid #e5e7eb; padding: 6px 8px; text-align: left; vertical-align: top; }
  th { background: #f3f4f6; }
  tr:nth-child(even) td { background: #fafafa; }
  .note { margin-top: 16px; color: #6b7280; font-size: 11px; }
  @media print { body { margin: 12mm; } .sql { break-inside: avoid; } thead { display: table-header-group; } tr { break-inside: avoid; } }
</style>
</head>
<body>
<h1>${escapeHtml(dataset.name)}</h1>
<p class="meta">${escapeHtml(dataset.connectionName)} · ${escapeHtml(datasetScopeLabel(dataset) || "—")} · ${escapeHtml(dataset.driver)} · 生成于 ${escapeHtml(generated)}</p>
<div class="sql">${escapeHtml(dataset.sql)}</div>
<p class="meta">快照 ${dataset.rows.length} 行 · 原始结果 ${dataset.rowCount} 行${dataset.truncated ? " · 已截断" : ""}</p>
<table>
<thead><tr>${dataset.columns.map((column) => `<th>${escapeHtml(column)}</th>`).join("")}</tr></thead>
<tbody>${rows}</tbody>
</table>
<p class="note">这是 mydb 保存的数据快照报表，不代表数据库当前状态；如需最新数据，请打开数据集 SQL 重新执行。</p>
</body>
</html>`;
}
