export type QueryRunMode = "statement" | "tab";

export const QUERY_RUN_MODE_KEY = "mydb.queryRunMode.v1";

export function loadQueryRunMode(): QueryRunMode {
  try {
    const value = window.localStorage.getItem(QUERY_RUN_MODE_KEY);
    return value === "tab" ? "tab" : "statement";
  } catch {
    return "statement";
  }
}

export function saveQueryRunMode(mode: QueryRunMode): void {
  try {
    window.localStorage.setItem(QUERY_RUN_MODE_KEY, mode);
  } catch {
    // Storage may be unavailable; the in-memory setting still applies.
  }
}

export function runModeLabel(mode: QueryRunMode): string {
  return mode === "tab" ? "整个标签" : "当前语句";
}