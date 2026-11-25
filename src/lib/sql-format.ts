export interface SqlFormatOptions {
  keywordCase: "upper" | "lower" | "preserve";
  tabWidth: 2 | 4;
  linesBetweenQueries: 0 | 1 | 2;
  preserveComments: boolean;
}

export const FORMAT_OPTIONS_KEY = "mydb.editor.format.v1";
export const SQL_FORMAT_SIZE_LIMIT = 100 * 1024;

export const DEFAULT_FORMAT_OPTIONS: SqlFormatOptions = {
  keywordCase: "upper",
  tabWidth: 2,
  linesBetweenQueries: 1,
  preserveComments: true,
};

export function loadFormatOptions(): SqlFormatOptions {
  try {
    const raw = window.localStorage.getItem(FORMAT_OPTIONS_KEY);
    if (!raw) return { ...DEFAULT_FORMAT_OPTIONS };
    const parsed = JSON.parse(raw) as Partial<SqlFormatOptions>;
    const keywordCase = ["upper", "lower", "preserve"].includes(
      String(parsed.keywordCase),
    )
      ? (parsed.keywordCase as SqlFormatOptions["keywordCase"])
      : DEFAULT_FORMAT_OPTIONS.keywordCase;
    const tabWidth = parsed.tabWidth === 4 ? 4 : 2;
    const linesBetweenQueries = [0, 1, 2].includes(
      Number(parsed.linesBetweenQueries),
    )
      ? (Number(parsed.linesBetweenQueries) as 0 | 1 | 2)
      : 1;
    return {
      keywordCase,
      tabWidth,
      linesBetweenQueries,
      preserveComments: parsed.preserveComments !== false,
    };
  } catch {
    return { ...DEFAULT_FORMAT_OPTIONS };
  }
}

export function saveFormatOptions(options: SqlFormatOptions): void {
  window.localStorage.setItem(FORMAT_OPTIONS_KEY, JSON.stringify(options));
}

export function formatGuard(text: string): string {
  const bytes = new TextEncoder().encode(text).length;
  if (bytes > SQL_FORMAT_SIZE_LIMIT) {
    return "SQL 文本超过 100 KB，请缩小选区后再格式化 / 压缩";
  }
  if (/\bDELIMITER\b/i.test(text)) {
    return "包含 DELIMITER 指令，请先拆分脚本或只处理过程体";
  }
  const head = text.replace(/^\s+/, "");
  if (
    /^CREATE\s+(OR\s+REPLACE\s+)?(DEFINER\s*=\s*\S+\s+)?(PROCEDURE|FUNCTION|TRIGGER|EVENT)\b/i.test(
      head,
    ) ||
    /^CREATE\s+(OR\s+REPLACE\s+)?(PROCEDURE|FUNCTION|TRIGGER)\b/i.test(head)
  ) {
    return "过程 / 函数 / 触发器定义暂不自动格式化";
  }
  return "";
}

type Mode =
  | "normal"
  | "single"
  | "double"
  | "backtick"
  | "bracket"
  | "line-comment"
  | "block-comment"
  | "dollar";

interface ScanState {
  mode: Mode;
  dollarTag: string;
}

function dollarTagAt(text: string, index: number): string | null {
  if (text[index] !== "$") return null;
  let cursor = index + 1;
  while (cursor < text.length && /[A-Za-z0-9_]/.test(text[cursor])) cursor += 1;
  if (text[cursor] !== "$") return null;
  return text.slice(index, cursor + 1);
}

/**
 * Lexical compaction. Strings, quoted identifiers, dollar quotes, MySQL
 * executable comments and optimizer hints are copied verbatim. Ordinary
 * comments are removed and runs of whitespace collapse to one space.
 */
export function compressSql(text: string): string {
  const state: ScanState = { mode: "normal", dollarTag: "" };
  let result = "";
  let pendingSpace = false;
  let index = 0;

  const push = (value: string) => {
    if (pendingSpace && result && !result.endsWith(" ") && !result.endsWith("\n")) {
      result += " ";
    }
    pendingSpace = false;
    result += value;
  };

  while (index < text.length) {
    const character = text[index];
    const next = text[index + 1];

    if (state.mode === "normal") {
      if (character === "'" || character === '"' || character === "`") {
        push(character);
        state.mode =
          character === "'" ? "single" : character === '"' ? "double" : "backtick";
        index += 1;
        continue;
      }
      if (character === "[") {
        push(character);
        state.mode = "bracket";
        index += 1;
        continue;
      }
      if (character === "-" && next === "-") {
        state.mode = "line-comment";
        index += 2;
        continue;
      }
      if (character === "#") {
        state.mode = "line-comment";
        index += 1;
        continue;
      }
      if (character === "/" && next === "*") {
        const executable = text[index + 2] === "!" || text[index + 2] === "+";
        if (executable) {
          const end = text.indexOf("*/", index + 2);
          const close = end < 0 ? text.length : end + 2;
          push(text.slice(index, close));
          index = close;
          continue;
        }
        state.mode = "block-comment";
        index += 2;
        continue;
      }
      if (character === "$") {
        const tag = dollarTagAt(text, index);
        if (tag) {
          push(tag);
          state.mode = "dollar";
          state.dollarTag = tag;
          index += tag.length;
          continue;
        }
      }
      if (/\s/.test(character)) {
        pendingSpace = true;
        index += 1;
        continue;
      }
      if (character === ";") {
        result = result.replace(/\s+$/, "");
        result += ";\n";
        pendingSpace = false;
        index += 1;
        while (index < text.length && /\s/.test(text[index])) index += 1;
        continue;
      }
      push(character);
      index += 1;
      continue;
    }

    if (state.mode === "line-comment") {
      if (character === "\n") {
        state.mode = "normal";
        pendingSpace = true;
      }
      index += 1;
      continue;
    }

    if (state.mode === "block-comment") {
      if (character === "*" && next === "/") {
        state.mode = "normal";
        pendingSpace = true;
        index += 2;
      } else {
        index += 1;
      }
      continue;
    }

    if (state.mode === "dollar") {
      if (text.startsWith(state.dollarTag, index)) {
        result += state.dollarTag;
        index += state.dollarTag.length;
        state.mode = "normal";
        state.dollarTag = "";
      } else {
        result += character;
        index += 1;
      }
      continue;
    }

    // Quoted string / identifier.
    result += character;
    if (character === "\\" && state.mode !== "backtick" && index + 1 < text.length) {
      result += text[index + 1];
      index += 2;
      continue;
    }
    const quote =
      state.mode === "single" ? "'" : state.mode === "double" ? '"' : state.mode === "bracket" ? "]" : "`";
    if (character === quote) {
      if (text[index + 1] === quote) {
        result += text[index + 1];
        index += 2;
        continue;
      }
      state.mode = "normal";
    }
    index += 1;
  }

  return result.trimEnd().replace(/\n{3,}/g, "\n\n");
}

/** Removes ordinary comments while keeping strings and executable hints. */
export function stripComments(text: string): string {
  let result = "";
  let index = 0;
  let mode: Mode = "normal";
  let dollarTag = "";
  while (index < text.length) {
    const character = text[index];
    const next = text[index + 1];
    if (mode === "normal") {
      if (character === "'" || character === '"' || character === "`") {
        mode =
          character === "'" ? "single" : character === '"' ? "double" : "backtick";
        result += character;
        index += 1;
        continue;
      }
      if (character === "[") {
        mode = "bracket";
        result += character;
        index += 1;
        continue;
      }
      if (character === "-" && next === "-") {
        mode = "line-comment";
        index += 2;
        continue;
      }
      if (character === "#") {
        mode = "line-comment";
        index += 1;
        continue;
      }
      if (character === "/" && next === "*") {
        if (text[index + 2] === "!" || text[index + 2] === "+") {
          const end = text.indexOf("*/", index + 2);
          const close = end < 0 ? text.length : end + 2;
          result += text.slice(index, close);
          index = close;
          continue;
        }
        mode = "block-comment";
        if (result && !/\s$/.test(result)) result += " ";
        index += 2;
        continue;
      }
      if (character === "$") {
        const tag = dollarTagAt(text, index);
        if (tag) {
          mode = "dollar";
          dollarTag = tag;
          result += tag;
          index += tag.length;
          continue;
        }
      }
      result += character;
      index += 1;
      continue;
    }
    if (mode === "line-comment") {
      if (character === "\n") {
        mode = "normal";
        result += "\n";
      }
      index += 1;
      continue;
    }
    if (mode === "block-comment") {
      if (character === "*" && next === "/") {
        mode = "normal";
        index += 2;
      } else {
        index += 1;
      }
      continue;
    }
    if (mode === "dollar") {
      if (text.startsWith(dollarTag, index)) {
        result += dollarTag;
        index += dollarTag.length;
        mode = "normal";
        dollarTag = "";
      } else {
        result += character;
        index += 1;
      }
      continue;
    }
    result += character;
    if (character === "\\" && mode !== "backtick" && index + 1 < text.length) {
      result += text[index + 1];
      index += 2;
      continue;
    }
    const quote =
      mode === "single" ? "'" : mode === "double" ? '"' : mode === "bracket" ? "]" : "`";
    if (character === quote) {
      if (text[index + 1] === quote) {
        result += text[index + 1];
        index += 2;
        continue;
      }
      mode = "normal";
    }
    index += 1;
  }
  return result;
}