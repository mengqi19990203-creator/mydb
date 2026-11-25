export type SnippetDriver = "any" | "mysql" | "postgres" | "sqlite";

export interface SqlSnippet {
  id: string;
  name: string;
  trigger: string;
  body: string;
  tags: string[];
  driver: SnippetDriver;
  builtin: boolean;
  createdAt: string;
  updatedAt: string;
}

interface SnippetsFile {
  version: number;
  snippets: SqlSnippet[];
}

export const SNIPPETS_STORAGE_KEY = "mydb.sqlSnippets.v1";
export const SNIPPET_LIMIT = 500;
export const SNIPPET_BODY_LIMIT = 50 * 1024;
export const SNIPPET_IMPORT_LIMIT = 2 * 1024 * 1024;

const CREATED_AT = "2026-01-01T00:00:00.000Z";

function builtin(
  id: string,
  name: string,
  trigger: string,
  body: string,
  tags: string[],
  driver: SnippetDriver = "any",
): SqlSnippet {
  return {
    id: `builtin-${id}`,
    name,
    trigger,
    body,
    tags,
    driver,
    builtin: true,
    createdAt: CREATED_AT,
    updatedAt: CREATED_AT,
  };
}

/** Read-only starter snippets; always merged ahead of the user's own list. */
export const BUILTIN_SNIPPETS: SqlSnippet[] = [
  builtin(
    "sel",
    "查询全部",
    "sel",
    "SELECT *\nFROM ${cursor};",
    ["查询"],
  ),
  builtin(
    "cnt",
    "统计行数",
    "cnt",
    "SELECT COUNT(*) AS total\nFROM ${cursor};",
    ["查询", "统计"],
  ),
  builtin(
    "join",
    "内连接模板",
    "join",
    "SELECT a.*, b.*\nFROM table_a AS a\nINNER JOIN table_b AS b ON b.id = a.${cursor}",
    ["查询", "JOIN"],
  ),
  builtin(
    "ins",
    "插入模板",
    "ins",
    "INSERT INTO table_name (column1, column2)\nVALUES (${cursor});",
    ["写入"],
  ),
  builtin(
    "upd",
    "更新模板",
    "upd",
    "UPDATE table_name\nSET column1 = ${cursor}\nWHERE id = 1;",
    ["写入"],
  ),
  builtin(
    "tx",
    "事务包裹",
    "tx",
    "BEGIN;\n\n${cursor}\n\nCOMMIT;",
    ["事务"],
  ),
];

export function newSnippet(body = "", driver: SnippetDriver = "any"): SqlSnippet {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID?.() ?? `snippet-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    name: "",
    trigger: "",
    body,
    tags: [],
    driver,
    builtin: false,
    createdAt: now,
    updatedAt: now,
  };
}

function normalizeSnippet(raw: unknown): SqlSnippet | null {
  if (!raw || typeof raw !== "object") return null;
  const item = raw as Record<string, unknown>;
  if (typeof item.name !== "string" || typeof item.trigger !== "string") {
    return null;
  }
  if (typeof item.body !== "string") return null;
  const driver = ["any", "mysql", "postgres", "sqlite"].includes(
    String(item.driver),
  )
    ? (item.driver as SnippetDriver)
    : "any";
  const tags = Array.isArray(item.tags)
    ? item.tags.filter((tag): tag is string => typeof tag === "string").slice(0, 20)
    : [];
  const now = new Date().toISOString();
  return {
    id:
      typeof item.id === "string" && item.id
        ? item.id
        : `snippet-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    name: item.name.trim(),
    trigger: item.trigger.trim(),
    body: item.body,
    tags: tags.map((tag) => tag.trim()).filter(Boolean),
    driver,
    builtin: false,
    createdAt: typeof item.createdAt === "string" ? item.createdAt : now,
    updatedAt: typeof item.updatedAt === "string" ? item.updatedAt : now,
  };
}

export function loadUserSnippets(): SqlSnippet[] {
  try {
    const raw = window.localStorage.getItem(SNIPPETS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as SnippetsFile;
    if (!parsed || parsed.version !== 1 || !Array.isArray(parsed.snippets)) {
      return [];
    }
    return parsed.snippets
      .map(normalizeSnippet)
      .filter((snippet): snippet is SqlSnippet => Boolean(snippet));
  } catch {
    return [];
  }
}

export function saveUserSnippets(snippets: SqlSnippet[]): void {
  const file: SnippetsFile = {
    version: 1,
    snippets: snippets.filter((snippet) => !snippet.builtin).slice(0, SNIPPET_LIMIT),
  };
  window.localStorage.setItem(SNIPPETS_STORAGE_KEY, JSON.stringify(file));
}

export function allSnippets(userSnippets: SqlSnippet[]): SqlSnippet[] {
  return [...BUILTIN_SNIPPETS, ...userSnippets];
}

export function triggerIssue(
  trigger: string,
  snippets: SqlSnippet[],
  exceptId?: string,
): string {
  const value = trigger.trim();
  if (!value) return "trigger 不能为空";
  if (!/^[A-Za-z0-9_-]+$/.test(value)) {
    return "trigger 只能包含字母、数字、下划线和短横线";
  }
  if (
    snippets.some(
      (snippet) =>
        snippet.id !== exceptId &&
        snippet.trigger.toLowerCase() === value.toLowerCase(),
    )
  ) {
    return "trigger 已被其他片段使用";
  }
  return "";
}

export function validateSnippet(
  snippet: SqlSnippet,
  all: SqlSnippet[],
): string {
  if (!snippet.name.trim()) return "名称不能为空";
  const issue = triggerIssue(snippet.trigger, all, snippet.id);
  if (issue) return issue;
  if (!snippet.body.trim()) return "SQL 正文不能为空";
  if (new TextEncoder().encode(snippet.body).length > SNIPPET_BODY_LIMIT) {
    return "单条片段正文不能超过 50 KB";
  }
  if (snippet.tags.length > 20) return "标签最多 20 个";
  return "";
}

export function driverMatches(
  snippet: SqlSnippet,
  driver: SnippetDriver | null,
): boolean {
  return snippet.driver === "any" || !driver || snippet.driver === driver;
}

export function searchSnippets(
  snippets: SqlSnippet[],
  term: string,
): SqlSnippet[] {
  const query = term.trim().toLowerCase();
  if (!query) return snippets;
  return snippets.filter((snippet) =>
    [
      snippet.name,
      snippet.trigger,
      snippet.tags.join(" "),
      snippet.body,
    ]
      .join("\n")
      .toLowerCase()
      .includes(query),
  );
}

export function uniqueTrigger(base: string, snippets: SqlSnippet[]): string {
  const used = new Set(snippets.map((snippet) => snippet.trigger.toLowerCase()));
  if (!used.has(base.toLowerCase())) return base;
  let index = 2;
  while (used.has(`${base}-${index}`.toLowerCase())) index += 1;
  return `${base}-${index}`;
}

export interface ImportResult {
  snippets: SqlSnippet[];
  imported: number;
  skipped: number;
}

/** Parses and validates an exported JSON file. The caller checks file size. */
export function parseSnippetsJson(
  raw: string,
  existing: SqlSnippet[],
): ImportResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("JSON 格式无效");
  }
  const file = parsed as { version?: number; snippets?: unknown };
  if (file?.version !== 1 || !Array.isArray(file.snippets)) {
    throw new Error("不是有效的 mydb 片段文件（需要 version: 1 与 snippets 数组）");
  }
  const imported: SqlSnippet[] = [];
  let skipped = 0;
  const pool = [...existing];
  for (const rawSnippet of file.snippets) {
    const snippet = normalizeSnippet(rawSnippet);
    if (!snippet) {
      skipped += 1;
      continue;
    }
    if (new TextEncoder().encode(snippet.body).length > SNIPPET_BODY_LIMIT) {
      skipped += 1;
      continue;
    }
    if (!snippet.name || !snippet.body.trim()) {
      skipped += 1;
      continue;
    }
    snippet.trigger = uniqueTrigger(
      snippet.trigger.replace(/[^A-Za-z0-9_-]/g, "-") || "snippet",
      pool,
    );
    pool.push(snippet);
    imported.push(snippet);
  }
  if (!imported.length && skipped) {
    throw new Error("文件里没有可导入的有效片段");
  }
  return { snippets: imported, imported: imported.length, skipped };
}

export function exportSnippetsJson(userSnippets: SqlSnippet[]): string {
  const file: SnippetsFile = {
    version: 1,
    snippets: userSnippets.filter((snippet) => !snippet.builtin),
  };
  return JSON.stringify(file, null, 2);
}