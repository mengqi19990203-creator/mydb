import type { DocsTable } from "./types";
import type { DocsNote } from "./database-docs";

function quote(value: string): string {
  return `"${value.replaceAll('"', '\\"')}"`;
}

function note(value: string): string {
  return `'${value.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`;
}

function typeName(value: string): string {
  const trimmed = value.trim() || "text";
  return /^[A-Za-z0-9_()., ]+$/.test(trimmed) ? trimmed : quote(trimmed);
}

function tableBlock(
  table: DocsTable,
  noteValue: DocsNote,
  warnings: string[],
): string {
  const primary = table.structure.columns.filter((column) => column.primaryKey);
  const composite = primary.length > 1;
  const lines: string[] = [`Table ${quote(table.name)} {`];

  for (const column of table.structure.columns) {
    const attributes: string[] = [];
    if (!composite && column.primaryKey) attributes.push("pk");
    if (!column.nullable) attributes.push("not null");
    if (column.autoIncrement) attributes.push("increment");
    if (column.defaultValue !== null && column.defaultValue !== undefined) {
      attributes.push(`default: ${note(column.defaultValue)}`);
    }
    const columnNote =
      noteValue.columns[column.name]?.trim() || column.comment?.trim() || "";
    if (columnNote) attributes.push(`note: ${note(columnNote)}`);
    lines.push(
      `  ${quote(column.name)} ${typeName(column.dataType)}${
        attributes.length ? ` [${attributes.join(", ")}]` : ""
      }`,
    );
  }

  const indexLines: string[] = [];
  for (const index of table.structure.indexes) {
    if (index.primary) {
      if (composite) {
        indexLines.push(
          `    (${index.columns.map(quote).join(", ")}) [pk, name: ${note(index.name)}]`,
        );
      }
      continue;
    }
    indexLines.push(
      `    (${index.columns.map(quote).join(", ")}) [${
        index.unique ? "unique, " : ""
      }name: ${note(index.name)}]`,
    );
  }
  if (indexLines.length) {
    lines.push("  indexes {");
    lines.push(...indexLines);
    lines.push("  }");
  }

  if (noteValue.tableNote.trim()) {
    lines.push(`  Note: ${note(noteValue.tableNote.trim())}`);
  }
  lines.push("}");
  if (table.kind !== "table") {
    warnings.push(`表 ${table.name} 是${table.kind === "view" ? "视图" : "物化视图"}，DBML 仅作为表导出`);
  }
  if (table.structure.indexes.some((index) => index.columns.some((column) => column.includes("(")))) {
    warnings.push(`表 ${table.name} 含表达式索引，DBML 可能无法完整表达`);
  }
  return lines.join("\n");
}

export function buildDbml(
  tables: DocsTable[],
  notes: Record<string, DocsNote>,
  projectName: string,
  databaseType: string,
): { text: string; warnings: string[] } {
  const warnings: string[] = [];
  const blocks: string[] = [
    `Project ${quote(projectName || "mydb_docs")} {`,
    `  database_type: ${note(databaseType)}`,
    "}",
    "",
  ];
  for (const table of tables) {
    blocks.push(tableBlock(table, notes[table.name] ?? { tableNote: "", columns: {} }, warnings));
    blocks.push("");
  }

  const refs: string[] = [];
  const seen = new Set<string>();
  for (const table of tables) {
    for (const foreignKey of table.structure.foreignKeys) {
      const key = `${table.name}:${foreignKey.name}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const local =
        foreignKey.columns.length === 1
          ? `${quote(table.name)}.${quote(foreignKey.columns[0])}`
          : `${quote(table.name)}.(${foreignKey.columns.map(quote).join(", ")})`;
      const remote =
        foreignKey.referencedColumns.length === 1
          ? `${quote(foreignKey.referencedTable)}.${quote(foreignKey.referencedColumns[0])}`
          : `${quote(foreignKey.referencedTable)}.(${foreignKey.referencedColumns.map(quote).join(", ")})`;
      refs.push(`Ref: ${local} > ${remote}`);
    }
  }
  if (!refs.length) {
    warnings.push("没有可用的外键关系；未生成 Ref");
  }
  return { text: [...blocks, ...refs].join("\n").trimEnd() + "\n", warnings };
}