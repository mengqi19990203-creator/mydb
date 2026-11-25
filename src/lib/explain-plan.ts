import type { ExplainResult } from "./types";

export interface ExplainNode {
  id: string;
  label: string;
  operation: string;
  table: string;
  index: string;
  cost: string;
  rows: string;
  detail: Array<{ key: string; value: string }>;
  children: ExplainNode[];
}

export interface ExplainSummary {
  nodeCount: number;
  truncated: boolean;
  topCost: string;
  topRows: string;
  width: string;
}

export interface ParsedExplain {
  roots: ExplainNode[];
  summary: ExplainSummary;
  raw: string;
  format: string;
  note: string;
}

const MAX_NODES = 2000;
const MAX_DEPTH = 50;

function valueText(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

function buildDetail(
  record: Record<string, unknown>,
  skipKeys: Set<string> = new Set(),
): Array<{ key: string; value: string }> {
  return Object.entries(record)
    .filter(([key]) => !skipKeys.has(key))
    .filter(([, value]) => typeof value !== "object" || value === null)
    .map(([key, value]) => ({ key, value: valueText(value) }));
}

function parsePostgres(
  payload: unknown,
  counter: { count: number },
): ExplainNode[] {
  const first = Array.isArray(payload) ? payload[0] : payload;
  const plan =
    first && typeof first === "object"
      ? (first as Record<string, unknown>).Plan
      : null;
  if (!plan || typeof plan !== "object") return [];
  return [pgNode(plan as Record<string, unknown>, counter, 0)];
}

function pgNode(
  record: Record<string, unknown>,
  counter: { count: number },
  depth: number,
): ExplainNode {
  counter.count += 1;
  const plans = Array.isArray(record.Plans) ? record.Plans : [];
  const children =
    depth >= MAX_DEPTH
      ? []
      : plans
          .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === "object")
          .slice(0, Math.max(0, MAX_NODES - counter.count))
          .map((child) => pgNode(child, counter, depth + 1));
  const operation = String(record["Node Type"] ?? "Plan");
  const table = String(record["Relation Name"] ?? "");
  const index = String(record["Index Name"] ?? "");
  return {
    id: `pg-${counter.count}`,
    label: operation,
    operation,
    table,
    index,
    cost: String(record["Total Cost"] ?? "—"),
    rows: String(record["Plan Rows"] ?? "—"),
    detail: buildDetail(record, new Set(["Plans"])),
    children,
  };
}

function parseMysql(
  payload: unknown,
  counter: { count: number },
): ExplainNode[] {
  if (!payload || typeof payload !== "object") return [];
  const record = payload as Record<string, unknown>;
  const root = mysqlNode(
    "Query Block",
    record.query_block && typeof record.query_block === "object"
      ? (record.query_block as Record<string, unknown>)
      : record,
    counter,
    0,
  );
  return [root];
}

function mysqlChildren(
  record: Record<string, unknown>,
  counter: { count: number },
  depth: number,
): ExplainNode[] {
  if (depth >= MAX_DEPTH) return [];
  const children: ExplainNode[] = [];
  const add = (value: unknown, label: string) => {
    if (counter.count >= MAX_NODES) return;
    if (Array.isArray(value)) {
      value.forEach((item, index) => {
        if (item && typeof item === "object") {
          children.push(
            mysqlNode(
              `${label} ${index + 1}`,
              item as Record<string, unknown>,
              counter,
              depth + 1,
            ),
          );
        }
      });
    } else if (value && typeof value === "object") {
      const entry = value as Record<string, unknown>;
      children.push(mysqlNode(label, entry, counter, depth + 1));
    }
  };

  add(record.nested_loop, "Nested Loop");
  add(record.ordering_operation, "Ordering");
  add(record.grouping_operation, "Grouping");
  add(record.duplicates_removal, "Duplicate Removal");
  add(record.union_result, "Union Result");
  add(record.materialized_from_subquery, "Materialized Subquery");
  add(record.table, "Table");
  return children;
}

function mysqlNode(
  label: string,
  record: Record<string, unknown>,
  counter: { count: number },
  depth: number,
): ExplainNode {
  counter.count += 1;
  const table = String(record.table_name ?? record.table ?? "");
  const operation =
    String(record.access_type ?? record.operation ?? label) || label;
  const index = String(record.key ?? "");
  const costInfo =
    record.cost_info && typeof record.cost_info === "object"
      ? (record.cost_info as Record<string, unknown>)
      : {};
  const children = mysqlChildren(record, counter, depth);
  return {
    id: `mysql-${counter.count}`,
    label: table ? `${operation} · ${table}` : label,
    operation,
    table,
    index,
    cost: valueText(
      costInfo.query_cost ??
        costInfo.read_cost ??
        record.cost ??
        "—",
    ),
    rows: valueText(
      record.rows_produced_per_join ??
        record.rows_examined_per_scan ??
        "—",
    ),
    detail: buildDetail(
      record,
      new Set([
        "nested_loop",
        "ordering_operation",
        "grouping_operation",
        "duplicates_removal",
        "union_result",
        "materialized_from_subquery",
        "table",
      ]),
    ),
    children,
  };
}

function parseSqliteTable(
  result: ExplainResult,
  counter: { count: number },
): ExplainNode[] {
  const columns = result.result.columns.map((column) => column.toLowerCase());
  const idIndex = columns.indexOf("id");
  const parentIndex = columns.indexOf("parent");
  const detailIndex = columns.indexOf("detail");
  if (idIndex < 0 || parentIndex < 0 || detailIndex < 0) return [];
  const nodes = new Map<string, ExplainNode>();
  const parents = new Map<string, string | null>();
  for (const row of result.result.rows) {
    const id = valueText(row[idIndex]);
    const parent = valueText(row[parentIndex]);
    const detail = valueText(row[detailIndex]);
    counter.count += 1;
    nodes.set(id, {
      id: `sqlite-${id}`,
      label: detail,
      operation: detail,
      table: "",
      index: "",
      cost: "—",
      rows: "—",
      detail: [{ key: "detail", value: detail }],
      children: [],
    });
    parents.set(id, parent === "0" || parent === "—" ? null : parent);
  }
  for (const [id, node] of nodes) {
    const parent = parents.get(id);
    if (parent && nodes.has(parent)) {
      nodes.get(parent)!.children.push(node);
    }
  }
  return [...nodes.entries()]
    .filter(([id]) => {
      const parent = parents.get(id);
      return !parent || !nodes.has(parent);
    })
    .map(([, node]) => node);
}

function parseTraditionalTable(
  result: ExplainResult,
  counter: { count: number },
): ExplainNode[] {
  const columns = result.result.columns.map((column) => column.toLowerCase());
  const rows = result.result.rows;
  if (!rows.length) return [];
  const tableIndex = columns.indexOf("table");
  const typeIndex = columns.indexOf("type");
  const keyIndex = columns.indexOf("key");
  const rowsIndex = columns.indexOf("rows");
  const children = rows.map((row, index) => {
    counter.count += 1;
    const table = tableIndex >= 0 ? valueText(row[tableIndex]) : "";
    const type = typeIndex >= 0 ? valueText(row[typeIndex]) : "Table";
    return {
      id: `table-${index + 1}`,
      label: table ? `${type} · ${table}` : type,
      operation: type,
      table,
      index: keyIndex >= 0 ? valueText(row[keyIndex]) : "",
      cost: "—",
      rows: rowsIndex >= 0 ? valueText(row[rowsIndex]) : "—",
      detail: result.result.columns.map((column, columnIndex) => ({
        key: column ?? `column ${columnIndex + 1}`,
        value: valueText(row[columnIndex]),
      })),
      children: [],
    } satisfies ExplainNode;
  });
  return [
    {
      id: "table-root",
      label: "传统计划",
      operation: "EXPLAIN",
      table: "",
      index: "",
      cost: "—",
      rows: "—",
      detail: [],
      children,
    },
  ];
}

function flattenCount(node: ExplainNode): number {
  return 1 + node.children.reduce((sum, child) => sum + flattenCount(child), 0);
}

function topValues(nodes: ExplainNode[]): { cost: string; rows: string } {
  let cost = 0;
  let rows = 0;
  let hasCost = false;
  let hasRows = false;
  const walk = (node: ExplainNode) => {
    const numericCost = Number(node.cost);
    if (Number.isFinite(numericCost)) {
      cost = Math.max(cost, numericCost);
      hasCost = true;
    }
    const numericRows = Number(node.rows);
    if (Number.isFinite(numericRows)) {
      rows = Math.max(rows, numericRows);
      hasRows = true;
    }
    node.children.forEach(walk);
  };
  nodes.forEach(walk);
  return {
    cost: hasCost ? String(cost) : "—",
    rows: hasRows ? String(rows) : "—",
  };
}

export function parseExplainResult(result: ExplainResult): ParsedExplain {
  const counter = { count: 0 };
  const raw = result.result.rows
    .map((row) => row.map(valueText).join("\t"))
    .join("\n");
  let roots: ExplainNode[] = [];
  let format = result.format;
  let note = "";

  const firstCell = result.result.rows[0]?.[0];
  if (format === "json" && firstCell !== undefined) {
    try {
      const payload =
        typeof firstCell === "string" ? JSON.parse(firstCell) : firstCell;
      roots =
        result.driver === "postgres"
          ? parsePostgres(payload, counter)
          : parseMysql(payload, counter);
    } catch {
      note = "JSON 计划解析失败，已降级为原始计划";
      format = "text";
    }
  }
  if (!roots.length) {
    roots =
      result.driver === "sqlite"
        ? parseSqliteTable(result, counter)
        : parseTraditionalTable(result, counter);
    if (roots.length) {
      format = "table";
      if (result.driver === "sqlite") {
        note = "SQLite 不提供成本 / 行数估算";
      }
    }
  }

  const nodeCount = roots.reduce((sum, root) => sum + flattenCount(root), 0);
  const top = topValues(roots);
  return {
    roots,
    summary: {
      nodeCount,
      truncated: nodeCount >= MAX_NODES,
      topCost: top.cost,
      topRows: top.rows,
      width: "—",
    },
    raw,
    format,
    note,
  };
}

export interface FlatExplainNode {
  node: ExplainNode;
  depth: number;
}

export function flattenExplain(
  nodes: ExplainNode[],
  expanded: Set<string>,
  depth = 0,
): FlatExplainNode[] {
  const list: FlatExplainNode[] = [];
  for (const node of nodes) {
    list.push({ node, depth });
    if (expanded.has(node.id)) {
      list.push(...flattenExplain(node.children, expanded, depth + 1));
    }
  }
  return list;
}

export function defaultExpandedIds(nodes: ExplainNode[]): Set<string> {
  const expanded = new Set<string>();
  const walk = (node: ExplainNode, depth: number) => {
    if (depth <= 1) expanded.add(node.id);
    node.children.forEach((child) => walk(child, depth + 1));
  };
  nodes.forEach((node) => walk(node, 0));
  return expanded;
}