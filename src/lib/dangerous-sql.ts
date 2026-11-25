const SETTINGS_KEY = "mydb.safety.v1";

interface SafetySettings {
  confirmDangerousSql: boolean;
}

export interface DangerFinding {
  kind: "missing-where" | "drop" | "truncate" | "drop-column";
  statement: string;
  message: string;
  highRisk: boolean;
}

export function loadDangerConfirm(): boolean {
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY);
    if (!raw) return true;
    const parsed = JSON.parse(raw) as Partial<SafetySettings>;
    return parsed.confirmDangerousSql !== false;
  } catch {
    return true;
  }
}

export function saveDangerConfirm(value: boolean): void {
  const payload: SafetySettings = { confirmDangerousSql: value };
  window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(payload));
}

/** Removes quoted strings and comments so keyword heuristics do not match data. */
function sanitize(sql: string): string {
  let result = "";
  let index = 0;
  let mode: "normal" | "single" | "double" | "backtick" | "line" | "block" =
    "normal";
  while (index < sql.length) {
    const character = sql[index];
    const next = sql[index + 1];
    if (mode === "normal") {
      if (character === "'" || character === '"' || character === "`") {
        mode =
          character === "'"
            ? "single"
            : character === '"'
              ? "double"
              : "backtick";
        result += " ";
        index += 1;
        continue;
      }
      if (character === "-" && next === "-") {
        mode = "line";
        index += 2;
        continue;
      }
      if (character === "#") {
        mode = "line";
        index += 1;
        continue;
      }
      if (character === "/" && next === "*") {
        mode = "block";
        index += 2;
        continue;
      }
      result += character;
      index += 1;
      continue;
    }
    if (mode === "line") {
      if (character === "\n") {
        mode = "normal";
        result += "\n";
      }
      index += 1;
      continue;
    }
    if (mode === "block") {
      if (character === "*" && next === "/") {
        mode = "normal";
        index += 2;
      } else {
        index += 1;
      }
      continue;
    }
    const quote = mode === "single" ? "'" : mode === "double" ? '"' : "`";
    if (character === "\\" && index + 1 < sql.length) {
      index += 2;
      continue;
    }
    if (character === quote) {
      if (sql[index + 1] === quote) {
        index += 2;
        continue;
      }
      mode = "normal";
    }
    index += 1;
  }
  return result;
}

function statements(sql: string): string[] {
  const clean = sanitize(sql);
  return clean
    .split(";")
    .map((statement) => statement.trim())
    .filter(Boolean);
}

export function findDangerousSql(sql: string): DangerFinding | null {
  for (const statement of statements(sql)) {
    const upper = statement.replace(/\s+/g, " ").trim().toUpperCase();
    if (/^(UPDATE|DELETE)\b/.test(upper) && !/\bWHERE\b/.test(upper)) {
      return {
        kind: "missing-where",
        statement,
        message: `${upper.startsWith("DELETE") ? "DELETE" : "UPDATE"} 没有 WHERE 条件，可能影响整张表。`,
        highRisk: true,
      };
    }
    if (/^DROP\s+(TABLE|DATABASE|SCHEMA)\b/.test(upper)) {
      return {
        kind: "drop",
        statement,
        message: "DROP 会永久删除对象，且本工具不会执行 CASCADE。",
        highRisk: true,
      };
    }
    if (/^TRUNCATE\b/.test(upper)) {
      return {
        kind: "truncate",
        statement,
        message: "TRUNCATE 会清空整张表且通常不可回滚。",
        highRisk: true,
      };
    }
    if (
      /^ALTER\s+TABLE\b/.test(upper) &&
      /\bDROP\s+(COLUMN|CONSTRAINT)\b/.test(upper)
    ) {
      return {
        kind: "drop-column",
        statement,
        message: "ALTER TABLE ... DROP 会删除字段或约束，可能导致数据丢失。",
        highRisk: true,
      };
    }
  }
  return null;
}