import { tableViewScopeKey } from "./table-views";

export const TABLE_COLUMN_ORDER_KEY = "mydb.tableColumnOrder.v1";
export const MAX_ORDERED_TABLES = 200;
export const MAX_ORDERED_COLUMNS = 500;

interface StoredEntry {
  order: string[];
  updatedAt: number;
}

interface StoredOrders {
  version: 1;
  entries: Record<string, StoredEntry>;
}

export function columnOrderScopeKey(
  driver: string,
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
  table: string,
  tableKind: string,
): string {
  return tableViewScopeKey(
    driver,
    connectionId,
    database,
    schema,
    table,
    tableKind,
  );
}

function emptyStore(): StoredOrders {
  return { version: 1, entries: {} };
}

function readStore(): StoredOrders {
  if (typeof window === "undefined") return emptyStore();
  let raw: string | null = null;
  try {
    raw = window.localStorage.getItem(TABLE_COLUMN_ORDER_KEY);
    if (!raw) return emptyStore();
    const parsed = JSON.parse(raw) as Partial<StoredOrders>;
    if (
      !parsed ||
      parsed.version !== 1 ||
      !parsed.entries ||
      typeof parsed.entries !== "object" ||
      Array.isArray(parsed.entries)
    ) {
      throw new Error("invalid column order store");
    }
    const entries: Record<string, StoredEntry> = {};
    for (const [key, value] of Object.entries(parsed.entries)) {
      if (!value || typeof value !== "object") continue;
      const entry = value as Partial<StoredEntry>;
      if (!Array.isArray(entry.order)) continue;
      const order = entry.order.filter(
        (item): item is string => typeof item === "string" && Boolean(item),
      );
      if (!order.length) continue;
      entries[key] = {
        order: [...new Set(order)].slice(0, MAX_ORDERED_COLUMNS),
        updatedAt:
          typeof entry.updatedAt === "number" && Number.isFinite(entry.updatedAt)
            ? entry.updatedAt
            : 0,
      };
    }
    const limited = Object.entries(entries)
      .sort(([, left], [, right]) => right.updatedAt - left.updatedAt)
      .slice(0, MAX_ORDERED_TABLES);
    return { version: 1, entries: Object.fromEntries(limited) };
  } catch {
    if (raw) {
      try {
        window.localStorage.removeItem(TABLE_COLUMN_ORDER_KEY);
      } catch {
        // Best effort; a corrupt store must not block the table.
      }
    }
    return emptyStore();
  }
}

export function loadColumnOrder(scope: string): string[] {
  const entry = readStore().entries[scope];
  return entry ? [...entry.order] : [];
}

export function saveColumnOrder(scope: string, order: string[]): boolean {
  if (typeof window === "undefined") return false;
  const store = readStore();
  const cleaned = [...new Set(order.filter(Boolean))].slice(
    0,
    MAX_ORDERED_COLUMNS,
  );
  const entries = { ...store.entries };
  if (cleaned.length) {
    entries[scope] = { order: cleaned, updatedAt: Date.now() };
  } else {
    delete entries[scope];
  }
  const limited = Object.entries(entries)
    .sort(([, left], [, right]) => right.updatedAt - left.updatedAt)
    .slice(0, MAX_ORDERED_TABLES);
  try {
    window.localStorage.setItem(
      TABLE_COLUMN_ORDER_KEY,
      JSON.stringify({
        version: 1,
        entries: Object.fromEntries(limited),
      }),
    );
    return true;
  } catch {
    return false;
  }
}

/** Keeps known keys, removes duplicates, appends new keys in input order. */
export function normalizeColumnOrder(
  order: readonly string[],
  keys: readonly string[],
): string[] {
  const known = new Set(keys);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const key of order) {
    if (!known.has(key) || seen.has(key)) continue;
    seen.add(key);
    result.push(key);
    if (result.length >= MAX_ORDERED_COLUMNS) break;
  }
  for (const key of keys) {
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(key);
    if (result.length >= MAX_ORDERED_COLUMNS) break;
  }
  return result;
}

/** Applies a stored order to current keys; missing columns go to the end. */
export function applyColumnOrder(
  keys: readonly string[],
  order: readonly string[],
): string[] {
  return normalizeColumnOrder(order, keys);
}

export function moveColumnKey(
  order: readonly string[],
  from: string,
  to: string,
  after = false,
): string[] {
  if (from === to) return [...order];
  const next = [...order];
  const fromIndex = next.indexOf(from);
  const toIndex = next.indexOf(to);
  if (fromIndex < 0 || toIndex < 0) return next;
  const [item] = next.splice(fromIndex, 1);
  const target = next.indexOf(to);
  next.splice(target + (after ? 1 : 0), 0, item);
  return next;
}

/** Keeps valid unique indexes and appends the rest. */
export function orderIndices(
  order: readonly number[],
  count: number,
): number[] {
  const seen = new Set<number>();
  const result: number[] = [];
  for (const index of order) {
    if (!Number.isInteger(index) || index < 0 || index >= count) continue;
    if (seen.has(index)) continue;
    seen.add(index);
    result.push(index);
  }
  for (let index = 0; index < count; index += 1) {
    if (!seen.has(index)) result.push(index);
  }
  return result;
}

/** Maps a previous index order to a new result set with the same columns. */
export function reorderByName(
  order: readonly number[],
  previousColumns: readonly string[],
  nextColumns: readonly string[],
): number[] {
  const sameShape =
    previousColumns.length === nextColumns.length &&
    previousColumns.every((name, index) => name === nextColumns[index]);
  if (sameShape) return orderIndices(order, nextColumns.length);
  const sameSet =
    previousColumns.length === nextColumns.length &&
    [...previousColumns].sort().join("\u0001") ===
      [...nextColumns].sort().join("\u0001");
  if (!sameSet) return nextColumns.map((_, index) => index);
  const used = new Set<number>();
  const mapped: number[] = [];
  for (const previousIndex of order) {
    const name = previousColumns[previousIndex];
    const nextIndex = nextColumns.findIndex(
      (candidate, index) => candidate === name && !used.has(index),
    );
    if (nextIndex < 0) continue;
    used.add(nextIndex);
    mapped.push(nextIndex);
  }
  for (let index = 0; index < nextColumns.length; index += 1) {
    if (!used.has(index)) mapped.push(index);
  }
  return mapped;
}
