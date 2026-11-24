import type { TableInfo } from "./types";

export interface ClosedQueryTab {
  kind: "query";
  tabId: string;
  title: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  sql: string;
  filePath: string | null;
  savedSql: string | null;
  hadTransaction: boolean;
}

export interface ClosedTableTab {
  kind: "table";
  tabId: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
  table: TableInfo;
  mode: "data" | "structure";
  focusedIndex: string | null;
}

export interface ClosedErTab {
  kind: "er";
  tabId: string;
  connectionId: string | null;
  database: string | null;
  schema: string | null;
}

export interface ClosedRedisTab {
  kind: "redis";
  tabId: string;
  connectionId: string;
}

export type ClosedTabSnapshot =
  | ClosedQueryTab
  | ClosedTableTab
  | ClosedErTab
  | ClosedRedisTab;

export const CLOSED_TAB_LIMIT = 10;

/** LIFO stack; the same tab id never appears twice. */
export function pushClosedTab(
  stack: ClosedTabSnapshot[],
  snapshot: ClosedTabSnapshot,
): ClosedTabSnapshot[] {
  const deduped = stack.filter((item) => item.tabId !== snapshot.tabId);
  return [...deduped, snapshot].slice(-CLOSED_TAB_LIMIT);
}

export function popClosedTab(
  stack: ClosedTabSnapshot[],
): { snapshot: ClosedTabSnapshot; rest: ClosedTabSnapshot[] } | null {
  const snapshot = stack[stack.length - 1];
  if (!snapshot) return null;
  return { snapshot, rest: stack.slice(0, -1) };
}