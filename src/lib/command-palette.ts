export type PaletteIcon =
  | "database"
  | "table"
  | "view"
  | "code"
  | "diagram"
  | "plus"
  | "structure"
  | "refresh"
  | "history"
  | "sync"
  | "play"
  | "check"
  | "close"
  | "filter"
  | "target"
  | "download"
  | "upload"
  | "more";

export type PaletteKind =
  | "connection"
  | "object"
  | "tab"
  | "command";

export interface PaletteItem {
  id: string;
  kind: PaletteKind;
  label: string;
  detail: string;
  badge: string;
  icon: PaletteIcon;
  keywords?: string[];
  shortcut?: string;
  disabledReason?: string;
  run: () => void | Promise<void>;
  runAlt?: () => void | Promise<void>;
}

export interface PaletteRecentEntry {
  kind: "connection" | "command";
  id: string;
  usedAt: number;
}

export const PALETTE_RECENT_KEY = "mydb.commandPalette.recent.v1";
export const PALETTE_RECENT_LIMIT = 10;

interface StoredRecent {
  version?: number;
  items?: PaletteRecentEntry[];
}

export function loadPaletteRecent(): PaletteRecentEntry[] {
  try {
    const raw = window.localStorage.getItem(PALETTE_RECENT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as StoredRecent;
    if (!Array.isArray(parsed.items)) return [];
    return parsed.items
      .filter(
        (item): item is PaletteRecentEntry =>
          Boolean(item) &&
          (item.kind === "connection" || item.kind === "command") &&
          typeof item.id === "string" &&
          typeof item.usedAt === "number",
      )
      .slice(-PALETTE_RECENT_LIMIT);
  } catch {
    return [];
  }
}

export function savePaletteRecent(entries: PaletteRecentEntry[]) {
  try {
    window.localStorage.setItem(
      PALETTE_RECENT_KEY,
      JSON.stringify({
        version: 1,
        items: entries.slice(-PALETTE_RECENT_LIMIT),
      }),
    );
  } catch {
    // Recent items are a convenience; ignore storage failures.
  }
}

export function recordPaletteRecent(
  entries: PaletteRecentEntry[],
  kind: "connection" | "command",
  id: string,
): PaletteRecentEntry[] {
  const next = entries.filter(
    (entry) => !(entry.kind === kind && entry.id === id),
  );
  next.push({ kind, id, usedAt: Date.now() });
  return next.slice(-PALETTE_RECENT_LIMIT);
}

export function clearPaletteRecent() {
  try {
    window.localStorage.removeItem(PALETTE_RECENT_KEY);
  } catch {
    // Ignore storage failures.
  }
}

function tokensFor(query: string): string[] {
  return query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

function haystack(item: PaletteItem): string {
  return [
    item.label,
    item.detail,
    item.badge,
    ...(item.keywords ?? []),
  ]
    .join(" ")
    .toLowerCase();
}

/** Returns null when any token is missing; larger scores sort first. */
export function scorePaletteItem(
  item: PaletteItem,
  tokens: string[],
): number | null {
  if (!tokens.length) return 0;
  const text = haystack(item);
  if (!tokens.every((token) => text.includes(token))) return null;
  const label = item.label.toLowerCase();
  const detail = item.detail.toLowerCase();
  let score = 0;
  for (const token of tokens) {
    if (label === token) score += 100;
    else if (label.startsWith(token)) score += 60;
    else if (label.includes(token)) score += 40;
    else if (detail.includes(token)) score += 20;
    else score += 5;
  }
  return score;
}

export function filterPaletteItems(
  items: PaletteItem[],
  query: string,
): PaletteItem[] {
  const tokens = tokensFor(query);
  if (!tokens.length) return items;
  return items
    .map((item, index) => ({
      item,
      index,
      score: scorePaletteItem(item, tokens),
    }))
    .filter(
      (entry): entry is { item: PaletteItem; index: number; score: number } =>
        entry.score !== null,
    )
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .map((entry) => entry.item);
}