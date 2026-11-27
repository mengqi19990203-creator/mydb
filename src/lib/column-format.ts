import { formatGridCell } from "./grid-search";

export type ColumnFormatType =
  | "date"
  | "number"
  | "jsonPath"
  | "mask"
  | "template";

export type DateFormatSource =
  | "auto"
  | "iso"
  | "epochSeconds"
  | "epochMillis"
  | "extendedJson";

export type DateFormatTimezone = "local" | "utc";
export type JsonPathOutput = "scalar" | "json";
export type MaskMode = "all" | "keep" | "email";

export interface DateFormatOptions {
  source: DateFormatSource;
  pattern: string;
  timezone: DateFormatTimezone;
}

export interface NumberFormatOptions {
  decimals: number;
  thousands: boolean;
  percent: boolean;
  prefix: string;
  suffix: string;
}

export interface JsonPathFormatOptions {
  path: string;
  output: JsonPathOutput;
}

export interface MaskFormatOptions {
  mode: MaskMode;
  keepStart: number;
  keepEnd: number;
  maskChar: "•" | "*";
}

export interface TemplateFormatOptions {
  template: string;
}

export interface ColumnFormatRule {
  type: ColumnFormatType;
  date?: DateFormatOptions;
  number?: NumberFormatOptions;
  jsonPath?: JsonPathFormatOptions;
  mask?: MaskFormatOptions;
  template?: TemplateFormatOptions;
}

export interface ColumnFormatEntry {
  columnKey: string;
  rule: ColumnFormatRule;
}

export interface FormattedValue {
  text: string;
  warning: string | null;
}

const STORAGE_KEY = "mydb.columnFormatters.v1";
const STORAGE_VERSION = 1;
const MAX_RULES_PER_TABLE = 50;
const MAX_SCOPES = 200;
const MAX_JSON_BYTES = 2 * 1024 * 1024;

const DATE_TOKENS = new Set(["YYYY", "MM", "DD", "HH", "mm", "ss", "SSS"]);

function rawText(value: unknown): string {
  return formatGridCell(value);
}

function fallback(value: unknown, warning: string): FormattedValue {
  return { text: rawText(value), warning };
}

export function formatColumnKeys(
  columns: ReadonlyArray<{ name: string }>,
): string[] {
  const counts = new Map<string, number>();
  for (const column of columns) {
    counts.set(column.name, (counts.get(column.name) ?? 0) + 1);
  }
  return columns.map((column, index) =>
    (counts.get(column.name) ?? 0) > 1
      ? `${column.name}#${index}`
      : column.name,
  );
}

export function formatColumnKey(
  name: string,
  index: number,
  columns: ReadonlyArray<{ name: string }>,
): string {
  return formatColumnKeys(columns)[index] ?? name;
}

export function validateRule(rule: ColumnFormatRule): string | null {
  if (!["date", "number", "jsonPath", "mask", "template"].includes(rule.type)) {
    return "未知的格式类型";
  }
  if (rule.type === "date") {
    const options = rule.date;
    if (!options) return "缺少日期格式参数";
    if (!options.pattern.trim()) return "日期格式不能为空";
    if (options.pattern.length > 80) return "日期格式不能超过 80 个字符";
    if (!["auto", "iso", "epochSeconds", "epochMillis", "extendedJson"].includes(options.source)) {
      return "日期来源无效";
    }
    if (options.timezone !== "local" && options.timezone !== "utc") {
      return "时区必须是本地或 UTC";
    }
    // Custom text is allowed, but unknown tokens are shown literally.
    const tokens = options.pattern.match(/YYYY|MM|DD|HH|mm|ss|SSS/g) ?? [];
    if (tokens.some((token) => !DATE_TOKENS.has(token))) {
      return "日期格式包含不支持的 token";
    }
    return null;
  }
  if (rule.type === "number") {
    const options = rule.number;
    if (!options) return "缺少数字格式参数";
    if (!Number.isInteger(options.decimals) || options.decimals < 0 || options.decimals > 6) {
      return "小数位必须在 0–6 之间";
    }
    if (options.prefix.length > 20 || options.suffix.length > 20) {
      return "前缀 / 后缀不能超过 20 个字符";
    }
    return null;
  }
  if (rule.type === "jsonPath") {
    const options = rule.jsonPath;
    if (!options) return "缺少 JSON 路径参数";
    if (!options.path.trim()) return "JSON 路径不能为空";
    if (options.path.length > 200) return "JSON 路径不能超过 200 个字符";
    if (options.output !== "scalar" && options.output !== "json") {
      return "JSON 输出方式无效";
    }
    const parsed = parseJsonPath(options.path);
    return parsed.error;
  }
  if (rule.type === "mask") {
    const options = rule.mask;
    if (!options) return "缺少脱敏参数";
    if (!["all", "keep", "email"].includes(options.mode)) return "脱敏模式无效";
    if (!Number.isInteger(options.keepStart) || options.keepStart < 0 || options.keepStart > 20) {
      return "保留前 N 位必须在 0–20 之间";
    }
    if (!Number.isInteger(options.keepEnd) || options.keepEnd < 0 || options.keepEnd > 20) {
      return "保留后 M 位必须在 0–20 之间";
    }
    if (options.maskChar !== "•" && options.maskChar !== "*") {
      return "掩码字符必须是 • 或 *";
    }
    return null;
  }
  const options = rule.template;
  if (!options) return "缺少模板参数";
  if (!options.template.includes("{value}")) return "模板必须包含 {value}";
  if (options.template.length > 200) return "模板不能超过 200 个字符";
  return null;
}

export function formatRuleSummary(rule: ColumnFormatRule): string {
  if (rule.type === "date") {
    return `日期：${rule.date?.pattern ?? ""}`;
  }
  if (rule.type === "number") {
    const parts = [];
    if (rule.number?.percent) parts.push("百分比");
    if (rule.number?.thousands) parts.push("千分位");
    parts.push(`${rule.number?.decimals ?? 0} 位小数`);
    return `数字：${parts.join(" / ")}`;
  }
  if (rule.type === "jsonPath") {
    return `JSON 路径：${rule.jsonPath?.path ?? ""}`;
  }
  if (rule.type === "mask") {
    if (rule.mask?.mode === "all") return "脱敏：全部";
    if (rule.mask?.mode === "email") return "脱敏：邮箱";
    return `脱敏：保留前 ${rule.mask?.keepStart ?? 0} 后 ${rule.mask?.keepEnd ?? 0}`;
  }
  return "模板";
}

interface ParsedPath {
  segments: Array<string | number>;
  error: string | null;
}

export function parseJsonPath(path: string): ParsedPath {
  const trimmed = path.trim();
  if (!trimmed) return { segments: [], error: "JSON 路径不能为空" };
  if (trimmed.length > 200) return { segments: [], error: "JSON 路径不能超过 200 个字符" };
  let rest = trimmed;
  if (rest.startsWith("$")) rest = rest.slice(1);
  if (rest.startsWith(".")) rest = rest.slice(1);
  const segments: Array<string | number> = [];
  if (!rest) return { segments, error: null };
  let guard = 0;
  while (rest.length) {
    guard += 1;
    if (guard > 20) return { segments: [], error: "JSON 路径层级过深" };
    const bracket = rest.indexOf("[");
    const dot = rest.indexOf(".");
    let keyEnd = rest.length;
    if (bracket >= 0) keyEnd = Math.min(keyEnd, bracket);
    if (dot >= 0) keyEnd = Math.min(keyEnd, dot);
    const key = rest.slice(0, keyEnd);
    if (key) {
      if (key.includes("]")) return { segments: [], error: "JSON 路径格式无效" };
      segments.push(key);
      if (segments.length > 10) return { segments: [], error: "JSON 路径深度不能超过 10 层" };
    } else if (rest[0] !== "[") {
      return { segments: [], error: "JSON 路径格式无效" };
    }
    rest = rest.slice(keyEnd);
    if (rest.startsWith("[")) {
      const match = /^\[(\d+)\]/.exec(rest);
      if (!match) return { segments: [], error: "JSON 路径只支持数字数组下标" };
      segments.push(Number(match[1]));
      if (segments.length > 10) return { segments: [], error: "JSON 路径深度不能超过 10 层" };
      rest = rest.slice(match[0].length);
    }
    if (rest.startsWith(".")) rest = rest.slice(1);
    else if (rest.length) return { segments: [], error: "JSON 路径格式无效" };
  }
  return { segments, error: null };
}

export function extractJsonPath(
  value: unknown,
  path: string,
): { found: boolean; value?: unknown; error?: string } {
  const parsed = parseJsonPath(path);
  if (parsed.error) return { found: false, error: parsed.error };
  let current = value;
  for (const segment of parsed.segments) {
    if (typeof segment === "number") {
      if (!Array.isArray(current) || segment >= current.length) {
        return { found: false };
      }
      current = current[segment];
    } else {
      if (!current || typeof current !== "object" || Array.isArray(current)) {
        return { found: false };
      }
      if (!Object.prototype.hasOwnProperty.call(current, segment)) {
        return { found: false };
      }
      current = (current as Record<string, unknown>)[segment];
    }
  }
  return { found: true, value: current };
}

export function parseDateValue(
  value: unknown,
  source: DateFormatSource,
  _timezone: DateFormatTimezone,
): { date: Date | null; warning: string | null } {
  const parseExtended = (input: unknown): Date | null => {
    if (!input || typeof input !== "object" || Array.isArray(input)) return null;
    const raw = (input as Record<string, unknown>).$date;
    if (typeof raw === "string") {
      const date = new Date(raw);
      return Number.isNaN(date.getTime()) ? null : date;
    }
    if (raw && typeof raw === "object") {
      const number = (raw as Record<string, unknown>).$numberLong;
      if (typeof number === "string" && /^-?\d+$/.test(number)) {
        const date = new Date(Number(number));
        return Number.isNaN(date.getTime()) ? null : date;
      }
    }
    return null;
  };

  if (source === "extendedJson") {
    const date = parseExtended(value);
    return date
      ? { date, warning: null }
      : { date: null, warning: "Extended JSON $date 无效" };
  }

  if (source === "auto") {
    const extended = parseExtended(value);
    if (extended) return { date: extended, warning: null };
    if (typeof value === "string" && /^\d{4}-\d{2}-\d{2}T/.test(value)) {
      const date = new Date(value);
      return Number.isNaN(date.getTime())
        ? { date: null, warning: "日期无效" }
        : { date, warning: null };
    }
    return { date: null, warning: "自动识别只支持 ISO 8601 或 Extended JSON $date" };
  }

  if (source === "iso") {
    if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T/.test(value)) {
      return { date: null, warning: "值不是 ISO 8601 日期字符串" };
    }
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? { date: null, warning: "日期无效" }
      : { date, warning: null };
  }

  const numberValue =
    typeof value === "number"
      ? value
      : typeof value === "string" && /^-?\d+(\.\d+)?$/.test(value)
        ? Number(value)
        : null;
  if (numberValue === null || !Number.isFinite(numberValue)) {
    return { date: null, warning: "值不是有效的 epoch 数字" };
  }
  const millis = source === "epochSeconds" ? numberValue * 1000 : numberValue;
  const date = new Date(millis);
  return Number.isNaN(date.getTime())
    ? { date: null, warning: "日期无效" }
    : { date, warning: null };
}

function pad(value: number, length = 2): string {
  return String(value).padStart(length, "0");
}

export function applyDatePattern(
  date: Date,
  pattern: string,
  timezone: DateFormatTimezone,
): string {
  const utc = timezone === "utc";
  return pattern.replace(/YYYY|MM|DD|HH|mm|ss|SSS/g, (token) => {
    if (token === "YYYY") return pad(utc ? date.getUTCFullYear() : date.getFullYear(), 4);
    if (token === "MM") return pad((utc ? date.getUTCMonth() : date.getMonth()) + 1);
    if (token === "DD") return pad(utc ? date.getUTCDate() : date.getDate());
    if (token === "HH") return pad(utc ? date.getUTCHours() : date.getHours());
    if (token === "mm") return pad(utc ? date.getUTCMinutes() : date.getMinutes());
    if (token === "ss") return pad(utc ? date.getUTCSeconds() : date.getSeconds());
    return pad(utc ? date.getUTCMilliseconds() : date.getMilliseconds(), 3);
  });
}

export function formatNumber(
  value: number,
  options: NumberFormatOptions,
): string {
  const adjusted = options.percent ? value * 100 : value;
  let text = adjusted.toFixed(options.decimals);
  if (options.thousands) {
    const sign = text.startsWith("-") ? "-" : "";
    if (sign) text = text.slice(1);
    const [integer, fraction] = text.split(".");
    const grouped = integer.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    text = `${sign}${grouped}${fraction === undefined ? "" : `.${fraction}`}`;
  }
  return `${options.prefix}${text}${options.suffix}`;
}

export function maskValue(
  value: string,
  options: MaskFormatOptions,
): { text: string; warning: string | null } {
  if (!value) return { text: value, warning: null };
  const mask = (length: number) => options.maskChar.repeat(Math.max(0, length));
  if (options.mode === "all") {
    return { text: mask(value.length), warning: null };
  }
  if (options.mode === "email") {
    const at = value.lastIndexOf("@");
    if (at <= 0) return { text: mask(value.length), warning: "字符串不是邮箱格式，已全部掩码" };
    const local = value.slice(0, at);
    return {
      text: `${local.slice(0, 1)}${mask(Math.max(1, local.length - 1))}${value.slice(at)}`,
      warning: null,
    };
  }
  const { keepStart, keepEnd } = options;
  if (value.length <= keepStart + keepEnd) {
    return { text: mask(value.length), warning: "字符串长度不足以保留前后位数，已全部掩码" };
  }
  return {
    text:
      value.slice(0, keepStart) +
      mask(value.length - keepStart - keepEnd) +
      (keepEnd ? value.slice(-keepEnd) : ""),
    warning: null,
  };
}

export function applyTemplate(value: unknown, template: string): FormattedValue {
  if (typeof value === "object" && value !== null) {
    return fallback(value, "模板只支持标量值");
  }
  return {
    text: template.replaceAll("{value}", value === null || value === undefined ? "NULL" : String(value)),
    warning: null,
  };
}

function formatJsonPathValue(value: unknown, options: JsonPathFormatOptions): FormattedValue {
  let parsed: unknown = value;
  if (typeof value === "string") {
    if (value.length > 1024 * 1024) {
      return fallback(value, "文本超过 1 MB，未执行 JSON 路径提取");
    }
    try {
      parsed = JSON.parse(value);
    } catch {
      return fallback(value, "值不是合法 JSON");
    }
  } else if (typeof value !== "object" || value === null) {
    return fallback(value, "JSON 路径只适用于 JSON 字符串或对象");
  }
  const extracted = extractJsonPath(parsed, options.path);
  if (extracted.error) return fallback(value, extracted.error);
  if (!extracted.found) return fallback(value, "JSON 路径不存在");
  const extractedValue = extracted.value;
  if (options.output === "json") {
    const json = JSON.stringify(extractedValue);
    return { text: json === undefined ? "undefined" : json, warning: null };
  }
  if (typeof extractedValue === "string") return { text: extractedValue, warning: null };
  if (extractedValue === null || extractedValue === undefined) return { text: "null", warning: null };
  if (typeof extractedValue === "object") {
    return { text: JSON.stringify(extractedValue), warning: null };
  }
  return { text: String(extractedValue), warning: null };
}

const validationCache = new WeakMap<ColumnFormatRule, string | null>();

export function formatDisplayValue(
  value: unknown,
  rule: ColumnFormatRule,
): FormattedValue {
  if (value === null || value === undefined) return { text: "NULL", warning: null };
  let invalid: string | null;
  if (validationCache.has(rule)) {
    invalid = validationCache.get(rule) ?? null;
  } else {
    invalid = validateRule(rule);
    validationCache.set(rule, invalid);
  }
  if (invalid) return fallback(value, invalid);

  if (rule.type === "date" && rule.date) {
    const parsed = parseDateValue(value, rule.date.source, rule.date.timezone);
    if (!parsed.date) return fallback(value, parsed.warning ?? "日期无效");
    return {
      text: applyDatePattern(parsed.date, rule.date.pattern, rule.date.timezone),
      warning: null,
    };
  }
  if (rule.type === "number" && rule.number) {
    if (typeof value !== "number") return { text: rawText(value), warning: null };
    if (!Number.isFinite(value)) return fallback(value, "数字不是有限值");
    return { text: formatNumber(value, rule.number), warning: null };
  }
  if (rule.type === "jsonPath" && rule.jsonPath) {
    return formatJsonPathValue(value, rule.jsonPath);
  }
  if (rule.type === "mask" && rule.mask) {
    if (typeof value !== "string") return { text: rawText(value), warning: null };
    return maskValue(value, rule.mask);
  }
  if (rule.type === "template" && rule.template) {
    return applyTemplate(value, rule.template.template);
  }
  return fallback(value, "格式规则不完整");
}

export function normalizeRule(raw: unknown): ColumnFormatRule | null {
  if (!raw || typeof raw !== "object") return null;
  const record = raw as Record<string, unknown>;
  const type = record.type;
  if (typeof type !== "string") return null;
  const candidate: ColumnFormatRule = { type: type as ColumnFormatType };
  if (type === "date" && record.date && typeof record.date === "object") {
    const options = record.date as Record<string, unknown>;
    candidate.date = {
      source: (options.source as DateFormatSource) ?? "auto",
      pattern: String(options.pattern ?? ""),
      timezone: (options.timezone as DateFormatTimezone) ?? "local",
    };
  } else if (type === "number" && record.number && typeof record.number === "object") {
    const options = record.number as Record<string, unknown>;
    candidate.number = {
      decimals: Number(options.decimals ?? 2),
      thousands: Boolean(options.thousands),
      percent: Boolean(options.percent),
      prefix: String(options.prefix ?? ""),
      suffix: String(options.suffix ?? ""),
    };
  } else if (type === "jsonPath" && record.jsonPath && typeof record.jsonPath === "object") {
    const options = record.jsonPath as Record<string, unknown>;
    candidate.jsonPath = {
      path: String(options.path ?? ""),
      output: options.output === "json" ? "json" : "scalar",
    };
  } else if (type === "mask" && record.mask && typeof record.mask === "object") {
    const options = record.mask as Record<string, unknown>;
    candidate.mask = {
      mode: (options.mode as MaskMode) ?? "all",
      keepStart: Number(options.keepStart ?? 3),
      keepEnd: Number(options.keepEnd ?? 4),
      maskChar: options.maskChar === "*" ? "*" : "•",
    };
  } else if (type === "template" && record.template && typeof record.template === "object") {
    const options = record.template as Record<string, unknown>;
    candidate.template = { template: String(options.template ?? "") };
  } else {
    return null;
  }
  return validateRule(candidate) ? null : candidate;
}

export function pruneFormats(
  entries: ColumnFormatEntry[],
  validKeys: ReadonlySet<string>,
): ColumnFormatEntry[] {
  return entries.filter((entry) => validKeys.has(entry.columnKey));
}

export function columnFormatScopeKey(parts: {
  driver?: string;
  connectionId: string;
  database?: string | null;
  schema?: string | null;
  table: string;
  tableKind?: string | null;
}): string {
  return [
    parts.driver ?? "",
    parts.connectionId,
    parts.database ?? "",
    parts.schema ?? "",
    parts.table,
    parts.tableKind ?? "",
  ].join("|");
}

interface StoredScope {
  updatedAt: number;
  rules: Record<string, ColumnFormatRule>;
}

interface StoredFormats {
  version: number;
  scopes: Record<string, StoredScope>;
}

function emptyStored(): StoredFormats {
  return { version: STORAGE_VERSION, scopes: {} };
}

function readStored(): StoredFormats {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return emptyStored();
    const parsed = JSON.parse(raw) as Partial<StoredFormats>;
    if (parsed.version !== STORAGE_VERSION || !parsed.scopes || typeof parsed.scopes !== "object") {
      localStorage.removeItem(STORAGE_KEY);
      return emptyStored();
    }
    const scopes: Record<string, StoredScope> = {};
    for (const [key, value] of Object.entries(parsed.scopes)) {
      if (!value || typeof value !== "object") continue;
      const record = value as Partial<StoredScope>;
      if (!record.rules || typeof record.rules !== "object") continue;
      const rules: Record<string, ColumnFormatRule> = {};
      for (const [columnKey, rawRule] of Object.entries(record.rules)) {
        const rule = normalizeRule(rawRule);
        if (rule) rules[columnKey] = rule;
      }
      scopes[key] = {
        updatedAt: typeof record.updatedAt === "number" ? record.updatedAt : Date.now(),
        rules,
      };
    }
    return { version: STORAGE_VERSION, scopes };
  } catch {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // Ignore storage failures.
    }
    return emptyStored();
  }
}

export function loadTableFormats(scopeKey: string): ColumnFormatEntry[] {
  const stored = readStored();
  const scope = stored.scopes[scopeKey];
  if (!scope) return [];
  return Object.entries(scope.rules)
    .map(([columnKey, rule]) => ({ columnKey, rule }))
    .slice(0, MAX_RULES_PER_TABLE);
}

export function saveTableFormats(
  scopeKey: string,
  entries: ColumnFormatEntry[],
): void {
  const stored = readStored();
  const rules: Record<string, ColumnFormatRule> = {};
  for (const entry of entries.slice(0, MAX_RULES_PER_TABLE)) {
    if (!entry.columnKey) continue;
    const normalized = normalizeRule(entry.rule);
    if (normalized) rules[entry.columnKey] = normalized;
  }
  if (Object.keys(rules).length) {
    stored.scopes[scopeKey] = { updatedAt: Date.now(), rules };
  } else {
    delete stored.scopes[scopeKey];
  }
  const scopeKeys = Object.keys(stored.scopes);
  if (scopeKeys.length > MAX_SCOPES) {
    scopeKeys
      .sort((left, right) => (stored.scopes[left]?.updatedAt ?? 0) - (stored.scopes[right]?.updatedAt ?? 0))
      .slice(0, scopeKeys.length - MAX_SCOPES)
      .forEach((key) => delete stored.scopes[key]);
  }
  try {
    const raw = JSON.stringify(stored);
    if (raw.length > MAX_JSON_BYTES) return;
    localStorage.setItem(STORAGE_KEY, raw);
  } catch {
    // Keep the current session state when persistence fails.
  }
}

export function clearTableFormats(scopeKey: string): void {
  const stored = readStored();
  if (!stored.scopes[scopeKey]) return;
  delete stored.scopes[scopeKey];
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
  } catch {
    // Ignore storage failures.
  }
}
