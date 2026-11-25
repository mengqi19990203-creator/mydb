import type { ColumnInfo, Driver } from "./types";

export interface ColumnDraft {
  name: string;
  type: string;
  nullable: boolean;
  defaultValue: string;
  comment?: string;
}

export interface IndexDraft {
  name: string;
  columns: string[];
  unique: boolean;
}

function quote(name: string, driver: Driver): string {
  if (driver === "mysql") {
    return `\`${name.replaceAll("`", "``")}\``;
  }
  return `"${name.replaceAll('"', '""')}"`;
}

function normalizedDefault(value: string | null | undefined): string {
  return (value ?? "").trim();
}

function sqlCommentLiteral(value: string | null | undefined): string {
  return `'${(value ?? "").replaceAll("\\", "\\\\").replaceAll("'", "''")}'`;
}

/**
 * MySQL's information_schema.COLUMN_DEFAULT returns string defaults without
 * quotes (for example `abc` instead of `'abc'`). Re-emitting them verbatim in
 * CHANGE COLUMN would produce invalid SQL, so turn the metadata value into a
 * valid literal when it does not already look like one.
 */
function mysqlDefaultLiteral(value: string, dataType: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (
    /^(NULL|CURRENT_TIMESTAMP(\(\d*\))?|CURRENT_DATE|CURRENT_TIME|LOCALTIME(STAMP)?(\(\d*\))?|NOW\(\)|UUID\(\))$/i.test(
      trimmed,
    )
  ) {
    return trimmed;
  }
  if (/^'.*'$/s.test(trimmed) || /^b'[01]+'$/i.test(trimmed) || /^\(.*\)$/s.test(trimmed)) {
    return trimmed;
  }

  const numericType =
    /^(TINYINT|SMALLINT|MEDIUMINT|INT|INTEGER|BIGINT|DECIMAL|NUMERIC|FLOAT|DOUBLE|REAL|BIT|BOOL|BOOLEAN)/i.test(
      dataType,
    );
  if (numericType && /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(trimmed)) {
    return trimmed;
  }

  return `'${trimmed.replaceAll("\\", "\\\\").replaceAll("'", "''")}'`;
}

/**
 * Builds the statements for adding a column (`existing === null`) or editing
 * one. Statements are returned in execution order; an empty array means the
 * draft matches the current definition.
 */
export function buildColumnStatements(
  driver: Driver,
  table: string,
  existing: ColumnInfo | null,
  draft: ColumnDraft,
): string[] {
  const tableRef = quote(table, driver);
  const name = draft.name.trim();
  const type = draft.type.trim();
  if (!name) throw new Error("字段名不能为空");
  if (!type) throw new Error("字段类型不能为空");

  if (!existing) {
    if (driver === "sqlite" && !draft.nullable && !draft.defaultValue.trim()) {
      throw new Error("SQLite 添加 NOT NULL 字段时必须提供默认值");
    }
    let sql = `ALTER TABLE ${tableRef} ADD COLUMN ${quote(name, driver)} ${type}`;
    if (!draft.nullable) sql += " NOT NULL";
    if (draft.defaultValue.trim()) sql += ` DEFAULT ${draft.defaultValue.trim()}`;
    const statements = [`${sql};`];
    if (driver === "mysql" && draft.comment?.trim()) {
      statements[0] = `${sql} COMMENT ${sqlCommentLiteral(
        draft.comment,
      )};`;
    } else if (driver === "postgres" && draft.comment?.trim()) {
      statements.push(
        `COMMENT ON COLUMN ${tableRef}.${quote(name, driver)} IS ${sqlCommentLiteral(
          draft.comment,
        )};`,
      );
    }
    return statements;
  }

  const renamed = name !== existing.name;
  const typeChanged = type !== existing.dataType.trim();
  const nullableChanged = draft.nullable !== existing.nullable;
  const newDefaultRaw = draft.defaultValue.trim();
  const oldDefaultRaw = normalizedDefault(existing.defaultValue);
  const defaultChanged =
    driver === "mysql"
      ? mysqlDefaultLiteral(newDefaultRaw, type) !==
        mysqlDefaultLiteral(oldDefaultRaw, existing.dataType)
      : newDefaultRaw !== oldDefaultRaw;
  const newComment = draft.comment?.trim() ?? "";
  const oldComment = (existing.comment ?? "").trim();
  const commentChanged = driver !== "sqlite" && newComment !== oldComment;

  if (
    !renamed &&
    !typeChanged &&
    !nullableChanged &&
    !defaultChanged &&
    !commentChanged
  ) {
    return [];
  }

  if (driver === "sqlite") {
    if (typeChanged || nullableChanged || defaultChanged) {
      throw new Error(
        "SQLite 只支持重命名字段，修改类型、可空性或默认值需要重建表",
      );
    }
    return [
      `ALTER TABLE ${tableRef} RENAME COLUMN ${quote(
        existing.name,
        driver,
      )} TO ${quote(name, driver)};`,
    ];
  }

  if (driver === "mysql") {
    let sql = `ALTER TABLE ${tableRef} CHANGE COLUMN ${quote(
      existing.name,
      driver,
    )} ${quote(name, driver)} ${type}`;
    sql += draft.nullable ? " NULL" : " NOT NULL";
    if (newDefaultRaw) {
      sql += ` DEFAULT ${mysqlDefaultLiteral(newDefaultRaw, type)}`;
    }
    if (existing.autoIncrement) sql += " AUTO_INCREMENT";
    if (newComment || oldComment) {
      sql += ` COMMENT ${sqlCommentLiteral(newComment)}`;
    }
    return [`${sql};`];
  }

  // PostgreSQL: rename must be its own statement, the rest can follow it.
  const statements: string[] = [];
  let current = existing.name;
  if (renamed) {
    statements.push(
      `ALTER TABLE ${tableRef} RENAME COLUMN ${quote(
        existing.name,
        driver,
      )} TO ${quote(name, driver)};`,
    );
    current = name;
  }
  if (typeChanged) {
    statements.push(
      `ALTER TABLE ${tableRef} ALTER COLUMN ${quote(
        current,
        driver,
      )} TYPE ${type} USING ${quote(current, driver)}::${type};`,
    );
  }
  if (nullableChanged) {
    statements.push(
      `ALTER TABLE ${tableRef} ALTER COLUMN ${quote(current, driver)} ${
        draft.nullable ? "DROP NOT NULL" : "SET NOT NULL"
      };`,
    );
  }
  if (defaultChanged) {
    statements.push(
      newDefaultRaw
        ? `ALTER TABLE ${tableRef} ALTER COLUMN ${quote(
            current,
            driver,
          )} SET DEFAULT ${newDefaultRaw};`
        : `ALTER TABLE ${tableRef} ALTER COLUMN ${quote(
            current,
            driver,
          )} DROP DEFAULT;`,
    );
  }
  if (commentChanged) {
    statements.push(
      `COMMENT ON COLUMN ${tableRef}.${quote(
        current,
        driver,
      )} IS ${sqlCommentLiteral(newComment)};`,
    );
  }
  return statements;
}

export function buildCreateIndexStatement(
  driver: Driver,
  table: string,
  draft: IndexDraft,
): string {
  const name = draft.name.trim();
  if (!name) throw new Error("索引名不能为空");
  if (!draft.columns.length) throw new Error("至少选择一个索引列");
  const columns = draft.columns
    .map((column) => quote(column, driver))
    .join(", ");
  return `CREATE ${draft.unique ? "UNIQUE " : ""}INDEX ${quote(
    name,
    driver,
  )} ON ${quote(table, driver)} (${columns});`;
}

export function buildDropIndexStatement(
  driver: Driver,
  table: string,
  indexName: string,
): string {
  if (driver === "mysql") {
    return `DROP INDEX ${quote(indexName, driver)} ON ${quote(
      table,
      driver,
    )};`;
  }
  return `DROP INDEX ${quote(indexName, driver)};`;
}

export function buildDropColumnStatement(
  driver: Driver,
  table: string,
  column: string,
): string {
  return `ALTER TABLE ${quote(table, driver)} DROP COLUMN ${quote(
    column,
    driver,
  )};`;
}