<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup } from "codemirror";
import { Compartment, EditorState, Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import {
  findNext,
  findPrevious,
  gotoLine,
  openSearchPanel,
} from "@codemirror/search";
import { toggleComment } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { MySQL, PostgreSQL, SQLite, sql } from "@codemirror/lang-sql";
import { tags } from "@lezer/highlight";
import { statementAtCursor, statementRangeAtCursor } from "../lib/sql-statements";
import {
  compressSql,
  formatGuard,
  loadFormatOptions,
  stripComments,
} from "../lib/sql-format";
import type { Driver, ScopeTableColumns } from "../lib/types";

const props = defineProps<{
  modelValue: string;
  tabId: string;
  driver: Driver;
  completionTables: ScopeTableColumns[];
}>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  selection: [value: string];
  run: [sql?: string];
  notice: [message: string, kind: "info" | "error"];
  busy: [value: boolean];
}>();

const host = ref<HTMLElement>();
let view: EditorView | undefined;
const language = new Compartment();

export type EditorCommand =
  | "search"
  | "find-next"
  | "find-previous"
  | "goto-line"
  | "toggle-comment"
  | "format-sql"
  | "compress-sql";

function sqlFormatterLanguage() {
  switch (props.driver) {
    case "mysql":
      return "mysql";
    case "postgres":
      return "postgresql";
    default:
      return "sqlite";
  }
}

function replaceTarget(
  from: number,
  to: number,
  text: string,
  select: boolean,
) {
  if (!view) return;
  view.dispatch({
    changes: { from, to, insert: text },
    selection: select
      ? { anchor: from, head: from + text.length }
      : { anchor: Math.min(from, view.state.doc.length) },
    scrollIntoView: true,
  });
  view.focus();
}

/** Selection, else the exact statement under the cursor, else the whole doc. */
function formatTarget(): { from: number; to: number; selected: boolean } | null {
  if (!view) return null;
  const { from, to } = view.state.selection.main;
  if (from !== to) return { from, to, selected: true };
  const statement = statementRangeAtCursor(view.state.doc.toString(), from);
  if (statement) {
    return { from: statement.start, to: statement.end, selected: false };
  }
  return { from: 0, to: view.state.doc.length, selected: false };
}

async function formatSql() {
  const target = formatTarget();
  if (!view || !target) return;
  const text = view.state.sliceDoc(target.from, target.to);
  if (!text.trim()) {
    emit("notice", "没有可格式化的 SQL", "error");
    return;
  }
  const guard = formatGuard(text);
  if (guard) {
    emit("notice", guard, "error");
    return;
  }

  emit("busy", true);
  try {
    const { format: formatSqlText } = await import("sql-formatter");
    const options = loadFormatOptions();
    const input = options.preserveComments ? text : stripComments(text);
    const formatted = formatSqlText(input, {
      language: sqlFormatterLanguage(),
      keywordCase: options.keywordCase,
      tabWidth: options.tabWidth,
      linesBetweenQueries: options.linesBetweenQueries,
    });
    if (!view) return;
    replaceTarget(target.from, target.to, formatted, target.selected);
    emit("notice", "已格式化 SQL", "info");
  } catch {
    emit("notice", "格式化失败，已保留原文", "error");
  } finally {
    emit("busy", false);
  }
}

function compressSelectedSql() {
  const target = formatTarget();
  if (!view || !target) return;
  const text = view.state.sliceDoc(target.from, target.to);
  if (!text.trim()) {
    emit("notice", "没有可压缩的 SQL", "error");
    return;
  }
  const guard = formatGuard(text);
  if (guard) {
    emit("notice", guard, "error");
    return;
  }
  const compressed = compressSql(text);
  replaceTarget(target.from, target.to, compressed, target.selected);
  emit("notice", "已压缩 SQL（普通注释已移除）", "info");
}

function runCommand(command: EditorCommand) {
  if (!view) return;
  view.focus();
  switch (command) {
    case "search":
      openSearchPanel(view);
      break;
    case "find-next":
      findNext(view);
      break;
    case "find-previous":
      findPrevious(view);
      break;
    case "goto-line":
      gotoLine(view);
      break;
    case "toggle-comment":
      toggleComment(view);
      break;
    case "format-sql":
      formatSql();
      break;
    case "compress-sql":
      compressSelectedSql();
      break;
  }
}

function insertSnippet(text: string) {
  if (!view) return;
  const { from, to } = view.state.selection.main;
  const marker = text.indexOf("${cursor}");
  const insert = marker >= 0 ? text.replace("${cursor}", "") : text;
  const cursor = marker >= 0 ? from + marker : from + insert.length;
  view.dispatch({
    changes: { from, to, insert },
    selection: { anchor: cursor },
  });
  view.focus();
}

defineExpose({
  runCommand,
  formatSql,
  insertSnippet,
  currentStatement(): string {
    if (!view) return "";
    const { from, to } = view.state.selection.main;
    if (from !== to) return view.state.sliceDoc(from, to);
    return statementAtCursor(view.state.doc.toString(), from);
  },
});

function dialectFor(driver: Driver) {
  switch (driver) {
    case "mysql":
      return MySQL;
    case "postgres":
      return PostgreSQL;
    default:
      return SQLite;
  }
}

function languageFor(driver: Driver, tables: ScopeTableColumns[]) {
  const schema: Record<string, string[]> = {};
  for (const table of tables) {
    schema[table.name] = table.columns.map((column) => column.name);
  }
  return sql({
    dialect: dialectFor(driver),
    schema,
    upperCaseKeywords: true,
  });
}

function selectedText(state: EditorState): string {
  const { from, to } = state.selection.main;
  return from === to ? "" : state.sliceDoc(from, to);
}

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "var(--editor-font-size, 13px)",
    backgroundColor: "var(--surface-well)",
    color: "var(--text)",
  },
  "&.cm-focused": {
    outline: "none",
    boxShadow: "inset 0 0 0 1px var(--accent-border)",
  },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    overflow: "auto",
  },
  ".cm-content": {
    padding: "10px 0",
  },
  ".cm-gutters": {
    backgroundColor: "var(--surface-well)",
    border: "none",
    color: "var(--disabled)",
  },
  ".cm-activeLine": {
    backgroundColor: "rgba(122, 167, 255, 0.08)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "rgba(122, 167, 255, 0.08)",
  },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
    backgroundColor: "rgba(122, 167, 255, 0.28) !important",
  },
});

const dbxHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "var(--syntax-keyword, #8ab4f8)" },
  {
    tag: [tags.string, tags.special(tags.string)],
    color: "var(--syntax-string, #81c995)",
  },
  {
    tag: [tags.number, tags.bool, tags.null],
    color: "var(--syntax-number, #fdd663)",
  },
  { tag: tags.comment, color: "var(--syntax-comment, #78787c)", fontStyle: "italic" },
  { tag: tags.operator, color: "var(--syntax-operator, #a3a3a7)" },
  { tag: tags.propertyName, color: "var(--syntax-property, #dcdce0)" },
  { tag: tags.typeName, color: "var(--syntax-type, #8ab4f8)" },
]);

onMounted(() => {
  if (!host.value) return;

  view = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        language.of(languageFor(props.driver, props.completionTables)),
        editorTheme,
        Prec.high(syntaxHighlighting(dbxHighlight)),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            emit("update:modelValue", update.state.doc.toString());
          }
          if (update.docChanged || update.selectionSet) {
            emit("selection", selectedText(update.state));
          }
        }),
        keymap.of([
          {
            key: "Mod-Enter",
            run: (target) => {
              const selection = selectedText(target.state);
              emit("run", selection || undefined);
              return true;
            },
          },
        ]),
      ],
    }),
    parent: host.value,
  });

  // The parent owns the per-tab selection so the toolbar Run button can use
  // it after the editor loses focus.
  emit("selection", "");
});

watch(
  () => props.tabId,
  () => {
    if (!view) return;
    view.dispatch({ selection: { anchor: 0 } });
    emit("selection", "");
  },
);

watch(
  () => props.modelValue,
  (value) => {
    if (!view) return;
    if (value !== view.state.doc.toString()) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
        selection: { anchor: 0 },
      });
      emit("selection", "");
    }
  },
);

watch(
  [() => props.driver, () => props.completionTables],
  ([driver, tables]) => {
    if (!view) return;
    view.dispatch({
      effects: language.reconfigure(languageFor(driver, tables)),
    });
  },
);

onBeforeUnmount(() => {
  view?.destroy();
  view = undefined;
});
</script>

<template>
  <div ref="host" class="sql-editor"></div>
</template>
