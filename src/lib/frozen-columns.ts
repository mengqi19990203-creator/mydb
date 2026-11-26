export const FROZEN_COLUMNS_KEY = "mydb.frozenColumns.v1";
const MAX_ENTRIES = 200;

interface StoredFrozenColumns {
  version: 1;
  entries: Array<{ key: string; names: string[] }>;
}

export function frozenScopeKey(
  driver: string,
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
  table: string,
): string {
  return [
    driver,
    connectionId,
    database?.trim() ?? "",
    schema?.trim() ?? "",
    table,
  ].join("\u0001");
}

function readStore(): StoredFrozenColumns {
  try {
    const raw = window.localStorage.getItem(FROZEN_COLUMNS_KEY);
    if (!raw) return { version: 1, entries: [] };
    const parsed = JSON.parse(raw) as Partial<StoredFrozenColumns>;
    if (!parsed || parsed.version !== 1 || !Array.isArray(parsed.entries)) {
      return { version: 1, entries: [] };
    }
    const entries: StoredFrozenColumns["entries"] = [];
    for (const entry of parsed.entries) {
      if (
        !entry ||
        typeof entry.key !== "string" ||
        !Array.isArray(entry.names) ||
        entry.names.some((name) => typeof name !== "string")
      ) {
        continue;
      }
      entries.push({ key: entry.key, names: [...entry.names] });
    }
    return { version: 1, entries: entries.slice(0, MAX_ENTRIES) };
  } catch {
    return { version: 1, entries: [] };
  }
}

function writeStore(store: StoredFrozenColumns): void {
  try {
    window.localStorage.setItem(
      FROZEN_COLUMNS_KEY,
      JSON.stringify({
        version: 1,
        entries: store.entries.slice(-MAX_ENTRIES),
      }),
    );
  } catch {
    // Storage may be unavailable; frozen columns still work in this run.
  }
}

export function loadFrozenNames(key: string): string[] {
  const entry = readStore().entries.find((item) => item.key === key);
  return entry ? [...entry.names] : [];
}

export function saveFrozenNames(key: string, names: string[]): void {
  const store = readStore();
  const next = store.entries.filter((entry) => entry.key !== key);
  if (names.length) {
    next.push({ key, names: [...names] });
  }
  writeStore({ version: 1, entries: next });
}

/** Keeps only names that still exist, in column order. */
export function normalizeFrozenNames(
  names: readonly string[],
  columns: readonly string[],
): string[] {
  const wanted = new Set(names);
  return columns.filter((name) => wanted.has(name));
}

/** Sticky `left` offsets for frozen columns, starting after lead columns. */
export function frozenOffsets(
  widths: readonly number[],
  indices: readonly number[],
  base: number,
): Record<number, number> {
  const offsets: Record<number, number> = {};
  let left = base;
  for (const index of indices) {
    offsets[index] = left;
    left += widths[index] ?? 0;
  }
  return offsets;
}