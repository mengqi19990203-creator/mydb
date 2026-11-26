import { SQL_KEYWORDS, tokenizeSql, type SqlToken } from "./sql-check";
import { splitSqlStatements } from "./sql-statements";
import type { Driver, ScopeTableColumns } from "./types";

export interface SqlNavigationPayload {
  kind?: "table" | "column";
  table?: string;
  column?: string;
  tableKind?: string;
  from: number;
  to: number;
  reason?: string;
}

export interface ResolveSqlNavigationOptions {
  document: string;
  offset: number;
  tables: ScopeTableColumns[];
  driver: Driver;
  scopeInfo?: { database?: string | null; schema?: string | null };
}

interface NavRef {
  parts: SqlToken[];
  alias: SqlToken | null;
  table: ScopeTableColumns | null;
  tableFunction: boolean;
  cte: boolean;
  ambiguous: boolean;
  crossScope: boolean;
}

const ALIAS_STOP = new Set([
  ...SQL_KEYWORDS,
  "GROUP",
  "HAVING",
  "LIMIT",
  "OFFSET",
  "ON",
  "ORDER",
  "RETURNING",
  "SET",
  "UNION",
  "USING",
  "VALUES",
  "WHERE",
]);

function upper(token: SqlToken | undefined): string {
  return token?.kind === "word" ? token.text.toUpperCase() : "";
}

function isNameToken(token: SqlToken | undefined): boolean {
  return Boolean(token && (token.kind === "word" || token.kind === "quoted"));
}

function tokenKey(token: SqlToken): string {
  return token.quoted
    ? `q:${token.value}`
    : `u:${token.value.toLocaleLowerCase()}`;
}

function matchesName(token: SqlToken, name: string): boolean {
  if (token.quoted) return token.value === name;
  return token.value.toLocaleLowerCase() === name.toLocaleLowerCase();
}

function readQualified(
  tokens: SqlToken[],
  start: number,
): { parts: SqlToken[]; next: number } | null {
  if (!isNameToken(tokens[start])) return null;
  const parts = [tokens[start]];
  let index = start + 1;
  while (
    tokens[index]?.kind === "punct" &&
    tokens[index].text === "." &&
    isNameToken(tokens[index + 1])
  ) {
    parts.push(tokens[index + 1]);
    index += 2;
  }
  return { parts, next: index };
}

function localQualifier(
  qualifiers: string[],
  driver: Driver,
  scopeInfo: ResolveSqlNavigationOptions["scopeInfo"],
): boolean {
  const local = new Set(
    [scopeInfo?.database, scopeInfo?.schema]
      .filter((value): value is string => Boolean(value))
      .map((value) => value.toLocaleLowerCase()),
  );
  if (driver === "sqlite") {
    local.add("main");
    local.add("temp");
  }
  return qualifiers.every((qualifier) => local.has(qualifier));
}

function findTableByName(
  tables: ScopeTableColumns[],
  nameToken: SqlToken,
): { table?: ScopeTableColumns; reason?: string } {
  const matches = tables.filter((table) => matchesName(nameToken, table.name));
  if (!matches.length) {
    return { reason: `未在当前库 / 模式找到表 ${nameToken.value}。` };
  }
  const regular = matches.filter((table) => table.kind === "table");
  if (regular.length === 1) return { table: regular[0] };
  if (matches.length === 1) return { table: matches[0] };
  return { reason: `找到多个同名对象 ${nameToken.value}，无法确定要打开哪一个。` };
}

function readRef(
  tokens: SqlToken[],
  start: number,
  tables: ScopeTableColumns[],
  cteNames: Set<string>,
  driver: Driver,
  scopeInfo: ResolveSqlNavigationOptions["scopeInfo"],
): { ref: NavRef; next: number } | null {
  let position = start;
  while (upper(tokens[position]) === "ONLY") position += 1;
  const qualified = readQualified(tokens, position);
  if (!qualified) return null;
  let next = qualified.next;
  let alias: SqlToken | null = null;
  if (upper(tokens[next]) === "AS" && isNameToken(tokens[next + 1])) {
    alias = tokens[next + 1];
    next += 2;
  } else if (
    isNameToken(tokens[next]) &&
    !ALIAS_STOP.has(upper(tokens[next]))
  ) {
    alias = tokens[next];
    next += 1;
  }
  const tableFunction = tokens[next]?.text === "(";
  const nameToken = qualified.parts[qualified.parts.length - 1];
  const cte =
    qualified.parts.length === 1 && cteNames.has(tokenKey(nameToken));
  if (cte) {
    return {
      ref: {
        parts: qualified.parts,
        alias,
        table: null,
        tableFunction,
        cte: true,
        ambiguous: false,
        crossScope: false,
      },
      next,
    };
  }
  const qualifiers = qualified.parts
    .slice(0, -1)
    .map((part) => part.value.toLocaleLowerCase());
  if (qualified.parts.length > 1 && !localQualifier(qualifiers, driver, scopeInfo)) {
    return {
      ref: {
        parts: qualified.parts,
        alias,
        table: null,
        tableFunction,
        cte: false,
        ambiguous: false,
        crossScope: true,
      },
      next,
    };
  }
  const found = findTableByName(tables, nameToken);
  return {
    ref: {
      parts: qualified.parts,
      alias,
      table: found.table ?? null,
      tableFunction,
      cte: false,
      ambiguous: Boolean(found.reason?.startsWith("找到多个")),
      crossScope: false,
    },
    next,
  };
}

function readRefList(
  tokens: SqlToken[],
  start: number,
  tables: ScopeTableColumns[],
  cteNames: Set<string>,
  driver: Driver,
  scopeInfo: ResolveSqlNavigationOptions["scopeInfo"],
): { refs: NavRef[]; next: number } {
  const refs: NavRef[] = [];
  let cursor = start;
  let next = start;
  while (true) {
    const populated = readRef(tokens, cursor, tables, cteNames, driver, scopeInfo);
    if (!populated) break;
    refs.push(populated.ref);
    next = populated.next;
    if (tokens[next]?.kind !== "punct" || tokens[next].text !== ",") break;
    cursor = next + 1;
  }
  return { refs, next };
}

/** Collect select-list aliases so `SELECT 1 AS email` is not treated as a column. */
function collectSelectAliases(
  tokens: SqlToken[],
  structural: (index: number) => boolean,
): { names: Set<string>; positions: Set<number> } {
  const names = new Set<string>();
  const positions = new Set<number>();
  for (let index = 0; index < tokens.length; index += 1) {
    if (!structural(index)) continue;
    if (upper(tokens[index]) !== "AS" || !isNameToken(tokens[index + 1])) {
      continue;
    }
    const alias = tokens[index + 1];
    if (alias.kind === "word" && SQL_KEYWORDS.has(upper(alias))) continue;
    names.add(tokenKey(alias));
    positions.add(alias.start);
  }

  // Implicit select aliases: `SELECT email mail FROM ...`.
  for (let index = 0; index < tokens.length; index += 1) {
    if (
      upper(tokens[index]) !== "SELECT" ||
      !structural(index)
    ) {
      continue;
    }
    let depth = 0;
    let itemStart = index + 1;
    for (let cursor = index + 1; cursor < tokens.length; cursor += 1) {
      const token = tokens[cursor];
      if (token.kind === "punct" && token.text === "(") depth += 1;
      else if (token.kind === "punct" && token.text === ")") depth -= 1;
      const isEnd =
        (depth === 0 && upper(token) === "FROM") ||
        (depth === 0 && token.kind === "punct" && token.text === ";");
      const isComma =
        depth === 0 && token.kind === "punct" && token.text === ",";
      if (!isEnd && !isComma) continue;
      const end = isEnd ? cursor : cursor;
      if (end - itemStart >= 2 && isNameToken(tokens[end - 1])) {
        const alias = tokens[end - 1];
        const previous = tokens[end - 2];
        const previousWord = upper(previous);
        const previousIsKeyword =
          previous.kind === "word" && SQL_KEYWORDS.has(previousWord);
        if (
          !(previous.kind === "punct" && previous.text === ".") &&
          !(
            previousIsKeyword &&
            !["END", "NULL", "TRUE", "FALSE"].includes(previousWord)
          )
        ) {
          names.add(tokenKey(alias));
          positions.add(alias.start);
        }
      }
      itemStart = cursor + 1;
      if (isEnd) break;
    }
  }
  return { names, positions };
}

interface NavContext {
  refs: NavRef[];
  aliases: Map<string, NavRef[]>;
  cteNames: Set<string>;
  aliasPositions: Map<number, NavRef>;
  partPositions: Map<number, NavRef>;
  selectAliasNames: Set<string>;
  selectAliasPositions: Set<number>;
  derivedAliases: Set<string>;
  derivedAliasPositions: Set<number>;
  tablesAvailable: boolean;
}

function buildContext(
  tokens: SqlToken[],
  tables: ScopeTableColumns[],
  driver: Driver,
  scopeInfo: ResolveSqlNavigationOptions["scopeInfo"],
): NavContext {
  const refs: NavRef[] = [];
  const cteNames = new Set<string>();
  const derivedAliases = new Set<string>();
  const derivedAliasPositions = new Set<number>();
  const stack: Array<{ subquery: boolean }> = [];
  let cteModeDepth = -1;
  let cteMode = false;

  const structural = (_index: number) =>
    stack.length === 0 || stack[stack.length - 1].subquery;

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === "punct" && token.text === "(") {
      const next = tokens[index + 1];
      stack.push({
        subquery: upper(next) === "SELECT" || upper(next) === "WITH",
      });
      continue;
    }
    if (token.kind === "punct" && token.text === ")") {
      const popped = stack.pop();
      if (popped?.subquery) {
        let aliasIndex = index + 1;
        if (upper(tokens[aliasIndex]) === "AS") aliasIndex += 1;
        const alias = tokens[aliasIndex];
        if (
          isNameToken(alias) &&
          !SQL_KEYWORDS.has(upper(alias))
        ) {
          derivedAliases.add(tokenKey(alias));
          derivedAliasPositions.add(alias.start);
        }
      }
      continue;
    }
    if (!structural(index) || token.kind !== "word") continue;
    const word = upper(token);

    if (word === "WITH") {
      cteMode = true;
      cteModeDepth = stack.length;
    } else if (cteMode && stack.length === cteModeDepth) {
      const previous = tokens[index - 1];
      if (
        isNameToken(token) &&
        !SQL_KEYWORDS.has(word) &&
        (upper(previous) === "WITH" ||
          upper(previous) === "RECURSIVE" ||
          (previous?.kind === "punct" && previous.text === ","))
      ) {
        cteNames.add(tokenKey(token));
      }
      if (["SELECT", "UPDATE", "INSERT", "DELETE"].includes(word)) {
        cteMode = false;
      }
    }

    if (word === "FROM" || word === "JOIN") {
      const parsed = readRefList(
        tokens,
        index + 1,
        tables,
        cteNames,
        driver,
        scopeInfo,
      );
      for (const ref of parsed.refs) refs.push(ref);
    }
    if (word === "UPDATE") {
      let start = index + 1;
      if (upper(tokens[start]) === "OR" && isNameToken(tokens[start + 1])) {
        start += 2;
      }
      while (
        ["LOW_PRIORITY", "IGNORE", "ONLY"].includes(upper(tokens[start]))
      ) {
        start += 1;
      }
      const parsed = readRefList(
        tokens,
        start,
        tables,
        cteNames,
        driver,
        scopeInfo,
      );
      for (const ref of parsed.refs) refs.push(ref);
    }
    if (
      word === "INTO" &&
      index > 0 &&
      ["INSERT", "REPLACE"].includes(upper(tokens[index - 1]))
    ) {
      const populated = readRef(
        tokens,
        index + 1,
        tables,
        cteNames,
        driver,
        scopeInfo,
      );
      if (populated) refs.push(populated.ref);
    }
    if (
      word === "TABLE" &&
      index > 0 &&
      ["ALTER", "TRUNCATE"].includes(upper(tokens[index - 1]))
    ) {
      const populated = readRef(
        tokens,
        index + 1,
        tables,
        cteNames,
        driver,
        scopeInfo,
      );
      if (populated) refs.push(populated.ref);
    }
  }

  const aliases = new Map<string, NavRef[]>();
  const aliasPositions = new Map<number, NavRef>();
  const partPositions = new Map<number, NavRef>();
  for (const ref of refs) {
    if (ref.alias) {
      const key = tokenKey(ref.alias);
      aliases.set(key, [...(aliases.get(key) ?? []), ref]);
      aliasPositions.set(ref.alias.start, ref);
    }
    const key = tokenKey(ref.parts[ref.parts.length - 1]);
    aliases.set(key, [...(aliases.get(key) ?? []), ref]);
    for (const part of ref.parts) partPositions.set(part.start, ref);
  }
  const selected = collectSelectAliases(tokens, structural);

  return {
    refs,
    aliases,
    cteNames,
    aliasPositions,
    partPositions,
    selectAliasNames: selected.names,
    selectAliasPositions: selected.positions,
    derivedAliases,
    derivedAliasPositions,
    tablesAvailable: tables.length > 0,
  };
}

function tablePayload(
  ref: NavRef,
  from: number,
  to: number,
): SqlNavigationPayload {
  const fullName = ref.parts.map((part) => part.value).join(".");
  if (!ref.table) {
    return {
      from,
      to,
      reason: ref.cte
        ? "CTE 名称不导航"
        : ref.tableFunction
          ? "该对象暂不支持从此处打开"
          : ref.crossScope
            ? `跨范围对象暂不支持导航：${fullName}`
            : ref.ambiguous
              ? `找到多个同名对象 ${fullName}，无法确定要打开哪一个。`
              : `未在当前库 / 模式找到表 ${fullName}，请刷新对象树后重试。`,
    };
  }
  const kind = ref.table.kind;
  if (!["table", "view", "matview"].includes(kind)) {
    return {
      from,
      to,
      reason: "该对象暂不支持从此处打开",
    };
  }
  return {
    kind: "table",
    table: ref.table.name,
    tableKind: kind,
    from,
    to,
  };
}

function resolveRefAt(
  ref: NavRef,
  token: SqlToken,
): SqlNavigationPayload | null {
  const part = ref.parts.find((item) => item.start === token.start);
  if (part) {
    return tablePayload(ref, ref.parts[0].start, ref.parts[ref.parts.length - 1].end);
  }
  if (ref.alias && ref.alias.start === token.start) {
    return tablePayload(ref, ref.alias.start, ref.alias.end);
  }
  return null;
}

function uniqueRefs(list: NavRef[]): NavRef[] {
  const seen = new Set<NavRef>();
  const result: NavRef[] = [];
  for (const ref of list) {
    if (seen.has(ref)) continue;
    seen.add(ref);
    result.push(ref);
  }
  return result;
}

export function resolveSqlNavigation(
  options: ResolveSqlNavigationOptions,
): SqlNavigationPayload | null {
  const { document, offset, tables, driver, scopeInfo } = options;
  const tokenized = tokenizeSql(document);
  if (tokenized.tooLarge) {
    return {
      from: offset,
      to: offset,
      reason: "SQL 过大，暂不支持导航。",
    };
  }
  const token = tokenized.tokens.find(
    (item) => item.start <= offset && offset < item.end,
  );
  if (!token || (token.kind !== "word" && token.kind !== "quoted")) {
    return null;
  }
  if (token.kind === "quoted" && token.value === "") return null;
  if (token.kind === "word" && SQL_KEYWORDS.has(token.text.toUpperCase())) {
    return null;
  }

  const statementRange =
    splitSqlStatements(document).find(
      (range) => token.start >= range.start && token.start < range.end,
    ) ?? { start: 0, end: document.length, text: document };
  const statementTokens = tokenized.tokens.filter(
    (item) => item.start >= statementRange.start && item.start < statementRange.end,
  );
  const context = buildContext(statementTokens, tables, driver, scopeInfo);

  if (!context.tablesAvailable) {
    return {
      from: token.start,
      to: token.end,
      reason: "当前库 / 模式还没有表与字段元数据，请刷新对象树后重试。",
    };
  }

  // Table reference name, qualifier, or alias.
  const refPayload = (() => {
    for (const ref of context.refs) {
      const payload = resolveRefAt(ref, token);
      if (payload) return payload;
    }
    return null;
  })();
  if (refPayload) return refPayload;

  if (context.cteNames.has(tokenKey(token))) {
    return {
      from: token.start,
      to: token.end,
      reason: "CTE 名称不导航",
    };
  }
  if (context.derivedAliasPositions.has(token.start)) {
    return {
      from: token.start,
      to: token.end,
      reason: "子查询 / 派生表别名无法直接打开。",
    };
  }

  if (context.selectAliasPositions.has(token.start)) {
    return {
      from: token.start,
      to: token.end,
      reason: "SELECT 别名不是表字段",
    };
  }

  const previous = statementTokens.find((item) => item.end === token.start);
  const previousPrevious = previous
    ? statementTokens.find((item) => item.end === previous.start)
    : null;
  const next = statementTokens.find((item) => item.start === token.end);
  const nextNext = next
    ? statementTokens.find((item) => item.start === next.end)
    : null;

  const resolveQualifier = (
    qualifier: SqlToken,
  ): { table?: ScopeTableColumns; reason?: string } => {
    if (context.cteNames.has(tokenKey(qualifier))) {
      return { reason: "无法确定该列所属表（CTE）" };
    }
    if (
      context.derivedAliases.has(tokenKey(qualifier)) ||
      context.derivedAliasPositions.has(qualifier.start)
    ) {
      return { reason: "无法确定该列所属表（子查询）" };
    }
    const refs = uniqueRefs(
      (context.aliases.get(tokenKey(qualifier)) ?? []).filter(
        (ref) => !ref.cte && !ref.tableFunction,
      ),
    );
    if (refs.length > 1) {
      return { reason: `别名 ${qualifier.value} 不唯一，无法确定该列所属表。` };
    }
    if (refs.length === 1) {
      return refs[0].table
        ? { table: refs[0].table }
        : { reason: "无法确定该列所属表" };
    }
    const direct = findTableByName(tables, qualifier);
    if (direct.table) return { table: direct.table };
    return { reason: direct.reason ?? "无法确定该列所属表" };
  };

  // table.column / alias.column. Clicking the qualifier opens its table.
  if (
    previous?.kind === "punct" &&
    previous.text === "." &&
    previousPrevious &&
    isNameToken(previousPrevious)
  ) {
    const qualified = resolveQualifier(previousPrevious);
    if (!qualified.table) {
      return {
        from: token.start,
        to: token.end,
        reason: qualified.reason ?? "无法确定该列所属表",
      };
    }
    if (
      !qualified.table.columns.some((column) => matchesName(token, column.name))
    ) {
      return {
        from: token.start,
        to: token.end,
        reason: `未找到列 ${token.value}（表 ${qualified.table.name}）。`,
      };
    }
    return {
      kind: "column",
      table: qualified.table.name,
      tableKind: qualified.table.kind,
      column: qualified.table.columns.find((column) =>
        matchesName(token, column.name),
      )?.name,
      from: token.start,
      to: token.end,
    };
  }

  if (
    next?.kind === "punct" &&
    next.text === "." &&
    nextNext &&
    isNameToken(nextNext)
  ) {
    // Clicking a table / alias used as a column qualifier.
    if (
      previous?.kind === "punct" &&
      previous.text === "." &&
      previousPrevious &&
      isNameToken(previousPrevious)
    ) {
      const ref = context.partPositions.get(token.start);
      if (ref) {
        return tablePayload(ref, token.start, token.end);
      }
      const direct = findTableByName(tables, token);
      if (direct.table) {
        return {
          kind: "table",
          table: direct.table.name,
          tableKind: direct.table.kind,
          from: token.start,
          to: token.end,
        };
      }
    }
    const qualified = resolveQualifier(token);
    if (qualified.table) {
      return {
        kind: "table",
        table: qualified.table.name,
        tableKind: qualified.table.kind,
        from: token.start,
        to: token.end,
      };
    }
    return {
      from: token.start,
      to: token.end,
      reason: qualified.reason ?? "无法确定该对象",
    };
  }

  if (previous?.kind === "punct" && previous.text === ".") {
    return {
      from: token.start,
      to: token.end,
      reason: "无法确定该列所属表",
    };
  }

  // Function name / parameter / select alias usage.
  if (next?.kind === "punct" && next.text === "(") return null;
  if (
    previous?.kind === "punct" &&
    ["@", "$"].includes(previous.text)
  ) {
    return null;
  }
  if (context.selectAliasNames.has(tokenKey(token))) {
    return {
      from: token.start,
      to: token.end,
      reason: "SELECT 别名不是表字段",
    };
  }
  if (context.derivedAliases.has(tokenKey(token))) {
    return {
      from: token.start,
      to: token.end,
      reason: "子查询 / 派生表别名无法直接打开。",
    };
  }

  const tableRefs = uniqueRefs(
    context.refs.filter((ref) => !ref.cte && !ref.tableFunction),
  );
  if (context.refs.some((ref) => ref.cte) || tableRefs.length !== 1) {
    return {
      from: token.start,
      to: token.end,
      reason: "无法确定该列所属表",
    };
  }
  const table = tableRefs[0].table;
  if (!table) {
    return {
      from: token.start,
      to: token.end,
      reason: "无法确定该列所属表",
    };
  }
  const column = table.columns.find((item) => matchesName(token, item.name));
  if (!column) {
    return {
      from: token.start,
      to: token.end,
      reason: `未找到列 ${token.value}（表 ${table.name}）。`,
    };
  }
  return {
    kind: "column",
    table: table.name,
    tableKind: table.kind,
    column: column.name,
    from: token.start,
    to: token.end,
  };
}
