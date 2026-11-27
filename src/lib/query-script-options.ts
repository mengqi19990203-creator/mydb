const STORAGE_KEY = "mydb.queryScriptOptions.v1";

export interface QueryScriptOptions {
  continueOnError: boolean;
  wrapInTransaction: boolean;
}

const DEFAULT_OPTIONS: QueryScriptOptions = {
  continueOnError: false,
  wrapInTransaction: false,
};

export function loadQueryScriptOptions(): QueryScriptOptions {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_OPTIONS };
    const parsed = JSON.parse(raw) as Partial<QueryScriptOptions>;
    return {
      continueOnError: parsed.continueOnError === true,
      wrapInTransaction: parsed.wrapInTransaction === true,
    };
  } catch {
    return { ...DEFAULT_OPTIONS };
  }
}

export function saveQueryScriptOptions(options: QueryScriptOptions): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(options));
  } catch {
    // Keep the current session setting when persistence is unavailable.
  }
}
