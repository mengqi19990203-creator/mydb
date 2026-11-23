import type { ColumnInfo, Driver } from "./types";
import { quoteIdentifier, sqlLiteral } from "./sql-literal";

interface IndexedColumn {
  column: ColumnInfo;
  index: number;
}

function indexedColumns(columns: ColumnInfo[]): IndexedColumn[] {
  return columns.map((column, index) => ({ column, index }));
}

/** Builds a single-row INSERT; auto-increment columns are omitted. */
export function buildRowInsert(
  columns: ColumnInfo[],
  row: unknown[],
  driver: Driver,
  qualifiedTable: string,
): string {
  const targets = indexedColumns(columns).filter(
    (item) => !item.column.autoIncrement,
  );
  if (!targets.length) {
    return driver === "mysql"
      ? `INSERT INTO ${qualifiedTable} () VALUES ();`
      : `INSERT INTO ${qualifiedTable} DEFAULT VALUES;`;
  }
  const names = targets
    .map((item) => quoteIdentifier(item.column.name, driver))
    .join(", ");
  const values = targets
    .map((item) => sqlLiteral(row[item.index], driver))
    .join(", ");
  return `INSERT INTO ${qualifiedTable} (${names}) VALUES (${values});`;
}

/**
 * Builds a single-row UPDATE using every primary key column in WHERE.
 * Returns null when the table has no primary key or nothing is updatable.
 */
export function buildRowUpdate(
  columns: ColumnInfo[],
  row: unknown[],
  driver: Driver,
  qualifiedTable: string,
): string | null {
  const indexed = indexedColumns(columns);
  const primaryKeys = indexed.filter((item) => item.column.primaryKey);
  const updates = indexed.filter(
    (item) => !item.column.primaryKey && !item.column.autoIncrement,
  );
  if (!primaryKeys.length || !updates.length) return null;

  const sets = updates
    .map(
      (item) =>
        `${quoteIdentifier(item.column.name, driver)} = ${sqlLiteral(
          row[item.index],
          driver,
        )}`,
    )
    .join(",\n  ");
  const condition = primaryKeys
    .map((item) => {
      const identifier = quoteIdentifier(item.column.name, driver);
      const value = row[item.index];
      return value === null || value === undefined
        ? `${identifier} IS NULL`
        : `${identifier} = ${sqlLiteral(value, driver)}`;
    })
    .join(" AND ");

  return `UPDATE ${qualifiedTable}\nSET ${sets}\nWHERE ${condition};`;
}

/** Builds a single pretty-printed JSON object for one row. */
export function buildRowJson(
  columnNames: string[],
  values: unknown[],
): string {
  const object: Record<string, unknown> = {};
  columnNames.forEach((name, index) => {
    object[name] = values[index] ?? null;
  });
  return JSON.stringify(object, null, 2);
}