export type ThemeMode = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

export const THEME_STORAGE_KEY = "mydb.theme.v1";
export const EDITOR_FONT_SIZE_KEY = "mydb.editor.fontSize.v1";

let media: MediaQueryList | null = null;
let mediaListener: ((event: MediaQueryListEvent) => void) | null = null;

export function loadThemeMode(): ThemeMode {
  try {
    const value = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (value === "light" || value === "dark" || value === "system") {
      return value;
    }
  } catch {
    // fall through to default
  }
  return "dark";
}

export function systemPrefersDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;
}

export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === "system") return systemPrefersDark() ? "dark" : "light";
  return mode;
}

function applyResolved(theme: ResolvedTheme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

export function applyTheme(mode: ThemeMode): ResolvedTheme {
  const resolved = resolveTheme(mode);
  applyResolved(resolved);
  if (mode === "system") {
    if (!media && window.matchMedia) {
      media = window.matchMedia("(prefers-color-scheme: dark)");
      mediaListener = (event) => applyResolved(event.matches ? "dark" : "light");
      media.addEventListener("change", mediaListener);
    }
  } else if (media && mediaListener) {
    media.removeEventListener("change", mediaListener);
    media = null;
    mediaListener = null;
  }
  return resolved;
}

export function saveThemeMode(mode: ThemeMode): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, mode);
  } catch {
    // Theme persistence is best-effort.
  }
}

export function loadEditorFontSize(): number {
  try {
    const value = Number(window.localStorage.getItem(EDITOR_FONT_SIZE_KEY));
    if (Number.isFinite(value) && value >= 11 && value <= 20) return value;
  } catch {
    // fall through
  }
  return 13;
}

export function applyEditorFontSize(size: number): void {
  const clamped = Math.max(11, Math.min(20, Math.round(size)));
  document.documentElement.style.setProperty(
    "--editor-font-size",
    `${clamped}px`,
  );
  try {
    window.localStorage.setItem(EDITOR_FONT_SIZE_KEY, String(clamped));
  } catch {
    // Persistence is best-effort.
  }
}

/** Called once before the app renders. */
export function initTheme(): { mode: ThemeMode; resolved: ResolvedTheme } {
  const mode = loadThemeMode();
  const resolved = applyTheme(mode);
  applyEditorFontSize(loadEditorFontSize());
  return { mode, resolved };
}