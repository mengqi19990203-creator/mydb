import { findDangerousSql } from "./dangerous-sql";
import { splitSqlStatements } from "./sql-statements";
import type {
  Driver,
  ScopeTableColumns,
  SqlCheckIssue,
  SqlCheckResult,
  SqlCheckSeverity,
} from "./types";

const MAX_SQL_BYTES = 100 * 1024;
const MAX_TOKENS = 5_000;
const MAX_ISSUES = 200;

export interface SqlToken {
  kind:
    | "word"
    | "quoted"
    | "string"
    | "number"
    | "punct"
    | "parameter";
  /// Raw source text.
  text: string;
  /// Identifier/string content without quote delimiters.
  value: string;
  start: number;
  end: number;
  quoted: boolean;
}

interface LexIssue {
  code: string;
  message: string;
  from: number;
  to: number;
}

interface TokenizeResult {
  tokens: SqlToken[];
  issues: LexIssue[];
  tooLarge: boolean;
}

const KEYWORDS = new Set(
  [
    "ADD",
    "ALL",
    "AT",
    "ALTER",
    "AND",
    "ANY",
    "ARRAY",
    "AS",
    "ASC",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CALL",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "CONFLICT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_CATALOG",
    "CURRENT_DATE",
    "CURRENT_ROLE",
    "CURRENT_SCHEMA",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "DATABASE",
    "DATE",
    "DATETIME",
    "DECIMAL",
    "DECLARE",
    "BIGINT",
    "BINARY",
    "BLOB",
    "BOOL",
    "BOOLEAN",
    "BYTEA",
    "CHAR",
    "CHARACTER",
    "DEFAULT",
    "DELETE",
    "DESC",
    "DISTINCT",
    "DO",
    "DROP",
    "ELSE",
    "END",
    "ESCAPE",
    "DO",
    "DOUBLE",
    "DIV",
    "EXCEPT",
    "EXISTS",
    "EXPLAIN",
    "EXTRACT",
    "FALSE",
    "FETCH",
    "FILTER",
    "FIRST",
    "FLOAT",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "FUNCTION",
    "GROUP",
    "HAVING",
    "GLOB",
    "IF",
    "ILIKE",
    "INT",
    "INTEGER",
    "IN",
    "INDEX",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTERVAL",
    "INTO",
    "IS",
    "JOIN",
    "KEY",
    "LAST",
    "LATERAL",
    "LEFT",
    "LIKE",
    "LIMIT",
    "LOCK",
    "JSON",
    "JSONB",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "MATCH",
    "MOD",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NOT",
    "NOTHING",
    "NUMERIC",
    "NULL",
    "NULLS",
    "OFFSET",
    "ON",
    "ONLY",
    "OR",
    "ORDER",
    "OUTER",
    "OVER",
    "PARTITION",
    "PRECISION",
    "PRIMARY",
    "PROCEDURE",
    "REAL",
    "REGEXP",
    "RECURSIVE",
    "REFERENCES",
    "REPLACE",
    "RETURNING",
    "RIGHT",
    "RLIKE",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SCHEMA",
    "SELECT",
    "SESSION_USER",
    "SIMILAR",
    "SMALLINT",
    "SYSTEM_USER",
    "TEXT",
    "SET",
    "TABLE",
    "TEMPORARY",
    "THEN",
    "TIME",
    "TIMESTAMP",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "TRUE",
    "TRUNCATE",
    "UNKNOWN",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USER",
    "USING",
    "UUID",
    "VARCHAR",
    "VARYING",
    "VALUES",
    "VIEW",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
    "XOR",
    "ZONE",
  ],
);

export const SQL_KEYWORDS: ReadonlySet<string> = KEYWORDS;

const COLUMN_COMPLEX_KEYWORDS = new Set([
  "ARRAY",
  "CASE",
  "CAST",
  "COLLATE",
  "CONVERT",
  "EXTRACT",
  "FILTER",
  "INTERVAL",
  "OVER",
  "POSITION",
  "SUBSTRING",
  "TRIM",
]);

function charClass(character: string): "word" | "digit" | "other" {
  if (/[A-Za-z_\u0080-￿]/.test(character)) return "word";
  if (/[0-9]/.test(character)) return "digit";
  return "other";
}

function isWordPart(character: string): boolean {
  return charClass(character) !== "other" || character === "$";
}

function dollarTagAt(
  sql: string,
  start: number,
): { tag: string; close: number } | null {
  if (sql[start] !== "$") return null;
  let index = start + 1;
  while (index < sql.length && /[A-Za-z0-9_]/.test(sql[index])) index += 1;
  if (sql[index] !== "$") return null;
  const tag = sql.slice(start, index + 1);
  const close = sql.indexOf(tag, index + 1);
  return { tag, close };
}

function readQuoted(
  sql: string,
  start: number,
  quote: string,
): { end: number; value: string } | null {
  let index = start + 1;
  let value = "";
  while (index < sql.length) {
    const character = sql[index];
    if (quote === "[" && character === "]") {
      if (sql[index + 1] === "]") {
        value += "]";
        index += 2;
        continue;
      }
      return { end: index + 1, value };
    }
    if (quote !== "[" && character === "\\" && index + 1 < sql.length) {
      value += sql[index + 1];
      index += 2;
      continue;
    }
    if (character === quote) {
      if (sql[index + 1] === quote) {
        value += quote;
        index += 2;
        continue;
      }
      return { end: index + 1, value };
    }
    value += character;
    index += 1;
  }
  return null;
}

/** Lightweight positional tokenizer. Comments are dropped from the token list. */
export function tokenizeSql(sql: string): TokenizeResult {
  const issues: LexIssue[] = [];
  if (sql.length > MAX_SQL_BYTES) {
    return {
      tokens: [],
      issues: [
        {
          code: "sql-too-large",
          message: "SQL 超过 100 KB，请缩小选区后再检查。",
          from: 0,
          to: sql.length,
        },
      ],
      tooLarge: true,
    };
  }
  const tokens: SqlToken[] = [];
  let index = 0;
  let tooLarge = false;
  while (index < sql.length) {
    if (tokens.length >= MAX_TOKENS) {
      tooLarge = true;
      break;
    }
    const character = sql[index];
    const next = sql[index + 1];
    if (/\s/.test(character)) {
      index += 1;
      continue;
    }
    if (character === "-" && next === "-") {
      const end = sql.indexOf("\n", index + 2);
      index = end < 0 ? sql.length : end + 1;
      continue;
    }
    if (character === "#") {
      const end = sql.indexOf("\n", index + 1);
      index = end < 0 ? sql.length : end + 1;
      continue;
    }
    if (character === "/" && next === "*") {
      const close = sql.indexOf("*/", index + 2);
      if (close < 0) {
        issues.push({
          code: "unclosed-comment",
          message: "块注释没有闭合（缺少 */）。",
          from: index,
          to: sql.length,
        });
        index = sql.length;
      } else {
        index = close + 2;
      }
      continue;
    }
    if (character === "'" || character === '"' || character === "`") {
      const quote = character;
      const read = readQuoted(sql, index, quote);
      if (!read) {
        issues.push({
          code: quote === "'" ? "unclosed-string" : "unclosed-quote",
          message:
            quote === "'"
              ? "字符串没有闭合（缺少单引号）。"
              : "引号标识符没有闭合。",
          from: index,
          to: sql.length,
        });
        tokens.push({
          kind: quote === "'" ? "string" : "quoted",
          text: sql.slice(index),
          value: sql.slice(index + 1),
          start: index,
          end: sql.length,
          quoted: quote !== "'",
        });
        index = sql.length;
        break;
      }
      tokens.push({
        kind: quote === "'" ? "string" : "quoted",
        text: sql.slice(index, read.end),
        value: read.value,
        start: index,
        end: read.end,
        quoted: quote !== "'",
      });
      index = read.end;
      continue;
    }
    if (character === "[") {
      const read = readQuoted(sql, index, "[");
      if (!read) {
        issues.push({
          code: "unclosed-quote",
          message: "方括号标识符没有闭合（缺少 ]）。",
          from: index,
          to: sql.length,
        });
        tokens.push({
          kind: "quoted",
          text: sql.slice(index),
          value: sql.slice(index + 1),
          start: index,
          end: sql.length,
          quoted: true,
        });
        index = sql.length;
        break;
      }
      tokens.push({
        kind: "quoted",
        text: sql.slice(index, read.end),
        value: read.value,
        start: index,
        end: read.end,
        quoted: true,
      });
      index = read.end;
      continue;
    }
    if (character === "$") {
      const dollar = dollarTagAt(sql, index);
      if (dollar) {
        if (dollar.close < 0) {
          issues.push({
            code: "unclosed-dollar",
            message: "dollar quote 没有闭合（缺少结束标记）。",
            from: index,
            to: sql.length,
          });
          tokens.push({
            kind: "string",
            text: sql.slice(index),
            value: sql.slice(index + dollar.tag.length),
            start: index,
            end: sql.length,
            quoted: false,
          });
          index = sql.length;
          break;
        }
        const end = dollar.close + dollar.tag.length;
        tokens.push({
          kind: "string",
          text: sql.slice(index, end),
          value: sql.slice(index + dollar.tag.length, dollar.close),
          start: index,
          end,
          quoted: false,
        });
        index = end;
        continue;
      }
      let end = index + 1;
      while (end < sql.length && /[0-9]/.test(sql[end])) end += 1;
      if (end > index + 1) {
        tokens.push({
          kind: "parameter",
          text: sql.slice(index, end),
          value: sql.slice(index, end),
          start: index,
          end,
          quoted: false,
        });
        index = end;
        continue;
      }
    }
    if (
      (character === ":" || character === "@") &&
      charClass(next ?? "") === "word"
    ) {
      let end = index + 2;
      while (end < sql.length && /[A-Za-z0-9_]/.test(sql[end])) end += 1;
      tokens.push({
        kind: "parameter",
        text: sql.slice(index, end),
        value: sql.slice(index, end),
        start: index,
        end,
        quoted: false,
      });
      index = end;
      continue;
    }
    if (character === "?") {
      tokens.push({
        kind: "parameter",
        text: "?",
        value: "?",
        start: index,
        end: index + 1,
        quoted: false,
      });
      index += 1;
      continue;
    }
    if (
      character === "0" &&
      (next === "x" || next === "X") &&
      /[0-9A-Fa-f]/.test(sql[index + 2] ?? "")
    ) {
      let end = index + 2;
      while (end < sql.length && /[0-9A-Fa-f_]/.test(sql[end])) end += 1;
      tokens.push({
        kind: "number",
        text: sql.slice(index, end),
        value: sql.slice(index, end),
        start: index,
        end,
        quoted: false,
      });
      index = end;
      continue;
    }
    if (
      character === "0" &&
      (next === "b" || next === "B") &&
      /[01]/.test(sql[index + 2] ?? "")
    ) {
      let end = index + 2;
      while (end < sql.length && /[01_]/.test(sql[end])) end += 1;
      tokens.push({
        kind: "number",
        text: sql.slice(index, end),
        value: sql.slice(index, end),
        start: index,
        end,
        quoted: false,
      });
      index = end;
      continue;
    }
    if (/[0-9]/.test(character)) {
      let end = index + 1;
      while (end < sql.length && /[0-9._]/.test(sql[end])) end += 1;
      if (
        (sql[end] === "e" || sql[end] === "E") &&
        /[0-9+-]/.test(sql[end + 1] ?? "")
      ) {
        end += 2;
        while (end < sql.length && /[0-9]/.test(sql[end])) end += 1;
      }
      tokens.push({
        kind: "number",
        text: sql.slice(index, end),
        value: sql.slice(index, end),
        start: index,
        end,
        quoted: false,
      });
      index = end;
      continue;
    }
    if (charClass(character) === "word") {
      let end = index + 1;
      while (end < sql.length && isWordPart(sql[end])) end += 1;
      const text = sql.slice(index, end);
      tokens.push({
        kind: "word",
        text,
        value: text,
        start: index,
        end,
        quoted: false,
      });
      index = end;
      continue;
    }
    const pair = sql.slice(index, index + 3);
    const operator =
      ["->>", "::"].find((item) => pair.startsWith(item)) ??
      ["->", ">=", "<=", "<>", "!=", "||", ":="].find((item) =>
        sql.startsWith(item, index),
      );
    const text = operator ?? character;
    tokens.push({
      kind: "punct",
      text,
      value: text,
      start: index,
      end: index + text.length,
      quoted: false,
    });
    index += text.length;
  }

  if (tooLarge) {
    issues.push({
      code: "sql-too-many-tokens",
      message: "SQL 超过 5,000 个 token，请缩小选区后再检查。",
      from: 0,
      to: sql.length,
    });
  }

  // Parenthesis balance. Quotes and comments never reach this list.
  const stack: SqlToken[] = [];
  let extraClosings = 0;
  for (const token of tokens) {
    if (token.kind !== "punct") continue;
    if (token.text === "(") {
      stack.push(token);
    } else if (token.text === ")") {
      if (stack.length) stack.pop();
      else if (extraClosings < 5) {
        extraClosings += 1;
        issues.push({
          code: "unbalanced-parentheses",
          message: "多余的右括号。",
          from: token.start,
          to: token.end,
        });
      }
    }
  }
  for (const token of stack.slice(0, 5)) {
    issues.push({
      code: "unbalanced-parentheses",
      message: "左括号没有闭合（缺少右括号）。",
      from: token.start,
      to: token.end,
    });
  }

  return { tokens, issues, tooLarge };
}

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

function columnMatches(token: SqlToken, column: { name: string }): boolean {
  if (token.quoted) return token.value === column.name;
  return token.value.toLocaleLowerCase() === column.name.toLocaleLowerCase();
}

interface TableRef {
  nameToken: SqlToken;
  parts: SqlToken[];
  alias: SqlToken | null;
  table: ScopeTableColumns | null;
  crossScope: boolean;
  cte: boolean;
  tableFunction: boolean;
}

interface QualifiedName {
  parts: SqlToken[];
  next: number;
}

function readQualified(tokens: SqlToken[], start: number): QualifiedName | null {
  if (!isNameToken(tokens[start])) return null;
  const parts: SqlToken[] = [tokens[start]];
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

const ALIAS_STOP = new Set([
  ...KEYWORDS,
  "WHERE",
  "SET",
  "VALUES",
  "ON",
  "USING",
  "GROUP",
  "ORDER",
  "LIMIT",
  "OFFSET",
  "RETURNING",
  "UNION",
]);

function readTableRef(
  tokens: SqlToken[],
  start: number,
  tables: ScopeTableColumns[],
  cteNames: Set<string>,
  driver: Driver,
  scope: { database?: string | null; schema?: string | null },
): TableRef | null {
  let position = start;
  while (upper(tokens[position]) === "ONLY") position += 1;
  const qualified = readQualified(tokens, position);
  if (!qualified) return null;
  const parts = qualified.parts;
  let index = qualified.next;
  let alias: SqlToken | null = null;
  if (upper(tokens[index]) === "AS" && isNameToken(tokens[index + 1])) {
    alias = tokens[index + 1];
    index += 2;
  } else if (
    isNameToken(tokens[index]) &&
    !ALIAS_STOP.has(upper(tokens[index])) &&
    tokens[index].kind !== "string"
  ) {
    alias = tokens[index];
    index += 1;
  }
  const nameToken = parts[parts.length - 1];
  let table: ScopeTableColumns | null = null;
  let crossScope = false;
  const qualifiers = parts
    .slice(0, -1)
    .map((part) => part.value.toLocaleLowerCase());
  const localQualifiers = new Set(
    [scope.database, scope.schema]
      .filter((value): value is string => Boolean(value))
      .map((value) => value.toLocaleLowerCase()),
  );
  if (driver === "sqlite") {
    localQualifiers.add("main");
    localQualifiers.add("temp");
  }
  const localScope =
    parts.length === 1 ||
    qualifiers.every((qualifier) => localQualifiers.has(qualifier));
  if (localScope) {
    table =
      tables.find((item) => {
        if (nameToken.quoted) return item.name === nameToken.value;
        return item.name.toLocaleLowerCase() === nameToken.value.toLocaleLowerCase();
      }) ?? null;
  } else {
    crossScope = true;
  }
  if (
    !table &&
    parts.length === 1 &&
    nameToken.kind === "word" &&
    ((driver === "mysql" && nameToken.text.toUpperCase() === "DUAL") ||
      (driver === "postgres" &&
        nameToken.text.toLocaleLowerCase().startsWith("pg_")) ||
      (driver === "sqlite" &&
        [
          "SQLITE_MASTER",
          "SQLITE_SCHEMA",
          "SQLITE_TEMP_MASTER",
          "SQLITE_SEQUENCE",
          "SQLITE_STAT1",
        ].includes(nameToken.text.toUpperCase())))
  ) {
    table = { name: nameToken.text, kind: "table", columns: [] };
  }
  return {
    nameToken,
    parts,
    alias,
    table,
    crossScope,
    cte: parts.length === 1 && cteNames.has(tokenKey(nameToken)),
    tableFunction: tokens[index]?.text === "(",
  };
}

function lineColumnAt(text: string, offset: number): { line: number; column: number } {
  const before = text.slice(0, Math.max(0, Math.min(text.length, offset)));
  const lines = before.split("\n");
  return {
    line: lines.length,
    column: (lines[lines.length - 1]?.length ?? 0) + 1,
  };
}

interface CheckOptions {
  text: string;
  document?: string;
  offset?: number;
  scope?: "statement" | "tab" | "selection";
  driver: Driver;
  tables: ScopeTableColumns[];
  /// Current database / schema names used to verify qualified table refs.
  scopeInfo?: { database?: string | null; schema?: string | null };
}

export function checkSql(options: CheckOptions): SqlCheckResult {
  const document = options.document ?? options.text;
  const offset = options.offset ?? 0;
  const scope = options.scope ?? "tab";
  const issues: SqlCheckIssue[] = [];
  const add = (
    severity: SqlCheckSeverity,
    code: string,
    message: string,
    from: number,
    to: number,
  ) => {
    if (issues.length >= MAX_ISSUES + 20) return;
    const absoluteFrom = offset + Math.max(0, Math.min(options.text.length, from));
    const absoluteTo = offset + Math.max(absoluteFrom - offset, Math.min(options.text.length, to));
    const position = lineColumnAt(document, absoluteFrom);
    issues.push({
      severity,
      code,
      message,
      from: absoluteFrom,
      to: absoluteTo,
      line: position.line,
      column: position.column,
    });
  };

  const lexed = tokenizeSql(options.text);
  const tooLarge = lexed.tooLarge || options.text.length > MAX_SQL_BYTES;
  for (const issue of lexed.issues) {
    add("error", issue.code, issue.message, issue.from, issue.to);
  }

  const tables = options.tables.filter((table) => table.name);
  const objectCheck = tables.length ? "available" : "unavailable";
  const procedural =
    findProceduralCreate(lexed.tokens) ??
    lexed.tokens.find((token) => upper(token) === "DELIMITER") ??
    null;
  if (!tooLarge && procedural) {
    add(
      "info",
      "procedural-skip",
      "包含 DELIMITER 或过程体定义，已跳过表与列检查。",
      procedural.start,
      procedural.end,
    );
  }
  if (!tooLarge && !procedural) {
    if (tables.length) {
      const ranges = splitSqlStatements(options.text);
      if (ranges.length <= 1) {
        checkObjects(lexed.tokens, tables, add, options.driver, options.scopeInfo ?? {});
      } else {
        for (const range of ranges) {
          const statementTokens = lexed.tokens.filter(
            (token) => token.start >= range.start && token.start < range.end,
          );
          if (statementTokens.length) {
            checkObjects(statementTokens, tables, add, options.driver, options.scopeInfo ?? {});
          }
        }
      }
    }
    checkDangerous(options.text, add);
  }

  const seen = new Set<string>();
  const unique = issues.filter((issue) => {
    const key = `${issue.code}:${issue.from}:${issue.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  unique.sort((left, right) => left.from - right.from || left.column - right.column);
  const truncated = unique.length > MAX_ISSUES;
  return {
    issues: unique.slice(0, MAX_ISSUES),
    scope,
    checkedFrom: offset,
    checkedTo: offset + options.text.length,
    objectCheck,
    tokenCount: lexed.tokens.length,
    truncated,
  };
}

function findProceduralCreate(tokens: SqlToken[]): SqlToken | null {
  for (let index = 0; index < tokens.length; index += 1) {
    if (upper(tokens[index]) !== "CREATE") continue;
    const stop = Math.min(tokens.length, index + 10);
    for (let next = index + 1; next < stop; next += 1) {
      const word = upper(tokens[next]);
      if (["PROCEDURE", "FUNCTION", "TRIGGER", "EVENT"].includes(word)) {
        return tokens[index];
      }
      if (
        ["TABLE", "VIEW", "INDEX", "SCHEMA", "DATABASE", "SEQUENCE"].includes(
          word,
        )
      ) {
        break;
      }
    }
  }
  return null;
}

function checkObjects(
  tokens: SqlToken[],
  tables: ScopeTableColumns[],
  add: (
    severity: SqlCheckSeverity,
    code: string,
    message: string,
    from: number,
    to: number,
  ) => void,
  driver: Driver,
  scope: { database?: string | null; schema?: string | null },
) {
  const refs: TableRef[] = [];
  const busy = new Set<number>();
  const cteNames = new Set<string>();
  let cteMode = false;
  let complex = false;
  let depth = 0;
  let kind = "";
  let selectIndex = -1;
  let fromIndex = -1;
  let whereIndex = -1;
  let groupIndex = -1;
  let orderIndex = -1;
  let havingIndex = -1;
  let setIndex = -1;
  let valuesIndex = -1;
  let intoIndex = -1;

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    const word = upper(token);
    if (token.kind === "punct" && token.text === "(") {
      const inner = upper(tokens[index + 1]);
      if (depth === 0 && (inner === "SELECT" || inner === "WITH")) {
        complex = true;
      }
      depth += 1;
      continue;
    }
    if (token.kind === "punct" && token.text === ")") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (depth !== 0 || token.kind !== "word") continue;
    if (!kind && ["SELECT", "UPDATE", "INSERT", "DELETE", "REPLACE"].includes(word)) {
      kind = word.toLowerCase();
    }
    if (word === "WITH") {
      complex = true;
      cteMode = true;
    }
    if (cteMode) {
      const previous = tokens[index - 1];
      const previousWord = upper(previous);
      if (
        isNameToken(token) &&
        !KEYWORDS.has(word) &&
        (previousWord === "WITH" ||
          previousWord === "RECURSIVE" ||
          (previous?.kind === "punct" && previous.text === ","))
      ) {
        cteNames.add(tokenKey(token));
      }
      if (
        ["SELECT", "UPDATE", "INSERT", "DELETE"].includes(word) &&
        !["WITH"].includes(word)
      ) {
        cteMode = false;
      }
    }
    if (word === "UNION" || word === "INTERSECT" || word === "EXCEPT") {
      complex = true;
    }
    if (word === "JOIN" || word === "USING" || word === "NATURAL") {
      complex = true;
    }
    if (word === "JOIN") {
      const ref = readTableRef(tokens, index + 1, tables, cteNames, driver, scope);
      if (ref) {
        refs.push(ref);
        busy.add(ref.nameToken.start);
        for (const part of ref.parts) busy.add(part.start);
        if (ref.alias) busy.add(ref.alias.start);
        if (!ref.tableFunction && !ref.table && !ref.crossScope && !ref.cte) {
          add(
            "warning",
            "unknown-table",
            `未在当前库 / 模式找到表 ${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        } else if (!ref.tableFunction && ref.crossScope) {
          add(
            "info",
            "cross-scope",
            `跨范围对象未检查：${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        }
      }
    }
    if (word === "SELECT") {
      if (selectIndex < 0) selectIndex = index;
      if (fromIndex < 0) {
        const nextFrom = tokens.findIndex(
          (candidate, position) =>
            position > index && upper(candidate) === "FROM",
        );
        if (nextFrom < 0) fromIndex = tokens.length;
      }
    }
    if (word === "FROM") {
      if (fromIndex < 0) fromIndex = index;
      const ref = readTableRef(tokens, index + 1, tables, cteNames, driver, scope);
      if (ref) {
        refs.push(ref);
        busy.add(ref.nameToken.start);
        for (const part of ref.parts) busy.add(part.start);
        if (ref.alias) busy.add(ref.alias.start);
        if (!ref.tableFunction && !ref.table && !ref.crossScope && !ref.cte) {
          add(
            "warning",
            "unknown-table",
            `未在当前库 / 模式找到表 ${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        } else if (!ref.tableFunction && ref.crossScope) {
          add(
            "info",
            "cross-scope",
            `跨范围对象未检查：${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        }
      } else if (tokens[index + 1]?.text === "(") {
        complex = true;
      }
    }
    if (word === "UPDATE") {
      let refStart = index + 1;
      if (upper(tokens[refStart]) === "OR" && isNameToken(tokens[refStart + 1])) {
        refStart += 2;
      }
      while (
        ["LOW_PRIORITY", "IGNORE", "ONLY"].includes(upper(tokens[refStart]))
      ) {
        refStart += 1;
      }
      const ref = readTableRef(tokens, refStart, tables, cteNames, driver, scope);
      if (ref) {
        refs.push(ref);
        busy.add(ref.nameToken.start);
        for (const part of ref.parts) busy.add(part.start);
        if (ref.alias) busy.add(ref.alias.start);
        if (!ref.tableFunction && !ref.table && !ref.crossScope && !ref.cte) {
          add(
            "warning",
            "unknown-table",
            `未在当前库 / 模式找到表 ${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        }
      }
    }
    if (
      word === "INTO" &&
      index > 0 &&
      ["INSERT", "REPLACE"].includes(upper(tokens[index - 1]))
    ) {
      intoIndex = index;
      const ref = readTableRef(tokens, index + 1, tables, cteNames, driver, scope);
      if (ref) {
        refs.push(ref);
        busy.add(ref.nameToken.start);
        for (const part of ref.parts) busy.add(part.start);
        if (ref.alias) busy.add(ref.alias.start);
        if (!ref.table && !ref.crossScope && !ref.cte) {
          add(
            "warning",
            "unknown-table",
            `未在当前库 / 模式找到表 ${ref.parts.map((part) => part.value).join(".")}。`,
            ref.nameToken.start,
            ref.nameToken.end,
          );
        }
      }
    }
    if (word === "WHERE" && whereIndex < 0) whereIndex = index;
    if (word === "GROUP" && groupIndex < 0) groupIndex = index;
    if (word === "ORDER" && orderIndex < 0) orderIndex = index;
    if (word === "HAVING" && havingIndex < 0) havingIndex = index;
    if (word === "SET" && setIndex < 0) setIndex = index;
    if (word === "VALUES" && valuesIndex < 0) valuesIndex = index;
  }

  if (refs.length === 0) return;

  const commaAfterFrom =
    fromIndex >= 0 &&
    tokens.some(
      (token, index) =>
        index > fromIndex &&
        token.kind === "punct" &&
        token.text === ",",
    );
  const commaBeforeSet =
    setIndex >= 0 &&
    tokens.some(
      (token, index) =>
        index < setIndex &&
        token.kind === "punct" &&
        token.text === ",",
    );
  const starInSelect =
    selectIndex >= 0 &&
    tokens
      .slice(selectIndex + 1, fromIndex >= 0 ? fromIndex : undefined)
      .some((token) => token.kind === "punct" && token.text === "*");
  const complexForColumns =
    complex ||
    commaAfterFrom ||
    commaBeforeSet ||
    starInSelect ||
    refs.length !== 1 ||
    !refs[0].table ||
    refs[0].table.columns.length === 0;

  if (complexForColumns) return;

  const table = refs[0].table;
  if (!table) return;
  const aliases = new Map<string, ScopeTableColumns>();
  if (refs[0].alias) {
    aliases.set(tokenKey(refs[0].alias), table);
  }
  aliases.set(tokenKey(refs[0].nameToken), table);

  const hasParentheses = tokens.some(
    (token) => token.kind === "punct" && token.text === "(",
  );
  const hasComplexKeyword = tokens.some(
    (token) =>
      token.kind === "word" &&
      COLUMN_COMPLEX_KEYWORDS.has(token.text.toUpperCase()),
  );
  const hasComplexOperator = tokens.some(
    (token) =>
      token.kind === "punct" &&
      ["::", "||", "->", "->>"].includes(token.text),
  );
  if (kind !== "insert" && (hasParentheses || hasComplexKeyword || hasComplexOperator)) {
    return;
  }

  const selectAliases = new Set<string>();
  const candidates: Array<{ token: SqlToken; qualifier: SqlToken | null }> = [];
  const addCandidate = (token: SqlToken, qualifier: SqlToken | null) => {
    if (!isNameToken(token)) return;
    const key = tokenKey(token);
    if (KEYWORDS.has(token.text.toUpperCase())) return;
    if (selectAliases.has(key)) return;
    if (!qualifier && aliases.has(key)) return;
    candidates.push({ token, qualifier });
  };

  const collectSegment = (start: number, end: number) => {
    for (let index = start; index < end; index += 1) {
      const token = tokens[index];
      if (!isNameToken(token)) continue;
      const next = tokens[index + 1];
      if (next?.kind === "punct" && next.text === ".") {
        const last = tokens[index + 2];
        if (isNameToken(last)) {
          if (aliases.has(tokenKey(token))) {
            addCandidate(last, token);
          }
          index += 2;
          continue;
        }
      }
      if (busy.has(token.start)) continue;
      if (KEYWORDS.has(token.text.toUpperCase())) continue;
      const previous = tokens[index - 1];
      if (next?.kind === "punct" && next.text === "(") continue;
      if (next?.kind === "string") continue;
      if (
        next?.kind === "punct" &&
        next.text === "&" &&
        token.text.toUpperCase() === "U"
      ) {
        continue;
      }
      if (previous?.kind === "punct" && previous.text === ".") continue;
      if (
        previous?.kind === "punct" &&
        ["@", "$"].includes(previous.text)
      ) {
        continue;
      }
      addCandidate(token, null);
    }
  };

  const segmentEnd = (start: number, stops: number[]) => {
    const candidatesEnd = stops.filter((value) => value > start);
    return candidatesEnd.length ? Math.min(...candidatesEnd) : tokens.length;
  };

  if (kind === "select" && selectIndex >= 0 && fromIndex > selectIndex) {
    let itemStart = selectIndex + 1;
    const items: Array<[number, number]> = [];
    let depth = 0;
    for (let index = selectIndex + 1; index < fromIndex; index += 1) {
      const token = tokens[index];
      if (token.kind === "punct" && token.text === "(") depth += 1;
      if (token.kind === "punct" && token.text === ")") depth -= 1;
      if (token.kind === "punct" && token.text === "," && depth === 0) {
        items.push([itemStart, index]);
        itemStart = index + 1;
      }
    }
    items.push([itemStart, fromIndex]);
    for (const [start, end] of items) {
      let from = start;
      while (from < end && upper(tokens[from]) === "DISTINCT") from += 1;
      let to = end;
      // "expr AS alias" / "expr alias".
      if (to - from >= 2 && upper(tokens[to - 2]) === "AS" && isNameToken(tokens[to - 1])) {
        const alias = tokens[to - 1];
        selectAliases.add(tokenKey(alias));
        to -= 2;
      } else if (
        to - from >= 2 &&
        isNameToken(tokens[to - 1]) &&
        !(tokens[to - 2].kind === "punct" && tokens[to - 2].text === ".")
      ) {
        const alias = tokens[to - 1];
        if (!KEYWORDS.has(alias.text.toUpperCase())) {
          selectAliases.add(tokenKey(alias));
          to -= 1;
        }
      }
      if (to <= from) continue;
      if (to - from === 1) {
        addCandidate(tokens[from], null);
      } else {
        const qualified = readQualified(tokens.slice(0, to), from);
        if (qualified && qualified.next === to) {
          const last = qualified.parts[qualified.parts.length - 1];
          const first =
            qualified.parts.length > 1 ? qualified.parts[0] : null;
          if (first) {
            const resolved = aliases.get(tokenKey(first));
            if (resolved) addCandidate(last, first);
          } else {
            addCandidate(last, null);
          }
        }
      }
    }
  }

  if (kind === "select") {
    if (whereIndex >= 0) {
      collectSegment(
        whereIndex + 1,
        segmentEnd(whereIndex, [groupIndex, orderIndex, havingIndex]),
      );
    }
    if (groupIndex >= 0) {
      collectSegment(
        tokens[groupIndex + 1] && upper(tokens[groupIndex + 1]) === "BY"
          ? groupIndex + 2
          : groupIndex + 1,
        segmentEnd(groupIndex, [orderIndex, havingIndex]),
      );
    }
    if (orderIndex >= 0) {
      collectSegment(
        tokens[orderIndex + 1] && upper(tokens[orderIndex + 1]) === "BY"
          ? orderIndex + 2
          : orderIndex + 1,
        segmentEnd(orderIndex, [havingIndex]),
      );
    }
    if (havingIndex >= 0) {
      collectSegment(havingIndex + 1, tokens.length);
    }
  } else if (kind === "update") {
    if (setIndex >= 0) {
      const setEnd = segmentEnd(setIndex, [whereIndex, orderIndex]);
      collectSegment(setIndex + 1, setEnd);
    }
    if (whereIndex >= 0) {
      collectSegment(whereIndex + 1, segmentEnd(whereIndex, [orderIndex]));
    }
  } else if (kind === "delete") {
    if (whereIndex >= 0) collectSegment(whereIndex + 1, tokens.length);
  } else if (kind === "insert" && intoIndex >= 0) {
    const open = tokens.findIndex(
      (token, index) =>
        index > intoIndex &&
        token.kind === "punct" &&
        token.text === "(",
    );
    const values = valuesIndex >= 0 ? valuesIndex : tokens.length;
    if (open >= 0 && open < values) {
      let depth = 0;
      for (let index = open; index < values; index += 1) {
        const token = tokens[index];
        if (token.kind === "punct" && token.text === "(") depth += 1;
        if (token.kind === "punct" && token.text === ")") depth -= 1;
        if (index > open && depth === 1 && isNameToken(token)) {
          addCandidate(token, null);
        }
      }
    }
  }

  for (const candidate of candidates) {
    let target = table;
    if (candidate.qualifier) {
      const resolved = aliases.get(tokenKey(candidate.qualifier));
      if (!resolved) continue;
      target = resolved;
    }
    if (target.columns.some((column) => columnMatches(candidate.token, column))) {
      continue;
    }
    add(
      "warning",
      "unknown-column",
      `未找到列 ${candidate.token.value}（表 ${target.name}）。`,
      candidate.token.start,
      candidate.token.end,
    );
  }
}

function checkDangerous(
  sql: string,
  add: (
    severity: SqlCheckSeverity,
    code: string,
    message: string,
    from: number,
    to: number,
  ) => void,
) {
  for (const statement of splitSqlStatements(sql)) {
    const finding = findDangerousSql(statement.text);
    if (!finding) continue;
    const code =
      finding.kind === "missing-where"
        ? "dangerous-dml"
        : finding.kind === "drop"
          ? "dangerous-drop"
          : finding.kind === "truncate"
            ? "dangerous-truncate"
            : "dangerous-alter";
    let from = statement.start;
    const rangeEnd = statement.start + statement.text.length;
    while (from < rangeEnd && /\s/.test(sql[from])) from += 1;
    add("warning", code, finding.message, from, rangeEnd);
  }
}
