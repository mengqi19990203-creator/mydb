import type { DatabaseDocsSnapshot, DocsTable } from "./types";
import type { DocsNote } from "./database-docs";

export interface DocsMeta {
  connectionName: string;
  database: string;
  schema: string;
  driver: string;
  generatedAt: string;
}

const KIND_LABELS: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
};

function kindLabel(table: DocsTable): string {
  return KIND_LABELS[table.kind] ?? table.kind;
}

function noteOf(notes: Record<string, DocsNote>, table: string): DocsNote {
  return notes[table] ?? { tableNote: "", columns: {} };
}

function slug(value: string): string {
  return `table-${value.replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "")}`;
}

function mdCell(value: string): string {
  return (value || "—").replaceAll("\\", "\\\\").replaceAll("|", "\\|").replace(/\r?\n/g, "<br>");
}

function hasAnyRelation(tables: DocsTable[]): boolean {
  return tables.some((table) => table.structure.foreignKeys.length > 0);
}

export function buildMarkdown(
  snapshot: DatabaseDocsSnapshot,
  notes: Record<string, DocsNote>,
  meta: DocsMeta,
): string {
  const lines: string[] = [
    `# 数据库文档`,
    "",
    `- 连接：${meta.connectionName}（${meta.driver}）`,
    `- 数据库 / 模式：${meta.database || "—"}${meta.schema ? ` / ${meta.schema}` : ""}`,
    `- 生成时间：${meta.generatedAt}`,
    `- 表 / 视图：${snapshot.tables.length}`,
    "",
  ];
  if (snapshot.truncated) {
    lines.push("> 对象数超过 500，文档只包含前 500 个对象。", "");
  }
  if (snapshot.failures.length) {
    lines.push("## 读取失败", "");
    for (const failure of snapshot.failures) {
      lines.push(`- ${failure.name}：${failure.error}`);
    }
    lines.push("");
  }

  lines.push("## 目录", "");
  for (const table of snapshot.tables) {
    lines.push(`- [${table.name}（${kindLabel(table)}）](#${slug(table.name)})`);
  }
  lines.push("");

  for (const table of snapshot.tables) {
    const note = noteOf(notes, table.name);
    lines.push(`## ${table.name}`, "");
    lines.push(`类型：${kindLabel(table)}`, "");
    if (note.tableNote.trim()) {
      lines.push("### 本地笔记", "", note.tableNote.trim(), "");
    }
    lines.push("| 字段 | 类型 | 可空 | 默认值 | 主键 | 数据库注释 | 本地笔记 |");
    lines.push("| --- | --- | --- | --- | --- | --- | --- |");
    for (const column of table.structure.columns) {
      lines.push(
        `| ${mdCell(column.name)} | ${mdCell(column.dataType)} | ${
          column.nullable ? "是" : "否"
        } | ${mdCell(column.defaultValue ?? "")} | ${
          column.primaryKey ? "PK" : ""
        } | ${mdCell(column.comment ?? "")} | ${mdCell(
          note.columns[column.name] ?? "",
        )} |`,
      );
    }
    lines.push("");

    if (table.structure.indexes.length) {
      lines.push("### 索引", "");
      for (const index of table.structure.indexes) {
        lines.push(
          `- ${index.name}（${index.columns.join(", ")}）${
            index.primary ? " · 主键" : index.unique ? " · 唯一" : ""
          }`,
        );
      }
      lines.push("");
    }

    if (table.structure.foreignKeys.length) {
      lines.push("### 外键", "");
      for (const foreignKey of table.structure.foreignKeys) {
        lines.push(
          `- ${foreignKey.name}：${foreignKey.columns.join(", ")} → ${
            foreignKey.referencedTable
          }(${foreignKey.referencedColumns.join(", ")}) · ON DELETE ${
            foreignKey.onDelete || "—"
          } · ON UPDATE ${foreignKey.onUpdate || "—"}`,
        );
      }
      lines.push("");
    }
  }

  lines.push("## 关系", "");
  if (!hasAnyRelation(snapshot.tables)) {
    lines.push("没有可用的外键关系元数据。", "");
  } else {
    for (const table of snapshot.tables) {
      for (const foreignKey of table.structure.foreignKeys) {
        lines.push(
          `- ${table.name}(${foreignKey.columns.join(", ")}) → ${
            foreignKey.referencedTable
          }(${foreignKey.referencedColumns.join(", ")})`,
        );
      }
    }
    lines.push("");
  }
  return lines.join("\n");
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function buildHtml(
  snapshot: DatabaseDocsSnapshot,
  notes: Record<string, DocsNote>,
  meta: DocsMeta,
): string {
  const parts: string[] = [];
  parts.push(`<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>${escapeHtml(meta.connectionName)} · 数据库文档</title>
<style>
:root { color-scheme: light dark; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 0; padding: 32px; line-height: 1.55; color: #1f2328; background: #fff; }
main { max-width: 1100px; margin: 0 auto; }
h1 { margin: 0 0 6px; font-size: 26px; }
h2 { margin: 32px 0 8px; padding-top: 8px; border-top: 1px solid #d0d7de; font-size: 18px; }
h3 { margin: 18px 0 6px; font-size: 13px; color: #57606a; text-transform: uppercase; letter-spacing: .04em; }
p.meta { color: #57606a; margin: 2px 0; }
table { width: 100%; border-collapse: collapse; margin: 8px 0 14px; font-size: 12px; }
th, td { border: 1px solid #d0d7de; padding: 5px 8px; text-align: left; vertical-align: top; }
th { background: #f6f8fa; }
code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }
ul.toc { columns: 2; padding-left: 20px; font-size: 12px; }
.warning { padding: 8px 10px; border: 1px solid #d4a72c; background: #fff8c5; border-radius: 6px; font-size: 12px; }
.note { white-space: pre-wrap; background: #f6f8fa; padding: 8px 10px; border-radius: 6px; }
@media print { body { padding: 0; } h2 { break-before: auto; } table { break-inside: avoid; } }
</style>
</head>
<body><main>
<h1>数据库文档</h1>
<p class="meta">连接：${escapeHtml(meta.connectionName)}（${escapeHtml(meta.driver)}）</p>
<p class="meta">数据库 / 模式：${escapeHtml(meta.database || "—")}${meta.schema ? ` / ${escapeHtml(meta.schema)}` : ""}</p>
<p class="meta">生成时间：${escapeHtml(meta.generatedAt)} · 表 / 视图：${snapshot.tables.length}</p>`);

  if (snapshot.truncated) {
    parts.push('<p class="warning">对象数超过 500，文档只包含前 500 个对象。</p>');
  }
  if (snapshot.failures.length) {
    parts.push("<h2>读取失败</h2><ul>");
    for (const failure of snapshot.failures) {
      parts.push(
        `<li><code>${escapeHtml(failure.name)}</code>：${escapeHtml(failure.error)}</li>`,
      );
    }
    parts.push("</ul>");
  }

  parts.push('<h2>目录</h2><ul class="toc">');
  for (const table of snapshot.tables) {
    parts.push(
      `<li><a href="#${slug(table.name)}">${escapeHtml(table.name)}（${kindLabel(table)}）</a></li>`,
    );
  }
  parts.push("</ul>");

  for (const table of snapshot.tables) {
    const note = noteOf(notes, table.name);
    parts.push(
      `<h2 id="${slug(table.name)}">${escapeHtml(table.name)} <small>${kindLabel(table)}</small></h2>`,
    );
    if (note.tableNote.trim()) {
      parts.push(`<h3>本地笔记</h3><div class="note">${escapeHtml(note.tableNote)}</div>`);
    }
    parts.push(
      "<table><thead><tr><th>字段</th><th>类型</th><th>可空</th><th>默认值</th><th>主键</th><th>数据库注释</th><th>本地笔记</th></tr></thead><tbody>",
    );
    for (const column of table.structure.columns) {
      parts.push(
        `<tr><td><code>${escapeHtml(column.name)}</code></td><td>${escapeHtml(
          column.dataType,
        )}</td><td>${column.nullable ? "是" : "否"}</td><td>${escapeHtml(
          column.defaultValue ?? "",
        )}</td><td>${column.primaryKey ? "PK" : ""}</td><td>${escapeHtml(
          column.comment ?? "",
        )}</td><td>${escapeHtml(note.columns[column.name] ?? "")}</td></tr>`,
      );
    }
    parts.push("</tbody></table>");

    if (table.structure.indexes.length) {
      parts.push("<h3>索引</h3><ul>");
      for (const index of table.structure.indexes) {
        parts.push(
          `<li><code>${escapeHtml(index.name)}</code>（${escapeHtml(
            index.columns.join(", "),
          )}）${index.primary ? " · 主键" : index.unique ? " · 唯一" : ""}</li>`,
        );
      }
      parts.push("</ul>");
    }
    if (table.structure.foreignKeys.length) {
      parts.push("<h3>外键</h3><ul>");
      for (const foreignKey of table.structure.foreignKeys) {
        parts.push(
          `<li><code>${escapeHtml(foreignKey.name)}</code>：${escapeHtml(
            foreignKey.columns.join(", "),
          )} → ${escapeHtml(foreignKey.referencedTable)}(${escapeHtml(
            foreignKey.referencedColumns.join(", "),
          )})</li>`,
        );
      }
      parts.push("</ul>");
    }
  }

  parts.push("<h2>关系</h2>");
  if (!hasAnyRelation(snapshot.tables)) {
    parts.push("<p>没有可用的外键关系元数据。</p>");
  } else {
    parts.push("<ul>");
    for (const table of snapshot.tables) {
      for (const foreignKey of table.structure.foreignKeys) {
        parts.push(
          `<li>${escapeHtml(table.name)}(${escapeHtml(
            foreignKey.columns.join(", "),
          )}) → ${escapeHtml(foreignKey.referencedTable)}(${escapeHtml(
            foreignKey.referencedColumns.join(", "),
          )})</li>`,
        );
      }
    }
    parts.push("</ul>");
  }
  parts.push("</main></body></html>");
  return parts.join("\n");
}