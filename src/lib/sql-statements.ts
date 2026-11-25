export interface SqlStatementRange {
  start: number;
  end: number;
  text: string;
}

function skipQuoted(sql: string, start: number, quote: string): number {
  let index = start + 1;
  while (index < sql.length) {
    if (quote !== "`" && sql[index] === "\\") {
      index += 2;
      continue;
    }
    if (sql[index] === quote) {
      if (sql[index + 1] === quote) {
        index += 2;
        continue;
      }
      return index + 1;
    }
    index += 1;
  }
  return sql.length;
}

function dollarQuoteEnd(sql: string, start: number): number | null {
  if (sql[start] !== "$") return null;
  let index = start + 1;
  while (index < sql.length && /[A-Za-z0-9_]/.test(sql[index])) index += 1;
  if (sql[index] !== "$") return null;
  const tag = sql.slice(start, index + 1);
  const close = sql.indexOf(tag, index + 1);
  return close < 0 ? sql.length : close + tag.length;
}

/** Splits SQL for editor helpers such as EXPLAIN. The backend splitter used by
 * script execution remains the authoritative one for file execution. */
export function splitSqlStatements(sql: string): SqlStatementRange[] {
  const ranges: SqlStatementRange[] = [];
  let start = 0;
  let index = 0;
  while (index < sql.length) {
    const character = sql[index];
    if (character === "-" && sql[index + 1] === "-") {
      const end = sql.indexOf("\n", index);
      index = end < 0 ? sql.length : end + 1;
      continue;
    }
    if (character === "#") {
      const end = sql.indexOf("\n", index);
      index = end < 0 ? sql.length : end + 1;
      continue;
    }
    if (character === "/" && sql[index + 1] === "*") {
      const end = sql.indexOf("*/", index + 2);
      index = end < 0 ? sql.length : end + 2;
      continue;
    }
    if (character === "'" || character === '"' || character === "`") {
      index = skipQuoted(sql, index, character);
      continue;
    }
    if (character === "[") {
      const end = sql.indexOf("]", index + 1);
      index = end < 0 ? sql.length : end + 1;
      continue;
    }
    if (character === "$") {
      const end = dollarQuoteEnd(sql, index);
      if (end !== null) {
        index = end;
        continue;
      }
    }
    if (character === ";") {
      const text = sql.slice(start, index).trim();
      if (text) ranges.push({ start, end: index, text });
      start = index + 1;
    }
    index += 1;
  }
  const text = sql.slice(start).trim();
  if (text) ranges.push({ start, end: sql.length, text });
  return ranges;
}

export function statementAtCursor(sql: string, position: number): string {
  const exact = statementRangeAtCursor(sql, position);
  if (exact) return exact.text;
  const ranges = splitSqlStatements(sql);
  if (!ranges.length) return "";
  const previous = [...ranges]
    .reverse()
    .find((range) => range.end <= position);
  return previous?.text ?? ranges[0].text;
}

/** Exact statement containing `position`, without the fallback used by run. */
export function statementRangeAtCursor(
  sql: string,
  position: number,
): SqlStatementRange | null {
  return (
    splitSqlStatements(sql).find(
      (range) => position >= range.start && position <= range.end,
    ) ?? null
  );
}