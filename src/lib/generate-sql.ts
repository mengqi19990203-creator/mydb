import type { ColumnInfo, Driver } from "./types";

export type GeneratedSqlKind = "select" | "insert" | "update" | "delete" | "ddl";

function quote(name: string, driver: Driver): string {
  if (driver === "mysql") {
    return `\`${name.replaceAll("`", "``")}\``;
  }
  return `"${name.replaceAll('"', '""')}"`;
}

/** SQL-qualified name: MySQL uses database.table, PostgreSQL schema.table. */
export function qualifiedSqlName(
  driver: Driver,
  table: string,
  database?: string,
  schema?: string | null,
): string {
  if (driver === "postgres") {
    return schema
      ? `${quote(schema, driver)}.${quote(table, driver)}`
      : quote(table, driver);
  }
  if (driver === "mysql") {
    return database
      ? `${quote(database, driver)}.${quote(table, driver)}`
      : quote(table, driver);
  }
  return quote(table, driver);
}

/** Quoted qualified name for copying to the clipboard. */
export function qualifiedDisplayName(
  driver: Driver,
  table: string,
  database?: string,
  schema?: string | null,
): string {
  if (driver === "postgres") {
    return schema
      ? `${quote(schema, driver)}.${quote(table, driver)}`
      : quote(table, driver);
  }
  return database
    ? `${quote(database, driver)}.${quote(table, driver)}`
    : quote(table, driver);
}

function placeholder(driver: Driver, position: number): string {
  return driver === "postgres" ? `$${position}` : "?";
}

export function buildGeneratedSql(
  kind: Exclude<GeneratedSqlKind, "ddl">,
  driver: Driver,
  table: string,
  columns: ColumnInfo[],
  database?: string,
  schema?: string | null,
): string {
  const tableName = qualifiedSqlName(driver, table, database, schema);

  if (kind === "select") {
    return `SELECT *\nFROM ${tableName}\nLIMIT 100;`;
  }

  const primaryKeys = columns.filter((column) => column.primaryKey);

  if (kind === "insert") {
    const targets = columns.filter((column) => !column.autoIncrement);
    if (!targets.length) {
      return driver === "mysql"
        ? `INSERT INTO ${tableName} () VALUES ();`
        : `INSERT INTO ${tableName}\nDEFAULT VALUES;`;
    }
    const names = targets
      .map((column) => quote(column.name, driver))
      .join(", ");
    const values = targets
      .map((_, index) => placeholder(driver, index + 1))
      .join(", ");
    const required = targets.filter(
      (column) =>
        !column.nullable &&
        !column.defaultValue?.trim() &&
        !column.primaryKey,
    );
    const comment = required.length
      ? `-- 必填字段：${required.map((column) => column.name).join(", ")}\n`
      : "";
    return `${comment}INSERT INTO ${tableName} (${names})\nVALUES (${values});`;
  }

  if (!primaryKeys.length) {
    throw new Error("该表没有主键，无法生成安全的 UPDATE/DELETE");
  }

  if (kind === "update") {
    const updates = columns.filter(
      (column) => !column.primaryKey && !column.autoIncrement,
    );
    const sets = updates
      .map(
        (column, index) =>
          `${quote(column.name, driver)} = ${placeholder(
            driver,
            index + 1,
          )}`,
      )
      .join(",\n  ");
    const condition = primaryKeys
      .map(
        (column, index) =>
          `${quote(column.name, driver)} = ${placeholder(
            driver,
            updates.length + index + 1,
          )}`,
      )
      .join(" AND ");
    return `UPDATE ${tableName}\nSET ${
      sets || "/* 请填写要更新的列 */"
    }\nWHERE ${condition};`;
  }

  const condition = primaryKeys
    .map(
      (column, index) =>
        `${quote(column.name, driver)} = ${placeholder(driver, index + 1)}`,
    )
    .join(" AND ");
  return `DELETE FROM ${tableName}\nWHERE ${condition};`;
}