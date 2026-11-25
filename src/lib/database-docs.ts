import type { DatabaseDocsSnapshot, DocsTable } from "./types";

export interface DocsGroup {
  id: string;
  name: string;
  color: string;
}

export interface DocsNote {
  tableNote: string;
  columns: Record<string, string>;
}

export interface DocsScope {
  notes: Record<string, DocsNote>;
  groups: DocsGroup[];
  tableGroups: Record<string, string>;
}

interface DocsFile {
  version: number;
  scopes: Record<string, DocsScope>;
}

export const DOCS_STORAGE_KEY = "mydb.databaseDocs.v1";
export const GROUP_COLORS = ["#58a6ff", "#3fb950", "#d29922", "#bc8cff", "#f85149", "#39c5cf"];

export function emptyScope(): DocsScope {
  return { notes: {}, groups: [], tableGroups: {} };
}

export function docsScopeKey(
  connectionId: string,
  database: string | null,
  schema: string | null,
): string {
  return [connectionId, database ?? "", schema ?? ""].join("|");
}

function readFile(): DocsFile {
  try {
    const raw = window.localStorage.getItem(DOCS_STORAGE_KEY);
    if (!raw) return { version: 1, scopes: {} };
    const parsed = JSON.parse(raw) as DocsFile;
    if (!parsed || parsed.version !== 1 || typeof parsed.scopes !== "object") {
      return { version: 1, scopes: {} };
    }
    return parsed;
  } catch {
    return { version: 1, scopes: {} };
  }
}

export function loadScope(scopeKey: string): DocsScope {
  const file = readFile();
  const scope = file.scopes[scopeKey];
  if (!scope) return emptyScope();
  return {
    notes: scope.notes ?? {},
    groups: Array.isArray(scope.groups) ? scope.groups : [],
    tableGroups: scope.tableGroups ?? {},
  };
}

export function saveScope(scopeKey: string, scope: DocsScope): void {
  const file = readFile();
  file.scopes[scopeKey] = scope;
  window.localStorage.setItem(DOCS_STORAGE_KEY, JSON.stringify(file));
}

export function noteFor(scope: DocsScope, table: string): DocsNote {
  return scope.notes[table] ?? { tableNote: "", columns: {} };
}

export function orphanTables(
  scope: DocsScope,
  snapshot: DatabaseDocsSnapshot | null,
): string[] {
  if (!snapshot) return Object.keys(scope.notes);
  const present = new Set(snapshot.tables.map((table) => table.name));
  return Object.keys(scope.notes).filter((name) => !present.has(name));
}

export function tablesInGroup(
  tables: DocsTable[],
  scope: DocsScope,
  groupId: string | null,
): DocsTable[] {
  return tables.filter((table) => {
    const assigned = scope.tableGroups[table.name] ?? null;
    return groupId === null ? assigned === null : assigned === groupId;
  });
}