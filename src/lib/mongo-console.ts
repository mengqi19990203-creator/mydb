export const MONGO_CONSOLE_DRAFT_KEY = "mydb.mongoConsoleDraft.v1";
export const MONGO_CONSOLE_MAX_BYTES = 100 * 1024;
export const MONGO_CONSOLE_MAX_STATEMENTS = 20;

export interface MongoStatementRange {
  text: string;
  start: number;
  end: number;
}

function stripComments(input: string): string {
  let output = "";
  let quote: string | null = null;
  let lineComment = false;
  let blockComment = false;
  for (let index = 0; index < input.length; index += 1) {
    const character = input[index];
    if (lineComment) {
      if (character === "\n") {
        lineComment = false;
        output += "\n";
      }
      continue;
    }
    if (blockComment) {
      if (character === "*" && input[index + 1] === "/") {
        blockComment = false;
        index += 1;
      } else if (character === "\n") {
        output += "\n";
      }
      continue;
    }
    if (quote) {
      output += character;
      if (character === "\\") {
        output += input[index + 1] ?? "";
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'" || character === "`") {
      quote = character;
      output += character;
      continue;
    }
    if (character === "/" && input[index + 1] === "/") {
      lineComment = true;
      index += 1;
      continue;
    }
    if (character === "/" && input[index + 1] === "*") {
      blockComment = true;
      index += 1;
      continue;
    }
    output += character;
  }
  return output;
}

export function hasMongoContent(text: string): boolean {
  return stripComments(text).trim().length > 0;
}

function isLineCommand(text: string): boolean {
  const lower = stripComments(text).trim().toLocaleLowerCase();
  return (
    lower.startsWith("use ") ||
    lower === "use" ||
    lower === "show dbs" ||
    lower === "show databases" ||
    lower === "show collections"
  );
}

/** Mirrors the Rust splitter: top-level semicolons, plus use/show line ends. */
export function splitMongoStatements(script: string): MongoStatementRange[] {
  const result: MongoStatementRange[] = [];
  let start = 0;
  let quote: string | null = null;
  let lineComment = false;
  let blockComment = false;
  let depth = 0;

  const push = (end: number) => {
    const text = script.slice(start, end);
    if (hasMongoContent(text)) {
      result.push({ text, start, end });
    }
  };

  for (let index = 0; index < script.length; index += 1) {
    const character = script[index];
    if (lineComment) {
      if (character === "\n") {
        lineComment = false;
        if (depth === 0 && isLineCommand(script.slice(start, index))) {
          push(index);
          start = index + 1;
        }
      }
      continue;
    }
    if (blockComment) {
      if (character === "*" && script[index + 1] === "/") {
        blockComment = false;
        index += 1;
      }
      continue;
    }
    if (quote) {
      if (character === "\\") {
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'" || character === "`") {
      quote = character;
      continue;
    }
    if (character === "/" && script[index + 1] === "/") {
      lineComment = true;
      index += 1;
      continue;
    }
    if (character === "/" && script[index + 1] === "*") {
      blockComment = true;
      index += 1;
      continue;
    }
    if (character === "(" || character === "[" || character === "{") {
      depth += 1;
      continue;
    }
    if (character === ")" || character === "]" || character === "}") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (character === ";" && depth === 0) {
      push(index + 1);
      start = index + 1;
      continue;
    }
    if (character === "\n" && depth === 0 && isLineCommand(script.slice(start, index))) {
      push(index);
      start = index + 1;
    }
  }
  push(script.length);
  return result;
}

export function mongoStatementAtCursor(
  script: string,
  position: number,
): MongoStatementRange | null {
  const statements = splitMongoStatements(script);
  if (!statements.length) return null;
  const exact = statements.find(
    (statement) => position >= statement.start && position <= statement.end,
  );
  if (exact) return exact;
  let previous = statements[0];
  for (const statement of statements) {
    if (statement.start > position) break;
    previous = statement;
  }
  return previous;
}

interface StoredDraft {
  version: 1;
  entries: Record<string, { text: string; updatedAt: number }>;
}

function readDrafts(): StoredDraft {
  if (typeof window === "undefined") return { version: 1, entries: {} };
  try {
    const raw = window.localStorage.getItem(MONGO_CONSOLE_DRAFT_KEY);
    if (!raw) return { version: 1, entries: {} };
    const parsed = JSON.parse(raw) as Partial<StoredDraft>;
    if (!parsed || parsed.version !== 1 || !parsed.entries) {
      throw new Error("invalid draft store");
    }
    const entries: StoredDraft["entries"] = {};
    for (const [key, value] of Object.entries(parsed.entries)) {
      if (
        !value ||
        typeof value.text !== "string" ||
        typeof value.updatedAt !== "number"
      ) {
        continue;
      }
      entries[key] = {
        text: value.text.slice(0, MONGO_CONSOLE_MAX_BYTES),
        updatedAt: value.updatedAt,
      };
    }
    return { version: 1, entries };
  } catch {
    return { version: 1, entries: {} };
  }
}

export function loadMongoConsoleDraft(connectionId: string): string {
  return readDrafts().entries[connectionId]?.text ?? "";
}

export function saveMongoConsoleDraft(
  connectionId: string,
  text: string,
): boolean {
  if (typeof window === "undefined") return false;
  try {
    const drafts = readDrafts();
    const entries = Object.entries(drafts.entries)
      .filter(([id]) => id !== connectionId)
      .sort(([, left], [, right]) => right.updatedAt - left.updatedAt)
      .slice(0, 49);
    const trimmed = text.slice(0, MONGO_CONSOLE_MAX_BYTES);
    if (trimmed.trim()) {
      entries.push([connectionId, { text: trimmed, updatedAt: Date.now() }]);
    }
    window.localStorage.setItem(
      MONGO_CONSOLE_DRAFT_KEY,
      JSON.stringify({
        version: 1,
        entries: Object.fromEntries(entries),
      }),
    );
    return true;
  } catch {
    return false;
  }
}

export function clearMongoConsoleDraft(connectionId: string): void {
  if (typeof window === "undefined") return;
  try {
    const drafts = readDrafts();
    delete drafts.entries[connectionId];
    window.localStorage.setItem(
      MONGO_CONSOLE_DRAFT_KEY,
      JSON.stringify(drafts),
    );
  } catch {
    // Draft persistence is best effort.
  }
}

export function mongoFieldUnion(
  documents: ReadonlyArray<Record<string, unknown>>,
): string[] {
  const seen = new Set<string>();
  const fields: string[] = [];
  for (const document of documents) {
    for (const key of Object.keys(document)) {
      if (seen.has(key)) continue;
      seen.add(key);
      fields.push(key);
    }
  }
  fields.sort((left, right) => {
    if (left === "_id") return -1;
    if (right === "_id") return 1;
    return left.localeCompare(right, "zh-CN");
  });
  return fields;
}

export function mongoCellText(value: unknown): string {
  if (value === undefined) return "";
  if (value === null) return "NULL";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function mongoResultJson(outcome: {
  documents: unknown[];
  values: unknown[];
}): string {
  if (outcome.documents.length) {
    return JSON.stringify(outcome.documents, null, 2);
  }
  return JSON.stringify(outcome.values, null, 2);
}
