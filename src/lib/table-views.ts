import type { TableSort } from "./types";

export const TABLE_VIEWS_KEY = "mydb.tableViews.v1";
export const MAX_PROFILES_PER_TABLE = 20;
export const MAX_TABLE_VIEW_SCOPES = 200;
export const MAX_PROFILE_NAME_LENGTH = 40;
export const TABLE_VIEW_PAGE_SIZES = [50, 100, 200, 500];

export interface TableViewProfile {
  id: string;
  name: string;
  filter: string;
  sorts: TableSort[];
  limit: number;
  /// Keyed by `profileColumnKey` (column name, or `name#index` when duplicated).
  widths: Record<string, number>;
  hidden: string[];
  frozen: string[];
  /// Optional display order keyed by profileColumnKey. Missing = keep current.
  order?: string[];
  updatedAt: number;
}

export interface TableViewState {
  profiles: TableViewProfile[];
  activeId: string | null;
  defaultId: string | null;
}

interface StoredEntry extends TableViewState {
  key: string;
  updatedAt: number;
}

interface StoredViews {
  version: 1;
  entries: StoredEntry[];
}

export function tableViewScopeKey(
  driver: string,
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
  table: string,
  tableKind: string,
): string {
  return [
    driver,
    connectionId,
    database?.trim() ?? "",
    schema?.trim() ?? "",
    table,
    tableKind,
  ].join("\u0001");
}

/** Stable per-column key; duplicates get `name#index`. */
export function profileColumnKeys(
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

export function profileColumnKey(
  name: string,
  index: number,
  columns: ReadonlyArray<{ name: string }>,
): string {
  return profileColumnKeys(columns)[index] ?? name;
}

function uniqueStrings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of value) {
    if (typeof item !== "string" || !item || seen.has(item)) continue;
    seen.add(item);
    result.push(item);
  }
  return result;
}

function normalizeSorts(value: unknown): TableSort[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const result: TableSort[] = [];
  for (const item of value) {
    if (!item || typeof item !== "object") continue;
    const sort = item as Partial<TableSort>;
    if (typeof sort.column !== "string" || !sort.column) continue;
    if (sort.direction !== "asc" && sort.direction !== "desc") continue;
    if (seen.has(sort.column)) continue;
    seen.add(sort.column);
    result.push({ column: sort.column, direction: sort.direction });
  }
  return result;
}

function normalizeWidths(value: unknown): Record<string, number> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  const widths: Record<string, number> = {};
  for (const [key, raw] of Object.entries(value as Record<string, unknown>)) {
    if (!key || typeof raw !== "number" || !Number.isFinite(raw)) continue;
    widths[key] = Math.max(24, Math.min(2000, Math.round(raw)));
  }
  return widths;
}

function normalizeProfile(value: unknown): TableViewProfile | null {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<TableViewProfile>;
  if (typeof raw.id !== "string" || !raw.id) return null;
  if (typeof raw.name !== "string") return null;
  const name = raw.name.trim().slice(0, MAX_PROFILE_NAME_LENGTH);
  if (!name) return null;
  const limit =
    typeof raw.limit === "number" && TABLE_VIEW_PAGE_SIZES.includes(raw.limit)
      ? raw.limit
      : 100;
  return {
    id: raw.id,
    name,
    filter: typeof raw.filter === "string" ? raw.filter : "",
    sorts: normalizeSorts(raw.sorts),
    limit,
    widths: normalizeWidths(raw.widths),
    hidden: uniqueStrings(raw.hidden),
    frozen: uniqueStrings(raw.frozen),
    order:
      Array.isArray(raw.order) ? uniqueStrings(raw.order) : undefined,
    updatedAt:
      typeof raw.updatedAt === "number" && Number.isFinite(raw.updatedAt)
        ? raw.updatedAt
        : 0,
  };
}

function emptyStore(): StoredViews {
  return { version: 1, entries: [] };
}

function readStore(): StoredViews {
  if (typeof window === "undefined") return emptyStore();
  let raw: string | null = null;
  try {
    raw = window.localStorage.getItem(TABLE_VIEWS_KEY);
    if (!raw) return emptyStore();
    const parsed = JSON.parse(raw) as Partial<StoredViews>;
    if (!parsed || parsed.version !== 1 || !Array.isArray(parsed.entries)) {
      throw new Error("invalid table views store");
    }
    const entries: StoredEntry[] = [];
    for (const value of parsed.entries) {
      if (!value || typeof value !== "object") continue;
      const entry = value as Partial<StoredEntry>;
      if (typeof entry.key !== "string" || !entry.key) continue;
      const profiles = (Array.isArray(entry.profiles) ? entry.profiles : [])
        .map(normalizeProfile)
        .filter((profile): profile is TableViewProfile => Boolean(profile))
        .sort((left, right) => right.updatedAt - left.updatedAt)
        .slice(0, MAX_PROFILES_PER_TABLE);
      const ids = new Set(profiles.map((profile) => profile.id));
      entries.push({
        key: entry.key,
        profiles,
        activeId:
          typeof entry.activeId === "string" && ids.has(entry.activeId)
            ? entry.activeId
            : null,
        defaultId:
          typeof entry.defaultId === "string" && ids.has(entry.defaultId)
            ? entry.defaultId
            : null,
        updatedAt:
          typeof entry.updatedAt === "number" && Number.isFinite(entry.updatedAt)
            ? entry.updatedAt
            : 0,
      });
    }
    entries.sort((left, right) => right.updatedAt - left.updatedAt);
    return { version: 1, entries: entries.slice(0, MAX_TABLE_VIEW_SCOPES) };
  } catch {
    if (raw) {
      try {
        window.localStorage.removeItem(TABLE_VIEWS_KEY);
      } catch {
        // Best effort: a corrupt store must not block the table.
      }
    }
    return emptyStore();
  }
}

export function loadTableViewState(key: string): TableViewState {
  const entry = readStore().entries.find((item) => item.key === key);
  if (!entry) return { profiles: [], activeId: null, defaultId: null };
  return {
    profiles: entry.profiles.map((profile) => ({
      ...profile,
      sorts: profile.sorts.map((sort) => ({ ...sort })),
      widths: { ...profile.widths },
      hidden: [...profile.hidden],
      frozen: [...profile.frozen],
      ...(profile.order ? { order: [...profile.order] } : {}),
    })),
    activeId: entry.activeId,
    defaultId: entry.defaultId,
  };
}

/**
 * Persists one table's view state. Returns false when storage rejects the
 * write (quota / unavailable); the caller keeps its in-memory state.
 */
export function saveTableViewState(
  key: string,
  state: TableViewState,
): boolean {
  if (typeof window === "undefined") return false;
  const store = readStore();
  const profiles = [...state.profiles]
    .sort((left, right) => right.updatedAt - left.updatedAt)
    .slice(0, MAX_PROFILES_PER_TABLE);
  const ids = new Set(profiles.map((profile) => profile.id));
  const activeId =
    state.activeId && ids.has(state.activeId) ? state.activeId : null;
  const defaultId =
    state.defaultId && ids.has(state.defaultId) ? state.defaultId : null;
  const now = Date.now();
  const entries = store.entries.filter((entry) => entry.key !== key);
  if (profiles.length) {
    entries.push({
      key,
      profiles,
      activeId,
      defaultId,
      updatedAt: now,
    });
  }
  entries.sort((left, right) => right.updatedAt - left.updatedAt);
  try {
    window.localStorage.setItem(
      TABLE_VIEWS_KEY,
      JSON.stringify({
        version: 1,
        entries: entries.slice(0, MAX_TABLE_VIEW_SCOPES),
      }),
    );
    return true;
  } catch {
    return false;
  }
}

export function tableViewSignature(config: {
  filter: string;
  sorts: TableSort[];
  limit: number;
  widths: Record<string, number>;
  hidden: string[];
  frozen: string[];
  order?: string[];
}): string {
  const widths = Object.fromEntries(
    Object.entries(config.widths).sort(([left], [right]) =>
      left.localeCompare(right),
    ),
  );
  return JSON.stringify({
    filter: config.filter.trim(),
    sorts: config.sorts.map((sort) => [sort.column, sort.direction]),
    limit: config.limit,
    widths,
    hidden: [...config.hidden].sort(),
    frozen: [...config.frozen],
    order: config.order ?? [],
  });
}

export function createTableViewProfile(
  name: string,
  config: Omit<TableViewProfile, "id" | "name" | "updatedAt">,
): TableViewProfile {
  const id =
    globalThis.crypto?.randomUUID?.() ??
    `view-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return {
    id,
    name: name.trim().slice(0, MAX_PROFILE_NAME_LENGTH),
    filter: config.filter,
    sorts: config.sorts.map((sort) => ({ ...sort })),
    limit: config.limit,
    widths: { ...config.widths },
    hidden: [...config.hidden],
    frozen: [...config.frozen],
    order: config.order ? [...config.order] : undefined,
    updatedAt: Date.now(),
  };
}

/** Maps profile column keys to current column indexes, dropping missing ones. */
export function resolveProfileColumns(
  columns: ReadonlyArray<{ name: string }>,
  keys: readonly string[],
): number[] {
  const keyToIndex = new Map(
    profileColumnKeys(columns).map((key, index) => [key, index]),
  );
  const indices: number[] = [];
  for (const key of keys) {
    const index = keyToIndex.get(key);
    if (index !== undefined && !indices.includes(index)) indices.push(index);
  }
  return indices;
}
