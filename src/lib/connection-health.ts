export const AUTO_RECONNECT_KEY = "mydb.autoReconnect.v1";
export const HEALTH_CHECK_INTERVAL_MS = 60_000;

const EXCLUDED_PATTERNS = [
  "查询超时",
  "已取消",
  "cancelled",
  "canceled",
  "syntax error",
  "语法错误",
  "permission denied",
  "access denied",
  "authentication failed",
  "认证失败",
  "does not exist",
  "不存在",
  "unknown column",
  "no such table",
  "no such column",
  "constraint",
  "约束",
  "duplicate entry",
  "只读",
  "read-only",
  "readonly",
];

const LOST_PATTERNS = [
  "mysql server has gone away",
  "lost connection to mysql server",
  "server closed the connection unexpectedly",
  "connection reset by peer",
  "connection reset",
  "connection aborted",
  "broken pipe",
  "unexpected eof",
  "ssl connection has been closed",
  "terminating connection due to administrator command",
  "terminating connection because of crash",
  "connection is closed",
  "connection already closed",
  "no connection to the server",
  "client has encountered a connection error",
  "connection timed out",
  "network is unreachable",
  "connection refused",
  "server selection timeout",
  "mongo network error",
  "topology is closed",
];

/** Conservative classifier: unknown errors remain ordinary SQL errors. */
export function isConnectionLostError(message: string): boolean {
  const text = message.toLocaleLowerCase();
  if (!text.trim()) return false;
  if (
    EXCLUDED_PATTERNS.some((pattern) => text.includes(pattern.toLocaleLowerCase()))
  ) {
    return false;
  }
  return LOST_PATTERNS.some((pattern) => text.includes(pattern));
}

export function loadAutoReconnect(): boolean {
  try {
    const value = window.localStorage.getItem(AUTO_RECONNECT_KEY);
    if (value === null) return true;
    return value !== "0";
  } catch {
    return true;
  }
}

export function saveAutoReconnect(enabled: boolean): void {
  try {
    window.localStorage.setItem(AUTO_RECONNECT_KEY, enabled ? "1" : "0");
  } catch {
    // The setting remains active for this run even when storage is unavailable.
  }
}

export function summarizeConnectionError(message: string): string {
  const singleLine = message.replace(/\s+/g, " ").trim();
  return singleLine.length > 180
    ? `${singleLine.slice(0, 177)}…`
    : singleLine;
}
