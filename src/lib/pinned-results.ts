import type { QueryResult } from "./types";

export const MAX_PINNED_RESULTS = 10;
export const MAX_PINNED_BYTES = 10 * 1024 * 1024;
export const MAX_PINNED_TOTAL_BYTES = 50 * 1024 * 1024;
export const MAX_PINNED_NAME_LENGTH = 60;

export interface PinnedQueryResult {
  id: string;
  name: string;
  pinnedAt: number;
  sql: string;
  connectionId: string;
  database: string | null;
  schema: string | null;
  resultSetIndex: number;
  result: QueryResult;
  approxBytes: number;
}

export function cloneQueryResult(result: QueryResult): QueryResult | null {
  try {
    if (typeof structuredClone === "function") {
      return structuredClone(result);
    }
  } catch {
    // Fall through to JSON cloning.
  }
  try {
    return JSON.parse(JSON.stringify(result)) as QueryResult;
  } catch {
    return null;
  }
}

/** Rough UTF-16 payload estimate used before taking a full snapshot. */
export function estimateResultBytes(result: QueryResult): number {
  try {
    return JSON.stringify(result).length * 2;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

export function formatPinClock(timestamp: number): string {
  const date = new Date(timestamp);
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
}

export function defaultPinName(
  resultSetIndex: number,
  resultSetCount: number,
  timestamp = Date.now(),
): string {
  const prefix =
    resultSetCount > 1 ? `结果 ${resultSetIndex + 1}` : "结果";
  return `${prefix} · ${formatPinClock(timestamp)}`;
}

export function uniquePinName(
  name: string,
  pins: readonly PinnedQueryResult[],
  excludeId: string | null = null,
): string {
  const base = name.trim().slice(0, MAX_PINNED_NAME_LENGTH) || "固定结果";
  const names = new Set(
    pins
      .filter((pin) => pin.id !== excludeId)
      .map((pin) => pin.name.toLocaleLowerCase()),
  );
  if (!names.has(base.toLocaleLowerCase())) return base;
  let suffix = 2;
  while (
    names.has(`${base} (${suffix})`.toLocaleLowerCase()) ||
    `${base} (${suffix})`.length > MAX_PINNED_NAME_LENGTH
  ) {
    suffix += 1;
  }
  return `${base} (${suffix})`.slice(0, MAX_PINNED_NAME_LENGTH);
}

export function nextPinId(): string {
  return (
    globalThis.crypto?.randomUUID?.() ??
    `pin-${Date.now()}-${Math.random().toString(16).slice(2)}`
  );
}

export function pinnedResultBytes(pins: readonly PinnedQueryResult[]): number {
  return pins.reduce((total, pin) => total + pin.approxBytes, 0);
}
