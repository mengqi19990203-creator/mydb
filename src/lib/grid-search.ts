export interface GridSearchColumn {
  /** Index into each row. */
  index: number;
}

export interface GridMatch {
  rowIndex: number;
  columnIndex: number;
  start: number;
  end: number;
}

export interface GridHighlightSegment {
  text: string;
  match: boolean;
}

/** Formats a cell exactly like the result / table grids display it. */
export function formatGridCell(value: unknown): string {
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

/**
 * Finds literal substring matches in already-loaded rows. Columns are searched
 * in the order given; matches are returned row-first, then column, then by
 * position inside the cell.
 */
export function findGridMatches(
  rows: readonly (readonly unknown[])[],
  columns: readonly GridSearchColumn[],
  query: string,
  options: { caseSensitive?: boolean } = {},
): GridMatch[] {
  if (!query.trim() || !columns.length) return [];
  const needle = options.caseSensitive ? query : query.toLowerCase();
  const matches: GridMatch[] = [];
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex];
    for (const column of columns) {
      const haystack = formatGridCell(row[column.index]);
      const text = options.caseSensitive ? haystack : haystack.toLowerCase();
      let from = 0;
      while (from <= text.length - needle.length) {
        const start = text.indexOf(needle, from);
        if (start < 0) break;
        matches.push({
          rowIndex,
          columnIndex: column.index,
          start,
          end: start + needle.length,
        });
        from = start + Math.max(1, needle.length);
      }
    }
  }
  return matches;
}

/** Splits already formatted text into plain / matching segments. The caller
 * renders these as spans, avoiding raw HTML insertion. */
export function highlightSegments(
  text: string,
  query: string,
  caseSensitive = false,
): GridHighlightSegment[] {
  if (!query.trim()) return [{ text, match: false }];
  const haystack = caseSensitive ? text : text.toLowerCase();
  const needle = caseSensitive ? query : query.toLowerCase();
  if (!needle) return [{ text, match: false }];
  const segments: GridHighlightSegment[] = [];
  let from = 0;
  while (from <= haystack.length - needle.length) {
    const start = haystack.indexOf(needle, from);
    if (start < 0) break;
    if (start > from) {
      segments.push({ text: text.slice(from, start), match: false });
    }
    segments.push({
      text: text.slice(start, start + needle.length),
      match: true,
    });
    from = start + needle.length;
  }
  if (from < text.length) segments.push({ text: text.slice(from), match: false });
  return segments.length ? segments : [{ text, match: false }];
}

export function countMatchRows(matches: readonly GridMatch[]): number {
  const rows = new Set<number>();
  for (const match of matches) rows.add(match.rowIndex);
  return rows.size;
}