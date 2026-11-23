export interface WorkspaceLayout {
  sidebarWidth: number;
  objectsWidth: number;
  rightWidth: number;
  editorRatio: number;
  sidebarCollapsed: boolean;
  objectsCollapsed: boolean;
  resultCollapsed: boolean;
  rightOpen: boolean;
}

export const DEFAULT_LAYOUT: WorkspaceLayout = {
  sidebarWidth: 240,
  objectsWidth: 216,
  rightWidth: 320,
  editorRatio: 0.42,
  sidebarCollapsed: false,
  objectsCollapsed: false,
  resultCollapsed: false,
  rightOpen: false,
};

export const LAYOUT_STORAGE_KEY = "mydb.workspaceLayout.v1";

const LIMITS = {
  sidebarWidth: [180, 360] as const,
  objectsWidth: [160, 420] as const,
  rightWidth: [260, 520] as const,
  editorRatio: [0.2, 0.8] as const,
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function numberOr(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function booleanOr(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

export function normalizeLayout(raw: unknown): WorkspaceLayout {
  const source =
    raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    sidebarWidth: clamp(
      numberOr(source.sidebarWidth, DEFAULT_LAYOUT.sidebarWidth),
      ...LIMITS.sidebarWidth,
    ),
    objectsWidth: clamp(
      numberOr(source.objectsWidth, DEFAULT_LAYOUT.objectsWidth),
      ...LIMITS.objectsWidth,
    ),
    rightWidth: clamp(
      numberOr(source.rightWidth, DEFAULT_LAYOUT.rightWidth),
      ...LIMITS.rightWidth,
    ),
    editorRatio: clamp(
      numberOr(source.editorRatio, DEFAULT_LAYOUT.editorRatio),
      ...LIMITS.editorRatio,
    ),
    sidebarCollapsed: booleanOr(
      source.sidebarCollapsed,
      DEFAULT_LAYOUT.sidebarCollapsed,
    ),
    objectsCollapsed: booleanOr(
      source.objectsCollapsed,
      DEFAULT_LAYOUT.objectsCollapsed,
    ),
    resultCollapsed: booleanOr(
      source.resultCollapsed,
      DEFAULT_LAYOUT.resultCollapsed,
    ),
    rightOpen: booleanOr(source.rightOpen, DEFAULT_LAYOUT.rightOpen),
  };
}

export function loadWorkspaceLayout(): WorkspaceLayout {
  try {
    const raw = window.localStorage.getItem(LAYOUT_STORAGE_KEY);
    if (!raw) return { ...DEFAULT_LAYOUT };
    const parsed = JSON.parse(raw) as { version?: number } & Record<
      string,
      unknown
    >;
    if (parsed?.version !== 1) return { ...DEFAULT_LAYOUT };
    return normalizeLayout(parsed);
  } catch {
    return { ...DEFAULT_LAYOUT };
  }
}

export function saveWorkspaceLayout(layout: WorkspaceLayout): void {
  try {
    window.localStorage.setItem(
      LAYOUT_STORAGE_KEY,
      JSON.stringify({ version: 1, ...layout }),
    );
  } catch {
    // Local storage may be unavailable; layout still works in memory.
  }
}

export interface EffectiveLayoutWidths {
  sidebar: number;
  objects: number;
  right: number;
}

/**
 * Keeps the main workspace usable on narrow windows without rewriting the
 * saved widths. The right dock is an overlay below 1200px, so it is excluded.
 */
export function effectiveLayoutWidths(
  layout: WorkspaceLayout,
  viewportWidth: number,
): EffectiveLayoutWidths {
  let sidebar = clamp(layout.sidebarWidth, ...LIMITS.sidebarWidth);
  let objects = clamp(layout.objectsWidth, ...LIMITS.objectsWidth);
  let right =
    layout.rightOpen && viewportWidth >= 1200
      ? clamp(layout.rightWidth, ...LIMITS.rightWidth)
      : 0;

  const minMain = 360;
  const handles = 20;
  let excess = sidebar + objects + right + handles - (viewportWidth - minMain);

  if (excess > 0 && right > LIMITS.rightWidth[0]) {
    const take = Math.min(excess, right - LIMITS.rightWidth[0]);
    right -= take;
    excess -= take;
  }
  if (excess > 0) {
    const take = Math.min(excess, objects - LIMITS.objectsWidth[0]);
    objects -= take;
    excess -= take;
  }
  if (excess > 0) {
    const take = Math.min(excess, sidebar - LIMITS.sidebarWidth[0]);
    sidebar -= take;
  }

  return { sidebar, objects, right };
}