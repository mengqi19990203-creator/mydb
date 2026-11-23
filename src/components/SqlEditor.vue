<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup } from "codemirror";
import { EditorState, Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { SQLite, sql } from "@codemirror/lang-sql";
import { tags } from "@lezer/highlight";

const props = defineProps<{ modelValue: string; tabId: string }>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  selection: [value: string];
  run: [sql?: string];
}>();

const host = ref<HTMLElement>();
let view: EditorView | undefined;

function selectedText(state: EditorState): string {
  const { from, to } = state.selection.main;
  return from === to ? "" : state.sliceDoc(from, to);
}

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "13px",
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
  { tag: tags.keyword, color: "#8ab4f8" },
  { tag: [tags.string, tags.special(tags.string)], color: "#81c995" },
  { tag: [tags.number, tags.bool, tags.null], color: "#fdd663" },
  { tag: tags.comment, color: "#78787c", fontStyle: "italic" },
  { tag: tags.operator, color: "#a3a3a7" },
  { tag: tags.propertyName, color: "#dcdce0" },
  { tag: tags.typeName, color: "#8ab4f8" },
]);

onMounted(() => {
  if (!host.value) return;

  view = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        sql({ dialect: SQLite }),
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

onBeforeUnmount(() => {
  view?.destroy();
  view = undefined;
});
</script>

<template>
  <div ref="host" class="sql-editor"></div>
</template>
