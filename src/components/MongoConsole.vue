<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { basicSetup } from "codemirror";
import {
  autocompletion,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { javascript } from "@codemirror/lang-javascript";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Prec } from "@codemirror/state";
import { tags } from "@lezer/highlight";
import { EditorView, keymap } from "@codemirror/view";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import {
  MONGO_CONSOLE_MAX_BYTES,
  loadMongoConsoleDraft,
  mongoCellText,
  mongoFieldUnion,
  mongoResultJson,
  mongoStatementAtCursor,
  saveMongoConsoleDraft,
  splitMongoStatements,
} from "../lib/mongo-console";
import {
  addMongoConsoleHistory,
  clearMongoConsoleHistory,
  listMongoConsoleHistory,
  type MongoConsoleHistoryEntry,
} from "../lib/mongo-console-history";
import type {
  MongoCollectionInfo,
  MongoConsoleConfirmation,
  MongoConsoleOutcome,
} from "../lib/types";

const props = defineProps<{
  connectionId: string;
  database: string;
  databases: string[];
  collections: MongoCollectionInfo[];
  fields: string[];
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "database-change": [database: string];
  "refresh-collections": [];
  notice: [message: string, kind: "info" | "error"];
  ready: [];
}>();

const draft = ref(loadMongoConsoleDraft(props.connectionId));
const running = ref(false);
const outcomes = ref<MongoConsoleOutcome[]>([]);
const activeOutcomeIndex = ref(0);
const error = ref("");
const confirmations = ref<MongoConsoleConfirmation[]>([]);
const pendingScript = ref("");
const confirmChecked = ref(false);
const runMode = ref<"statement" | "all">("statement");
const historyOpen = ref(false);
const history = ref<MongoConsoleHistoryEntry[]>([]);
const scopeIndex = ref(0);
const scopeTotal = ref(0);
const selectedText = ref("");
const editorHost = ref<HTMLElement | null>(null);
let view: EditorView | null = null;
let draftTimer = 0;
let historyTimer = 0;

const examples = [
  {
    label: "查找文档",
    text: 'db.users.find({"status":"active"}).limit(20)',
  },
  {
    label: "筛选 + 排序",
    text: 'db.users.find({}).sort({"createdAt":-1}).skip(0).limit(20)',
  },
  {
    label: "精确计数",
    text: 'db.users.countDocuments({"status":"active"})',
  },
  {
    label: "Distinct",
    text: 'db.users.distinct("city")',
  },
  {
    label: "聚合分组",
    text:
      'db.orders.aggregate([\n' +
      '  {"$match": {"status": "paid"}},\n' +
      '  {"$group": {"_id": "$user_id", "total": {"$sum": "$amount"}}},\n' +
      '  {"$sort": {"total": -1}}\n' +
      '])',
  },
  {
    label: "插入文档",
    text: 'db.users.insertOne({"name":"测试用户","age":30})',
  },
  {
    label: "更新文档",
    text:
      'db.users.updateOne({"_id":{"$oid":"507f1f77bcf86cd799439011"}},' +
      '{"$set":{"age":31}})',
  },
  {
    label: "删除文档",
    text: 'db.users.deleteOne({"name":"测试用户"})',
  },
];

const currentOutcome = computed(
  () => outcomes.value[activeOutcomeIndex.value] ?? null,
);

const resultFields = computed(() => {
  const outcome = currentOutcome.value;
  if (!outcome?.documents.length) return [];
  return mongoFieldUnion(outcome.documents);
});

const resultRows = computed(() => currentOutcome.value?.documents ?? []);
const currentStatement = computed(() => {
  if (!view) return "";
  const position = view.state.selection.main.head;
  return mongoStatementAtCursor(draft.value, position)?.text ?? "";
});

const canRun = computed(
  () => draft.value.length <= MONGO_CONSOLE_MAX_BYTES && !running.value,
);
const scopeLabel = computed(() =>
  scopeTotal.value > 1 ? `第 ${scopeIndex.value} / ${scopeTotal.value} 条` : "",
);
const currentCollectionName = computed(() => {
  const match = currentStatement.value.match(
    /^db\.(?:getCollection\("([^"]+)"\)|([A-Za-z0-9_-]+))\./,
  );
  return match?.[1] ?? match?.[2] ?? "";
});

function flash(message: string, kind: "info" | "error" = "info") {
  emit("notice", message, kind);
}

function saveDraftSoon() {
  window.clearTimeout(draftTimer);
  draftTimer = window.setTimeout(() => {
    saveMongoConsoleDraft(props.connectionId, draft.value);
  }, 250);
}

function updateScope(state = view?.state) {
  if (!state) {
    scopeIndex.value = 0;
    scopeTotal.value = 0;
    selectedText.value = "";
    return;
  }
  const selection = state.selection.main;
  selectedText.value =
    selection.from === selection.to
      ? ""
      : state.sliceDoc(selection.from, selection.to);
  const statements = splitMongoStatements(state.doc.toString());
  scopeTotal.value = statements.length;
  if (selection.from !== selection.to || !statements.length) {
    scopeIndex.value = 0;
    return;
  }
  const position = selection.head;
  let index = statements.findIndex(
    (statement) => position >= statement.start && position <= statement.end,
  );
  if (index < 0) index = 0;
  scopeIndex.value = index + 1;
}

function completionSource(context: CompletionContext): CompletionResult | null {
  const before = context.state.sliceDoc(0, context.pos);
  const word = context.matchBefore(/[\w$-]*/);
  if (!word) return null;
  let options: Completion[] = [];
  let from = word.from;
  if (/show\s+\w*$/i.test(before)) {
    options = [
      { label: "dbs", type: "keyword", detail: "数据库列表" },
      { label: "databases", type: "keyword", detail: "数据库列表" },
      { label: "collections", type: "keyword", detail: "当前数据库集合" },
    ];
  } else if (/use\s+[\w-]*$/i.test(before)) {
    options = props.databases.map((name) => ({
      label: name,
      type: "namespace",
      detail: "数据库",
    }));
  } else if (/db\.getCollection\("[^"]*$/i.test(before)) {
    options = props.collections.map((item) => ({
      label: item.name,
      type: "class",
      detail: item.kind,
    }));
  } else if (/db\.([\w-]+)\.\w*$/.test(before)) {
    options = [
      "find",
      "findOne",
      "countDocuments",
      "estimatedDocumentCount",
      "distinct",
      "aggregate",
      "insertOne",
      "insertMany",
      "updateOne",
      "updateMany",
      "replaceOne",
      "deleteOne",
      "deleteMany",
      "findOneAndUpdate",
      "findOneAndReplace",
      "findOneAndDelete",
    ].map((method) => ({
      label: method,
      type: "method",
      detail: "MongoDB 方法",
    }));
  } else if (/db\.[\w-]*$/.test(before)) {
    options = props.collections.map((item) => ({
      label: item.name,
      type: "class",
      detail: item.kind,
    }));
  } else if (/\$[\w]*$/.test(before)) {
    options = [
      "$eq",
      "$gt",
      "$gte",
      "$lt",
      "$lte",
      "$in",
      "$nin",
      "$ne",
      "$exists",
      "$regex",
      "$and",
      "$or",
      "$set",
      "$unset",
      "$inc",
      "$push",
      "$pull",
      "$addToSet",
      "$rename",
      "$match",
      "$group",
      "$project",
      "$sort",
      "$limit",
      "$skip",
      "$unwind",
      "$lookup",
      "$count",
    ].map((operator) => ({
      label: operator,
      type: "keyword",
      detail: "MongoDB 操作符",
    }));
  } else if (props.fields.length) {
    options = props.fields.map((field) => ({
      label: field,
      type: "property",
      detail: currentCollectionName.value || "已加载字段",
    }));
  }
  if (!options.length) return null;
  return {
    from,
    options: options.slice(0, 200),
    validFor: /^[\w$-]*$/,
  };
}

const consoleTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "11px",
    backgroundColor: "var(--surface-well)",
    color: "var(--text)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    overflow: "auto",
    fontFamily: "var(--font-mono)",
  },
  ".cm-content": { padding: "8px 0" },
  ".cm-gutters": {
    backgroundColor: "var(--surface-well)",
    border: "none",
    color: "var(--disabled)",
  },
  ".cm-activeLine": { backgroundColor: "rgba(122, 167, 255, 0.08)" },
  ".cm-activeLineGutter": { backgroundColor: "rgba(122, 167, 255, 0.08)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
    backgroundColor: "rgba(122, 167, 255, 0.28) !important",
  },
});

const consoleHighlight = HighlightStyle.define([
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

function createEditor() {
  if (!editorHost.value) return;
  view = new EditorView({
    doc: draft.value,
    parent: editorHost.value,
    extensions: [
      basicSetup,
      javascript(),
      autocompletion({ override: [completionSource] }),
      Prec.highest(
        keymap.of([
          {
            key: "Mod-Enter",
            run: () => {
              void run();
              return true;
            },
          },
          {
            key: "Ctrl-Space",
            run: (target) => {
              startCompletion(target);
              return true;
            },
          },
        ]),
      ),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged && !update.selectionSet) return;
        draft.value = update.state.doc.toString();
        updateScope(update.state);
        if (update.docChanged) saveDraftSoon();
      }),
      consoleTheme,
      Prec.high(syntaxHighlighting(consoleHighlight)),
      EditorView.lineWrapping,
    ],
  });
  updateScope(view.state);
  view.focus();
  emit("ready");
}

function insertExample(text: string) {
  if (!view) return;
  const selection = view.state.selection.main;
  view.dispatch({
    changes: {
      from: selection.from,
      to: selection.to,
      insert: text,
    },
    selection: { anchor: selection.from + text.length },
  });
  view.focus();
}

function insertHistory(entry: MongoConsoleHistoryEntry) {
  historyOpen.value = false;
  if (!view) return;
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: entry.command },
    selection: { anchor: entry.command.length },
  });
  view.focus();
}

function clearHistory() {
  clearMongoConsoleHistory();
  history.value = [];
}

function clearResults() {
  outcomes.value = [];
  activeOutcomeIndex.value = 0;
  error.value = "";
  confirmations.value = [];
}

function outcomeLabel(outcome: MongoConsoleOutcome): string {
  if (outcome.kind === "documents") return `${outcome.documents.length} 个文档`;
  if (outcome.kind === "document") return "单个文档";
  if (outcome.kind === "scalar") return "标量";
  if (outcome.kind === "values") return `${outcome.values.length} 个值`;
  if (outcome.kind === "write") return "写入摘要";
  return outcome.message ?? "完成";
}

function outcomeSummary(outcome: MongoConsoleOutcome): string {
  return outcome.statement.replace(/\s+/g, " ").slice(0, 80);
}

function applyResponse(
  response: Awaited<ReturnType<typeof api.mongoConsoleExecute>>,
  script: string,
) {
  if (response.confirmations.length) {
    confirmations.value = response.confirmations;
    pendingScript.value = script;
    confirmChecked.value = false;
    return;
  }
  outcomes.value = response.outcomes;
  activeOutcomeIndex.value = response.error ? Math.max(0, response.outcomes.length - 1) : 0;
  error.value = response.error ?? "";
  confirmations.value = [];
  pendingScript.value = "";
  if (response.database && response.database !== props.database) {
    emit("database-change", response.database);
  }
  if (response.refreshCollections) {
    emit("refresh-collections");
  }
  addMongoConsoleHistory({
    command: script,
    database: response.database,
    ok: !response.error,
    error: response.error ?? undefined,
  });
  history.value = listMongoConsoleHistory();
  if (response.error) {
    flash(response.error, "error");
  } else {
    const writes = response.outcomes
      .filter((outcome) => outcome.kind === "write" && outcome.message)
      .map((outcome) => outcome.message);
    flash(
      writes.length
        ? writes.join("；")
        : `执行完成：${response.outcomes.length} 条结果`,
    );
  }
}

async function runWith(script: string, confirmed: boolean) {
  if (running.value) return;
  if (!script.trim()) {
    flash("没有可执行的命令", "error");
    return;
  }
  if (new TextEncoder().encode(script).length > MONGO_CONSOLE_MAX_BYTES) {
    flash("命令超过 100 KB 上限", "error");
    return;
  }
  running.value = true;
  error.value = "";
  try {
    const response = await api.mongoConsoleExecute(
      props.connectionId,
      props.database,
      script,
      confirmed,
    );
    applyResponse(response, script);
  } catch (reason) {
    error.value = String(reason);
    flash(String(reason), "error");
    addMongoConsoleHistory({
      command: script,
      database: props.database,
      ok: false,
      error: String(reason),
    });
    history.value = listMongoConsoleHistory();
  } finally {
    running.value = false;
  }
}

async function run() {
  if (!view) return;
  const selection = view.state.selection.main;
  let script: string;
  if (selection.from !== selection.to) {
    script = view.state.sliceDoc(selection.from, selection.to);
  } else if (runMode.value === "all") {
    script = draft.value;
  } else {
    script =
      mongoStatementAtCursor(draft.value, selection.head)?.text ?? draft.value;
  }
  await runWith(script, false);
}

async function confirmRun() {
  if (!confirmChecked.value || !pendingScript.value) return;
  const script = pendingScript.value;
  confirmations.value = [];
  pendingScript.value = "";
  confirmChecked.value = false;
  await runWith(script, true);
}

async function exportResult() {
  const outcome = currentOutcome.value;
  if (!outcome) return;
  const path = await saveDialog({
    defaultPath: `mongo-result-${Date.now()}.json`,
    filters: [
      { name: "JSON", extensions: ["json"] },
      { name: "CSV", extensions: ["csv"] },
    ],
  });
  if (typeof path !== "string") return;
  const isCsv = path.toLocaleLowerCase().endsWith(".csv");
  let contents: string;
  if (isCsv) {
    const escape = (value: string) =>
      /[",\r\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
    if (outcome.values.length) {
      contents = ["values", ...outcome.values.map((value) => escape(mongoCellText(value)))].join("\r\n");
    } else {
      const fields = mongoFieldUnion(outcome.documents);
      const lines = [["#", ...fields].map(escape).join(",")];
      outcome.documents.forEach((document, index) => {
        lines.push(
          [String(index + 1), ...fields.map((field) => mongoCellText(document[field]))]
            .map(escape)
            .join(","),
        );
      });
      contents = lines.join("\r\n");
    }
  } else {
    contents = mongoResultJson({
      documents: outcome.documents,
      values: outcome.values,
    });
  }
  try {
    await api.writeTextFile(path, contents);
    flash(`已导出到 ${path}`);
  } catch (reason) {
    flash(`导出失败：${reason}`, "error");
  }
}

async function copyJson() {
  if (!currentOutcome.value) return;
  try {
    await writeClipboard(
      mongoResultJson({
        documents: currentOutcome.value.documents,
        values: currentOutcome.value.values,
      }),
    );
    flash("已复制结果 JSON");
  } catch (reason) {
    flash(`复制失败：${reason}`, "error");
  }
}

async function copyTsv() {
  const outcome = currentOutcome.value;
  if (!outcome) return;
  let text = "";
  if (outcome.values.length) {
    text = ["values", ...outcome.values.map((value) => mongoCellText(value))].join(
      "\n",
    );
  } else if (outcome.documents.length) {
    const fields = mongoFieldUnion(outcome.documents);
    const header = ["#", ...fields];
    const lines = [header.join("\t")];
    outcome.documents.forEach((document, index) => {
      lines.push(
        [
          index + 1,
          ...fields.map((field) => mongoCellText(document[field])),
        ].join("\t"),
      );
    });
    text = lines.join("\n");
  }
  if (!text) {
    flash("当前结果为空", "error");
    return;
  }
  try {
    await writeClipboard(text);
    flash("已复制结果 TSV");
  } catch (reason) {
    flash(`复制失败：${reason}`, "error");
  }
}

watch(
  () => props.connectionId,
  () => {
    draft.value = loadMongoConsoleDraft(props.connectionId);
    clearResults();
    if (view) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: draft.value },
      });
      updateScope(view.state);
    }
  },
);

watch(
  () => props.collections,
  () => {
    // Completion reads props directly; this watch only forces a Vue update.
  },
  { deep: true },
);

watch(historyOpen, (open) => {
  if (open) history.value = listMongoConsoleHistory();
});

onMounted(() => {
  if (!draft.value.trim()) {
    draft.value = examples[0].text;
    saveMongoConsoleDraft(props.connectionId, draft.value);
  }
  history.value = listMongoConsoleHistory();
  void nextTick(() => createEditor());
  document.addEventListener("click", closeHistoryOnOutside);
});

onBeforeUnmount(() => {
  window.clearTimeout(draftTimer);
  window.clearTimeout(historyTimer);
  document.removeEventListener("click", closeHistoryOnOutside);
  view?.destroy();
});

function closeHistoryOnOutside(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest(".mongo-console-history-wrap")) return;
  historyOpen.value = false;
}

function insertScript(text: string): boolean {
  if (!view) return false;
  const selection = view.state.selection.main;
  view.dispatch({
    changes: { from: selection.from, to: selection.to, insert: text },
    selection: { anchor: selection.from + text.length },
    scrollIntoView: true,
  });
  view.focus();
  return true;
}

function replaceScript(text: string): boolean {
  if (!view) return false;
  const selection = view.state.selection.main;
  const from = selection.empty ? 0 : selection.from;
  const to = selection.empty ? view.state.doc.length : selection.to;
  view.dispatch({
    changes: { from, to, insert: text },
    selection: { anchor: from + text.length },
    scrollIntoView: true,
  });
  view.focus();
  return true;
}

defineExpose({
  currentScript: () => draft.value,
  insertScript,
  replaceScript,
});
</script>

<template>
  <div class="mongo-console">
    <header class="mongo-console-toolbar">
      <div class="mongo-console-mode">
        <button
          class="button primary compact"
          :disabled="!canRun"
          title="运行（⌘/Ctrl+Enter）"
          @click="run"
        >
          <Icon name="play" :size="13" />
          {{ running ? "运行中…" : "运行" }}
        </button>
        <select v-model="runMode" class="mongo-console-runmode" aria-label="执行范围">
          <option value="statement">运行当前命令</option>
          <option value="all">运行全部</option>
        </select>
        <span v-if="scopeLabel" class="mongo-console-scope">{{ scopeLabel }}</span>
        <span v-if="readOnly" class="mongo-console-write-note readonly">
          只读连接 · 写命令会被拒绝
        </span>
        <span v-else class="mongo-console-write-note">
          写命令立即生效，不支持撤销
        </span>
      </div>
      <div class="mongo-console-actions">
        <details class="mongo-console-examples">
          <summary>示例</summary>
          <div class="context-menu mongo-console-example-menu">
            <button
              v-for="example in examples"
              :key="example.label"
              class="context-menu-item"
              @click="insertExample(example.text); ($event.target as HTMLElement).closest('details')?.removeAttribute('open')"
            >
              {{ example.label }}
            </button>
          </div>
        </details>
        <div class="mongo-console-history-wrap">
          <button
            class="button ghost compact"
            :disabled="!history.length"
            @click.stop="historyOpen = !historyOpen"
          >
            <Icon name="history" :size="13" />
            历史
          </button>
          <div v-if="historyOpen" class="context-menu mongo-console-history-menu" @click.stop>
            <div class="mongo-console-history-head">
              <span>最近 50 条</span>
              <button class="link-button" @click="clearHistory">清空</button>
            </div>
            <button
              v-for="entry in history"
              :key="entry.id"
              class="context-menu-item mongo-console-history-item"
              :class="{ failed: !entry.ok }"
              @click="insertHistory(entry)"
            >
              <span class="mongo-console-history-command">
                {{ entry.command.replace(/\s+/g, " ").slice(0, 100) }}
              </span>
              <span class="hint">{{ entry.database }} · {{ entry.ok ? "成功" : "失败" }}</span>
            </button>
            <p v-if="!history.length" class="mongo-console-history-empty">暂无历史</p>
          </div>
        </div>
        <button
          class="button ghost compact"
          :disabled="!outcomes.length && !error"
          @click="clearResults"
        >
          清空结果
        </button>
      </div>
    </header>

    <div ref="editorHost" class="mongo-console-editor" aria-label="MongoDB 命令编辑器"></div>

    <div class="mongo-console-results">
      <div v-if="error" class="mongo-console-error" role="alert">
        <Icon name="warning" :size="14" />
        <span>{{ error }}</span>
      </div>
      <template v-if="outcomes.length">
        <div v-if="outcomes.length > 1" class="mongo-console-tabs" role="tablist">
          <button
            v-for="(outcome, index) in outcomes"
            :key="outcome.index"
            class="mongo-console-tab"
            :class="{ active: index === activeOutcomeIndex }"
            role="tab"
            :aria-selected="index === activeOutcomeIndex"
            @click="activeOutcomeIndex = index"
          >
            <strong>结果 {{ index + 1 }}</strong>
            <span>{{ outcomeSummary(outcome) }}</span>
            <em>{{ outcomeLabel(outcome) }}</em>
          </button>
        </div>
        <div v-if="currentOutcome" class="mongo-console-result">
          <div class="mongo-console-result-head">
            <span>
              {{
                currentOutcome.kind === "values"
                  ? `${currentOutcome.values.length} 个值`
                  : currentOutcome.kind === "documents"
                    ? `${currentOutcome.documents.length} 个文档`
                    : outcomeLabel(currentOutcome)
              }}
              · {{ currentOutcome.elapsedMs }} ms
              <template v-if="currentOutcome.collection">
                · {{ currentOutcome.database }} / {{ currentOutcome.collection }}
              </template>
            </span>
            <span class="spacer"></span>
            <button class="button ghost compact" @click="copyJson">复制 JSON</button>
            <button class="button ghost compact" @click="copyTsv">复制 TSV</button>
            <button class="button ghost compact" @click="exportResult">导出…</button>
          </div>
          <p v-if="currentOutcome.message" class="hint mongo-console-message">
            {{ currentOutcome.message }}
          </p>
          <p
            v-for="warning in currentOutcome.warnings"
            :key="warning"
            class="data-message warning mongo-console-warning"
          >
            {{ warning }}
          </p>
          <p v-if="currentOutcome.truncated" class="data-message warning mongo-console-warning">
            结果已截断，仅显示前 500 条。
          </p>
          <div v-if="currentOutcome.error" class="mongo-console-error" role="alert">
            <Icon name="warning" :size="14" />
            <span>{{ currentOutcome.error }}</span>
          </div>
          <div v-if="currentOutcome.values.length" class="mongo-console-value-list">
            <div v-for="(value, index) in currentOutcome.values" :key="index">
              <span class="hint">{{ index + 1 }}</span>
              <code>{{ mongoCellText(value) }}</code>
            </div>
          </div>
          <div v-else-if="resultRows.length" class="mongo-console-grid-wrap">
            <table class="mongo-grid">
              <thead>
                <tr>
                  <th class="mongo-index-col">#</th>
                  <th v-for="field in resultFields" :key="field">{{ field }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(document, index) in resultRows" :key="index">
                  <td class="mongo-index-col">{{ index + 1 }}</td>
                  <td
                    v-for="field in resultFields"
                    :key="field"
                    :class="{ 'mongo-null': document[field] === null }"
                    :title="mongoCellText(document[field])"
                  >
                    {{ mongoCellText(document[field]) || "—" }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-else class="mongo-console-empty">
            <Icon name="search" :size="18" />
            <span>{{ currentOutcome.error ? "命令失败" : "没有匹配文档" }}</span>
          </div>
        </div>
      </template>
      <div v-else-if="!error" class="mongo-console-empty">
        <Icon name="code" :size="20" />
        <strong>输入 MongoDB 命令后运行</strong>
        <span>
          支持 use / show 和 find、aggregate、count、distinct、insert、update、delete 等单条命令。
          参数使用 Extended JSON。
        </span>
      </div>
    </div>

    <div
      v-if="confirmations.length"
      class="modal-backdrop"
      @click.self="confirmations = []"
    >
      <div class="modal mongo-console-confirm" role="dialog" aria-modal="true">
        <header class="modal-header">
          <div>
            <h2>确认写入命令</h2>
            <p>以下命令可能影响大量文档或整个集合，确认后立即执行，不支持撤销。</p>
          </div>
          <button
            class="icon-button"
            title="关闭"
            aria-label="关闭确认"
            @click="confirmations = []"
          >
            <Icon name="close" :size="14" />
          </button>
        </header>
        <div class="mongo-console-confirm-list">
          <div v-for="item in confirmations" :key="item.index" class="mongo-console-confirm-item">
            <strong>第 {{ item.index }} 条 · {{ item.collection }}.{{ item.method }}()</strong>
            <span>{{ item.reason }}</span>
            <span class="hint">
              {{ item.database }} / {{ item.collection }}
              · 预计 {{ item.estimatedCount === null ? "未知" : `${item.estimatedCount} 个文档` }}
            </span>
            <code>{{ item.preview }}</code>
          </div>
        </div>
        <label class="mongo-console-confirm-check">
          <input v-model="confirmChecked" type="checkbox" />
          我确认将影响整个集合或大量文档
        </label>
        <footer class="modal-actions">
          <button class="button ghost" @click="confirmations = []">取消</button>
          <button
            class="button danger"
            :disabled="!confirmChecked || running"
            @click="confirmRun"
          >
            确认并执行
          </button>
        </footer>
      </div>
    </div>
  </div>
</template>
