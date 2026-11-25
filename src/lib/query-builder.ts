import { quoteIdentifier, sqlLiteral } from "./sql-literal";
import type { Driver, ErDiagram, ErRelation } from "./types";

export interface BuilderTable {
  /** Real table / view name. */
  name: string;
  alias: string;
  kind: string;
}

export interface BuilderJoin {
  leftAlias: string;
  leftColumn: string;
  rightAlias: string;
  rightColumn: string;
  type: "INNER" | "LEFT";
}

export interface BuilderFilter {
  alias: string;
  column: string;
  operator:
    | "="
    | "<>"
    | ">"
    | ">="
    | "<"
    | "<="
    | "LIKE"
    | "NOT LIKE"
    | "IS NULL"
    | "IS NOT NULL";
  value: string;
}

export interface BuilderSort {
  alias: string;
  column: string;
  direction: "ASC" | "DESC";
}

export interface BuilderState {
  tables: BuilderTable[];
  /** Keys are `alias.column`. */
  selected: string[];
  joins: BuilderJoin[];
  filters: BuilderFilter[];
  sorts: BuilderSort[];
  limit: number;
}

export interface BuilderSql {
  sql: string;
  warnings: string[];
  emptyReason: string;
}

export function nullOperator(operator: string): boolean {
  return operator === "IS NULL" || operator === "IS NOT NULL";
}

export function aliasIssue(
  alias: string,
  tables: BuilderTable[],
  exceptName?: string,
): string {
  const trimmed = alias.trim();
  if (!trimmed) return "别名不能为空";
  if (
    tables.some(
      (table) => table.name !== exceptName && table.alias === trimmed,
    )
  ) {
    return "别名已被其他表使用";
  }
  return "";
}

export function defaultAlias(name: string, tables: BuilderTable[]): string {
  const used = new Set(tables.map((table) => table.alias));
  if (!used.has(name)) return name;
  let index = 2;
  while (used.has(`${name}_${index}`)) index += 1;
  return `${name}_${index}`;
}

export function addTable(
  state: BuilderState,
  name: string,
  kind: string,
  diagram: ErDiagram | null,
): BuilderJoin[] {
  const alias = defaultAlias(name, state.tables);
  state.tables.push({ name, alias, kind });
  if (state.tables.length === 1 || !diagram) return [];
  const previous = new Set(
    state.tables.slice(0, -1).map((table) => table.name),
  );
  const suggestions: BuilderJoin[] = [];
  for (const relation of diagram.relations) {
    let leftAlias = "";
    let leftColumn = "";
    let rightAlias = "";
    let rightColumn = "";
    if (relation.fromTable === name && previous.has(relation.toTable)) {
      leftAlias =
        state.tables.find((table) => table.name === relation.toTable)?.alias ?? "";
      leftColumn = relation.toColumns[0] ?? "";
      rightAlias = alias;
      rightColumn = relation.fromColumns[0] ?? "";
    } else if (relation.toTable === name && previous.has(relation.fromTable)) {
      leftAlias =
        state.tables.find((table) => table.name === relation.fromTable)?.alias ?? "";
      leftColumn = relation.fromColumns[0] ?? "";
      rightAlias = alias;
      rightColumn = relation.toColumns[0] ?? "";
    }
    if (leftAlias && leftColumn && rightColumn) {
      suggestions.push({
        leftAlias,
        leftColumn,
        rightAlias,
        rightColumn,
        type: "INNER",
      });
    }
  }
  return suggestions;
}

export function relationFor(
  diagram: ErDiagram | null,
  name: string,
  previous: string[],
): ErRelation | null {
  if (!diagram) return null;
  return (
    diagram.relations.find(
      (relation) =>
        (relation.fromTable === name && previous.includes(relation.toTable)) ||
        (relation.toTable === name && previous.includes(relation.fromTable)),
    ) ?? null
  );
}

export function buildQuery(
  state: BuilderState,
  driver: Driver,
): BuilderSql {
  const warnings: string[] = [];
  if (!state.tables.length) {
    return { sql: "", warnings, emptyReason: "请先添加至少一张表" };
  }
  if (!state.selected.length) {
    return { sql: "", warnings, emptyReason: "请勾选至少一列" };
  }
  const aliases = new Map(state.tables.map((table) => [table.alias, table]));
  const duplicateAliases = new Set<string>();
  for (const table of state.tables) {
    if (duplicateAliases.has(table.alias)) {
      warnings.push(`别名 ${table.alias} 重复`);
    }
    duplicateAliases.add(table.alias);
  }

  const selected = state.selected
    .map((key) => {
      const [alias, ...rest] = key.split(".");
      const column = rest.join(".");
      return { alias, column };
    })
    .filter(
      (item) => aliases.has(item.alias) && item.column.length > 0,
    );
  if (!selected.length) {
    return { sql: "", warnings, emptyReason: "请勾选至少一列" };
  }
  const columnCounts = new Map<string, number>();
  for (const item of selected) {
    columnCounts.set(item.column, (columnCounts.get(item.column) ?? 0) + 1);
  }
  if ([...columnCounts.values()].some((count) => count > 1)) {
    warnings.push("存在同名列，输出别名已加上表别名");
  }

  const selectList = selected
    .map((item) => {
      const expression = `${quoteIdentifier(item.alias, driver)}.${quoteIdentifier(
        item.column,
        driver,
      )}`;
      const output =
        (columnCounts.get(item.column) ?? 0) > 1
          ? `${item.alias}_${item.column}`
          : item.column;
      return `${expression} AS ${quoteIdentifier(output, driver)}`;
    })
    .join(",\n  ");

  const first = state.tables[0];
  const fromAlias = (alias: string) =>
    alias === aliases.get(alias)?.name
      ? quoteIdentifier(alias, driver)
      : `${quoteIdentifier(aliases.get(alias)?.name ?? alias, driver)} AS ${quoteIdentifier(
          alias,
          driver,
        )}`;
  const fromClause = `FROM ${fromAlias(first.alias)}`;

  const joinClauses: string[] = [];
  for (let index = 1; index < state.tables.length; index += 1) {
    const table = state.tables[index];
    const previous = new Set(
      state.tables.slice(0, index).map((item) => item.alias),
    );
    const conditions = state.joins.filter(
      (join) =>
        (join.rightAlias === table.alias && previous.has(join.leftAlias)) ||
        (join.leftAlias === table.alias && previous.has(join.rightAlias)),
    );
    if (!conditions.length) {
      joinClauses.push(`CROSS JOIN ${fromAlias(table.alias)}`);
      warnings.push(
        `表 ${table.alias} 未与之前的表连接，已生成 CROSS JOIN（笛卡尔积风险）`,
      );
      continue;
    }
    const type = conditions.some((join) => join.type === "LEFT")
      ? "LEFT JOIN"
      : "INNER JOIN";
    const on = conditions
      .map((join) => {
        const left =
          join.leftAlias === table.alias
            ? join.rightAlias
            : join.leftAlias;
        const leftColumn =
          join.leftAlias === table.alias
            ? join.rightColumn
            : join.leftColumn;
        const rightColumn =
          join.leftAlias === table.alias
            ? join.leftColumn
            : join.rightColumn;
        return `${quoteIdentifier(left, driver)}.${quoteIdentifier(
          leftColumn,
          driver,
        )} = ${quoteIdentifier(table.alias, driver)}.${quoteIdentifier(
          rightColumn,
          driver,
        )}`;
      })
      .join(" AND ");
    joinClauses.push(`${type} ${fromAlias(table.alias)} ON ${on}`);
  }

  const where = state.filters
    .filter((filter) => aliases.has(filter.alias) && filter.column)
    .map((filter) => {
      const identifier = `${quoteIdentifier(filter.alias, driver)}.${quoteIdentifier(
        filter.column,
        driver,
      )}`;
      if (nullOperator(filter.operator)) {
        return `${identifier} ${filter.operator}`;
      }
      return `${identifier} ${filter.operator} ${sqlLiteral(
        filter.value,
        driver,
      )}`;
    });
  if (where.length) warnings.push(`${where.length} 个筛选条件`);

  const sorts = state.sorts
    .filter((sort) => aliases.has(sort.alias) && sort.column)
    .slice(0, 3)
    .map(
      (sort) =>
        `${quoteIdentifier(sort.alias, driver)}.${quoteIdentifier(
          sort.column,
          driver,
        )} ${sort.direction}`,
    );
  if (!sorts.length) warnings.push("未设置排序，结果顺序未定义");

  const limit = Math.max(1, Math.min(10_000, Math.floor(state.limit) || 100));
  const lines = [
    `SELECT\n  ${selectList}`,
    fromClause,
    ...joinClauses,
    ...(where.length ? [`WHERE ${where.join(" AND ")}`] : []),
    ...(sorts.length ? [`ORDER BY ${sorts.join(", ")}`] : []),
    `LIMIT ${limit};`,
  ];
  return { sql: lines.join("\n"), warnings, emptyReason: "" };
}