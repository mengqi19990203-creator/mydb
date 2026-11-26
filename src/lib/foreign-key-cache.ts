import * as api from "./api";
import type { ScopeForeignKey } from "./types";

const TTL_MS = 60_000;
const MAX_SCOPES = 20;

interface CacheEntry {
  expiresAt: number;
  items: ScopeForeignKey[];
}

const cache = new Map<string, CacheEntry>();
const inflight = new Map<string, Promise<ScopeForeignKey[]>>();

function keyFor(
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
): string {
  return [
    connectionId,
    database?.trim() ?? "",
    schema?.trim() ?? "",
  ].join("\u0001");
}

export function peekScopeForeignKeys(
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
): ScopeForeignKey[] | null {
  const key = keyFor(connectionId, database, schema);
  const entry = cache.get(key);
  if (!entry) return null;
  if (entry.expiresAt <= Date.now()) {
    cache.delete(key);
    return null;
  }
  return entry.items;
}

export async function loadScopeForeignKeys(
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
  force = false,
): Promise<ScopeForeignKey[]> {
  const key = keyFor(connectionId, database, schema);
  if (!force) {
    const cached = peekScopeForeignKeys(connectionId, database, schema);
    if (cached) return cached;
  }
  const pending = inflight.get(key);
  if (pending) return pending;
  const request = api
    .listScopeForeignKeys(connectionId)
    .then((items) => {
      cache.set(key, { expiresAt: Date.now() + TTL_MS, items });
      while (cache.size > MAX_SCOPES) {
        const oldest = cache.keys().next().value;
        if (oldest === undefined) break;
        cache.delete(oldest);
      }
      return items;
    })
    .finally(() => {
      inflight.delete(key);
    });
  inflight.set(key, request);
  return request;
}

export function invalidateScopeForeignKeys(connectionId?: string): void {
  if (!connectionId) {
    cache.clear();
    return;
  }
  for (const key of [...cache.keys()]) {
    if (key.startsWith(`${connectionId}\u0001`)) cache.delete(key);
  }
}
