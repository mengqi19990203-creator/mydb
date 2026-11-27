import { tokenizeSql } from "./sql-check";
import { hasSqlContent, splitSqlStatements } from "./sql-statements";
import { sqlLiteral } from "./sql-literal";
import type { Driver } from "./types";

export type SqlParameterKind = "named" | "positional";
export type SqlParameterType = "string" | "number" | "boolean" | "null" | "raw";

export interface SqlParameterDefinition {
  key: string;
  name: string;
  kind: SqlParameterKind;
  /// 1-based statement numbers where this parameter appears.
  statements: number[];
  occurrences: number;
}

export interface SqlParameterOccurrence {
  key: string;
  statementIndex: number;
  from: number;
  to: number;
}

export interface SqlParameterExtraction {
  definitions: SqlParameterDefinition[];
  occurrences: SqlParameterOccurrence[];
  errors: string[];
  unsupported: string[];
  statementCount: number;
  tooLarge: boolean;
}

export interface SqlParameterValue {
  type: SqlParameterType;
  value: string;
  sensitive?: boolean;
  rawConfirmed?: boolean;
  /// Distinguishes an explicitly entered empty string from an untouched field.
  touched?: boolean;
}

export interface SqlParameterSubstitution {
  sql: string;
  applied: number;
  missing: string[];
  errors: string[];
}

const MAX_SQL_BYTES = 100 * 1024;
const MAX_PARAMETERS = 50;
const MAX_VALUE_BYTES = 10 * 1024;
const MAX_NAME_LENGTH = 64;
const NAMED_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;
const NUMBER_PATTERN = /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/;

function executableStatements(sql: string) {
  return splitSqlStatements(sql).filter((range) => hasSqlContent(range.text));
}

function statementIndexFor(
  ranges: ReturnType<typeof executableStatements>,
  start: number,
  end: number,
): number {
  const index = ranges.findIndex(
    (range) => start >= range.start && end <= range.end,
  );
  return index < 0 ? 0 : index + 1;
}

export function extractSqlParameters(
  sql: string,
  driver: Driver,
): SqlParameterExtraction {
  const empty: SqlParameterExtraction = {
    definitions: [],
    occurrences: [],
    errors: [],
    unsupported: [],
    statementCount: 0,
    tooLarge: false,
  };
  if (sql.length > MAX_SQL_BYTES) {
    return { ...empty, tooLarge: true, errors: ["SQL 超过 100 KB，无法提取参数"] };
  }
  const statements = executableStatements(sql);
  empty.statementCount = statements.length;
  if (!statements.length) return empty;
  const tokenized = tokenizeSql(sql);
  if (tokenized.tooLarge) {
    return { ...empty, tooLarge: true, errors: ["SQL 过大，无法提取参数"] };
  }

  const definitions = new Map<string, SqlParameterDefinition>();
  const occurrences: SqlParameterOccurrence[] = [];
  const errors: string[] = [];
  const unsupported: string[] = [];
  let positionalIndex = 0;
  let positionalCount = 0;
  const mixedStatements = new Set<number>();

  for (const token of tokenized.tokens) {
    if (token.kind !== "parameter") continue;
    const text = token.text;
    if (text === "?") {
      if (driver === "postgres") continue; // JSONB operator, never a parameter.
      const statementIndex = statementIndexFor(statements, token.start, token.end);
      positionalIndex += 1;
      positionalCount += 1;
      const key = `?${positionalIndex}`;
      const definition: SqlParameterDefinition = {
        key,
        name: `?${positionalIndex}`,
        kind: "positional",
        statements: [statementIndex],
        occurrences: 1,
      };
      definitions.set(key, definition);
      occurrences.push({
        key,
        statementIndex,
        from: token.start,
        to: token.end,
      });
      continue;
    }
    if (text.startsWith(":")) {
      if (sql[token.start - 1] === ":") continue; // PostgreSQL ::type
      const name = text.slice(1);
      if (!NAMED_PATTERN.test(name) || name.length > MAX_NAME_LENGTH) {
        unsupported.push(text);
        continue;
      }
      const statementIndex = statementIndexFor(statements, token.start, token.end);
      const key = `:${name}`;
      const existing = definitions.get(key);
      if (existing) {
        existing.occurrences += 1;
        if (!existing.statements.includes(statementIndex)) {
          existing.statements.push(statementIndex);
        }
      } else {
        definitions.set(key, {
          key,
          name,
          kind: "named",
          statements: [statementIndex],
          occurrences: 1,
        });
      }
      occurrences.push({
        key,
        statementIndex,
        from: token.start,
        to: token.end,
      });
      continue;
    }
    if (text.startsWith("@") || text.startsWith("$")) {
      unsupported.push(text);
      continue;
    }
    if (text.startsWith("?") && text !== "?") {
      unsupported.push(text);
    }
  }

  const namedStatements = new Map<number, boolean>();
  const positionalStatements = new Map<number, boolean>();
  for (const occurrence of occurrences) {
    if (occurrence.key.startsWith(":")) namedStatements.set(occurrence.statementIndex, true);
    else positionalStatements.set(occurrence.statementIndex, true);
  }
  for (const index of positionalStatements.keys()) {
    if (namedStatements.has(index)) mixedStatements.add(index);
  }
  if (mixedStatements.size) {
    errors.push("同一条语句不能混用 :name 与 ? 参数");
  }
  if ((positionalCount > 0 || positionalStatements.size > 0) && statements.length > 1) {
    errors.push("位置参数 ? 仅支持单条语句；请改用 :name 或分别执行");
  }
  if (definitions.size > MAX_PARAMETERS) {
    errors.push(`一次最多使用 ${MAX_PARAMETERS} 个唯一参数`);
  }
  return {
    definitions: [...definitions.values()],
    occurrences,
    errors,
    unsupported,
    statementCount: statements.length,
    tooLarge: false,
  };
}

export function validateParameterValue(
  value: SqlParameterValue | undefined,
): string | null {
  if (!value) return "参数未填写";
  if (value.type === "null") return null;
  if (value.type === "string" && !value.touched) return "参数未填写";
  if (value.value.length > MAX_VALUE_BYTES) return "参数值不能超过 10 KB";
  if (value.type === "string") return null;
  if (value.type === "number") {
    if (!NUMBER_PATTERN.test(value.value.trim())) return "数字格式无效";
    if (!Number.isFinite(Number(value.value))) return "数字必须是有限值";
    return null;
  }
  if (value.type === "boolean") {
    return value.value === "true" || value.value === "false" ? null : "布尔值无效";
  }
  if (value.type === "raw") {
    if (!value.value.trim()) return "原始 SQL 不能为空";
    if (!value.rawConfirmed) return "请先确认原始 SQL 是可信片段";
    if (/^\s*(--|#|\/\*)/.test(value.value)) return "原始 SQL 不能以注释开头";
    const statements = executableStatements(value.value);
    if (statements.length !== 1) return "原始 SQL 必须是单条且不能包含顶层分号";
    return null;
  }
  return "未知的参数类型";
}

function parameterLiteral(
  value: SqlParameterValue,
  driver: Driver,
  redactSensitive: boolean,
): string {
  if (redactSensitive && value.sensitive) return "'••••'";
  if (value.type === "null") return "NULL";
  if (value.type === "boolean") return value.value === "true" ? "TRUE" : "FALSE";
  if (value.type === "number") return value.value.trim();
  if (value.type === "raw") return value.value;
  return sqlLiteral(value.value, driver);
}

export function substituteSqlParameters(
  sql: string,
  values: Record<string, SqlParameterValue>,
  driver: Driver,
  redactSensitive = false,
): SqlParameterSubstitution {
  const extracted = extractSqlParameters(sql, driver);
  if (extracted.errors.length) {
    return { sql, applied: 0, missing: [], errors: extracted.errors };
  }
  const errors: string[] = [];
  const missing: string[] = [];
  const valid = new Map<string, SqlParameterValue>();
  for (const definition of extracted.definitions) {
    const value = values[definition.key];
    const error = validateParameterValue(value);
    if (error) {
      if (!value) missing.push(definition.key);
      else errors.push(`${definition.key}：${error}`);
      continue;
    }
    valid.set(definition.key, value);
  }
  if (missing.length || errors.length) {
    return { sql, applied: 0, missing, errors };
  }
  let output = sql;
  for (const occurrence of [...extracted.occurrences].sort((a, b) => b.from - a.from)) {
    const value = valid.get(occurrence.key);
    if (!value) continue;
    output =
      output.slice(0, occurrence.from) +
      parameterLiteral(value, driver, redactSensitive) +
      output.slice(occurrence.to);
  }
  const before = executableStatements(sql).length;
  const after = executableStatements(output).length;
  if (before !== after) {
    return {
      sql,
      applied: 0,
      missing: [],
      errors: ["参数替换后语句数量发生变化，请检查原始 SQL 参数是否包含分号"],
    };
  }
  return {
    sql: output,
    applied: extracted.definitions.length,
    missing: [],
    errors: [],
  };
}

export function parameterValuesSignature(
  values: Record<string, SqlParameterValue>,
): string {
  return JSON.stringify(
    Object.keys(values)
      .sort()
      .map((key) => [key, values[key]?.type, values[key]?.value, values[key]?.rawConfirmed ?? false]),
  );
}

export function maskedParameterValue(value: SqlParameterValue): SqlParameterValue {
  if (!value.sensitive) return value;
  return { ...value, value: value.type === "raw" ? "*" : "••••" };
}

export function defaultParameterValue(
  definition: SqlParameterDefinition,
): SqlParameterValue {
  const type = definition.kind === "positional" ? "number" : "string";
  return { type, value: "", touched: false };
}

export function parameterDefinitionsSignature(
  definitions: SqlParameterDefinition[],
): string {
  return JSON.stringify(
    definitions.map((definition) => [
      definition.key,
      definition.kind,
      definition.occurrences,
      definition.statements,
    ]),
  );
}
