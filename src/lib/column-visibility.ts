export type AutoHideMode = "off" | "null" | "null-empty";

export interface ColumnVisibilityColumn {
  key: string;
  name: string;
  index: number;
  comment?: string | null;
}

export interface EmptyColumnOptions {
  mode: AutoHideMode;
  maxRows?: number;
  maxColumns?: number;
}

export const MAX_AUTO_HIDE_ROWS = 10_000;
export const MAX_AUTO_HIDE_COLUMNS = 500;

export function columnMatches(
  column: ColumnVisibilityColumn,
  query: string,
): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return true;
  return (
    column.name.toLocaleLowerCase().includes(needle) ||
    (column.comment ?? "").toLocaleLowerCase().includes(needle)
  );
}

/**
 * Returns the keys of columns whose checked values are all empty.
 * Results are based on loaded rows only; callers decide whether those rows are
 * filtered or include pending table edits.
 */
export function findEmptyColumnKeys(
  rows: ReadonlyArray<ReadonlyArray<unknown>>,
  columns: ReadonlyArray<ColumnVisibilityColumn>,
  options: EmptyColumnOptions,
): Set<string> {
  const result = new Set<string>();
  if (options.mode === "off" || !rows.length || !columns.length) return result;
  const rowLimit = Math.min(
    rows.length,
    options.maxRows ?? MAX_AUTO_HIDE_ROWS,
  );
  const columnLimit = Math.min(
    columns.length,
    options.maxColumns ?? MAX_AUTO_HIDE_COLUMNS,
  );
  for (let columnPosition = 0; columnPosition < columnLimit; columnPosition += 1) {
    const column = columns[columnPosition];
    let empty = true;
    for (let rowIndex = 0; rowIndex < rowLimit; rowIndex += 1) {
      const value = rows[rowIndex]?.[column.index];
      if (value === null || value === undefined) continue;
      if (options.mode === "null-empty" && value === "") continue;
      empty = false;
      break;
    }
    if (empty) result.add(column.key);
  }
  return result;
}

export function autoHiddenKeys(
  columns: ReadonlyArray<ColumnVisibilityColumn>,
  emptyKeys: ReadonlySet<string>,
  exceptions: ReadonlySet<string>,
): Set<string> {
  const result = new Set<string>();
  for (const column of columns) {
    if (emptyKeys.has(column.key) && !exceptions.has(column.key)) {
      result.add(column.key);
    }
  }
  return result;
}

/** Removes automatic hiding from the first visible column when every column
 * would otherwise be hidden. Manual hiding is intentionally not changed. */
export function keepOneAutoVisible(
  columns: ReadonlyArray<ColumnVisibilityColumn>,
  manualHidden: ReadonlySet<string>,
  autoHidden: Set<string>,
): boolean {
  const remaining = columns.filter(
    (column) => !manualHidden.has(column.key) && !autoHidden.has(column.key),
  );
  if (remaining.length) return false;
  const fallback = columns.find((column) => !manualHidden.has(column.key));
  if (!fallback) return false;
  return autoHidden.delete(fallback.key);
}
