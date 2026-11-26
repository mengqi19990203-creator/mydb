import { formatGridCell } from "./grid-search";
import type { QueryResult } from "./types";

export const COMPARE_ROW_LIMIT = 1000;
export const COMPARE_COLUMN_LIMIT = 200;

export type CompareStatus = "added" | "removed" | "changed" | "same";

export interface CompareColumn {
  key: string;
  name: string;
  index: number;
  duplicateName: boolean;
}

export interface CompareStructure {
  baseline: CompareColumn[];
  target: CompareColumn[];
  common: CompareColumn[];
  baselineOnly: CompareColumn[];
  targetOnly: CompareColumn[];
  commonTruncated: boolean;
}

export interface CompareRow {
  status: CompareStatus;
  key: string | null;
  baselineIndex: number | null;
  targetIndex: number | null;
  changedColumns: string[];
}

export interface CompareSummary {
  baselineRows: number;
  targetRows: number;
  added: number;
  removed: number;
  changed: number;
  same: number;
  commonColumns: number;
  structureDifferences: number;
}

export interface CompareOutcome {
  structure: CompareStructure;
  rows: CompareRow[];
  summary: CompareSummary;
  keyKey: string | null;
  keyLabel: string;
  keyError: string;
  rowMode: "key" | "row";
  truncated: boolean;
}

export interface CompareSource {
  id: string;
  name: string;
  sql: string;
  kind: "live" | "pinned";
  pinnedAt?: number;
  result: QueryResult;
}

function keysFor(columns: string[]): CompareColumn[] {
  const counts = new Map<string, number>();
  for (const name of columns) {
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return columns.map((name, index) => ({
    key: (counts.get(name) ?? 0) > 1 ? `${name}#${index + 1}` : name,
    name,
    index,
    duplicateName: (counts.get(name) ?? 0) > 1,
  }));
}

export function buildCompareColumns(columns: string[]): CompareColumn[] {
  return keysFor(columns);
}

export function compareStructure(
  baseline: QueryResult,
  target: QueryResult,
): CompareStructure {
  const left = keysFor(baseline.columns);
  const right = keysFor(target.columns);
  const rightKeys = new Set(right.map((column) => column.key));
  const leftKeys = new Set(left.map((column) => column.key));
  const common = left.filter((column) => rightKeys.has(column.key));
  return {
    baseline: left,
    target: right,
    common: common.slice(0, COMPARE_COLUMN_LIMIT),
    baselineOnly: left.filter((column) => !rightKeys.has(column.key)),
    targetOnly: right.filter((column) => !leftKeys.has(column.key)),
    commonTruncated: common.length > COMPARE_COLUMN_LIMIT,
  };
}

function displayed(value: unknown): string {
  return formatGridCell(value);
}

/** Returns a reason when a column cannot be used as a unique row key. */
export function keyColumnProblem(
  result: QueryResult,
  index: number,
): string {
  if (index < 0 || index >= result.columns.length) return "列不存在";
  const seen = new Set<string>();
  const rows = result.rows.slice(0, COMPARE_ROW_LIMIT);
  for (const row of rows) {
    const value = row[index];
    if (value === null || value === undefined) {
      return "该列存在 NULL，无法唯一匹配行";
    }
    const key = displayed(value);
    if (seen.has(key)) return "该列存在重复值，无法唯一匹配行";
    seen.add(key);
  }
  return "";
}

export function chooseDefaultKey(
  baseline: QueryResult,
  target: QueryResult,
  common: CompareColumn[],
): CompareColumn | null {
  const ordered = [...common].sort((left, right) => {
    const leftId = left.name.toLocaleLowerCase() === "id" ? 0 : 1;
    const rightId = right.name.toLocaleLowerCase() === "id" ? 0 : 1;
    return leftId - rightId;
  });
  for (const column of ordered) {
    if (!keyColumnProblem(baseline, column.index) &&
        !keyColumnProblem(target, column.index)) {
      return column;
    }
  }
  return null;
}

function compareRow(
  row: CompareRow,
  baseline: QueryResult,
  target: QueryResult,
  common: CompareColumn[],
  targetIndexByKey: Map<string, number>,
): void {
  if (row.baselineIndex === null || row.targetIndex === null) return;
  const left = baseline.rows[row.baselineIndex];
  const right = target.rows[row.targetIndex];
  const changed: string[] = [];
  for (const column of common) {
    const targetIndex = targetIndexByKey.get(column.key) ?? column.index;
    if (displayed(left[column.index]) !== displayed(right[targetIndex])) {
      changed.push(column.key);
    }
  }
  row.changedColumns = changed;
  row.status = changed.length ? "changed" : "same";
}

export function compareResultSets(
  baseline: QueryResult,
  target: QueryResult,
  keyKey: string | null,
): CompareOutcome {
  const structure = compareStructure(baseline, target);
  const common = structure.common;
  const targetIndexByKey = new Map(
    structure.target.map((column) => [column.key, column.index]),
  );
  const summary: CompareSummary = {
    baselineRows: baseline.rows.length,
    targetRows: target.rows.length,
    added: 0,
    removed: 0,
    changed: 0,
    same: 0,
    commonColumns: common.length,
    structureDifferences:
      structure.baselineOnly.length + structure.targetOnly.length,
  };
  const truncated =
    baseline.rows.length > COMPARE_ROW_LIMIT ||
    target.rows.length > COMPARE_ROW_LIMIT;
  const rows: CompareRow[] = [];

  if (!common.length) {
    return {
      structure,
      rows,
      summary,
      keyKey: null,
      keyLabel: "无共同列",
      keyError: "两边没有共同列，只能查看结构差异。",
      rowMode: "row",
      truncated,
    };
  }

  if (!keyKey) {
    const baselineCount = Math.min(baseline.rows.length, COMPARE_ROW_LIMIT);
    const targetCount = Math.min(target.rows.length, COMPARE_ROW_LIMIT);
    const paired = Math.min(baselineCount, targetCount);
    for (let index = 0; index < paired; index += 1) {
      const row: CompareRow = {
        status: "same",
        key: String(index + 1),
        baselineIndex: index,
        targetIndex: index,
        changedColumns: [],
      };
      compareRow(row, baseline, target, common, targetIndexByKey);
      rows.push(row);
    }
    for (let index = paired; index < baselineCount; index += 1) {
      rows.push({
        status: "removed",
        key: String(index + 1),
        baselineIndex: index,
        targetIndex: null,
        changedColumns: [],
      });
    }
    for (let index = paired; index < targetCount; index += 1) {
      rows.push({
        status: "added",
        key: String(index + 1),
        baselineIndex: null,
        targetIndex: index,
        changedColumns: [],
      });
    }
    for (const row of rows) summary[row.status] += 1;
    return {
      structure,
      rows,
      summary,
      keyKey: null,
      keyLabel: "按行号",
      keyError: "",
      rowMode: "row",
      truncated,
    };
  }

  const baselineKey = structure.baseline.find((column) => column.key === keyKey);
  const targetKey = structure.target.find((column) => column.key === keyKey);
  if (!baselineKey || !targetKey) {
    return {
      structure,
      rows,
      summary,
      keyKey: null,
      keyLabel: "无法匹配",
      keyError: "所选键列不在两边共同列中。",
      rowMode: "row",
      truncated,
    };
  }
  const problem =
    keyColumnProblem(baseline, baselineKey.index) ||
    keyColumnProblem(target, targetKey.index);
  if (problem) {
    return {
      structure,
      rows,
      summary,
      keyKey: null,
      keyLabel: baselineKey.name,
      keyError: problem,
      rowMode: "row",
      truncated,
    };
  }

  const targetMap = new Map<string, number>();
  const targetCount = Math.min(target.rows.length, COMPARE_ROW_LIMIT);
  for (let index = 0; index < targetCount; index += 1) {
    targetMap.set(displayed(target.rows[index][targetKey.index]), index);
  }
  const matched = new Set<number>();
  const baselineCount = Math.min(baseline.rows.length, COMPARE_ROW_LIMIT);
  for (let index = 0; index < baselineCount; index += 1) {
    const key = displayed(baseline.rows[index][baselineKey.index]);
    const targetIndex = targetMap.get(key);
    const row: CompareRow = {
      status: "removed",
      key,
      baselineIndex: index,
      targetIndex: targetIndex ?? null,
      changedColumns: [],
    };
    if (targetIndex !== undefined) {
      matched.add(targetIndex);
      compareRow(row, baseline, target, common, targetIndexByKey);
    }
    rows.push(row);
  }
  for (let index = 0; index < targetCount; index += 1) {
    if (matched.has(index)) continue;
    rows.push({
      status: "added",
      key: displayed(target.rows[index][targetKey.index]),
      baselineIndex: null,
      targetIndex: index,
      changedColumns: [],
    });
  }
  for (const row of rows) summary[row.status] += 1;
  return {
    structure,
    rows,
    summary,
    keyKey,
    keyLabel: baselineKey.duplicateName
      ? baselineKey.key
      : baselineKey.name,
    keyError: "",
    rowMode: "key",
    truncated,
  };
}

export function compareSummaryLines(
  outcome: CompareOutcome,
  baselineName: string,
  targetName: string,
): string[] {
  return [
    `基准\t${baselineName}`,
    `目标\t${targetName}`,
    `键\t${outcome.keyLabel}${outcome.rowMode === "row" ? "（顺序敏感）" : ""}`,
    `基准行数\t${outcome.summary.baselineRows}`,
    `目标行数\t${outcome.summary.targetRows}`,
    `新增\t${outcome.summary.added}`,
    `删除\t${outcome.summary.removed}`,
    `修改\t${outcome.summary.changed}`,
    `未变\t${outcome.summary.same}`,
    `共同列\t${outcome.summary.commonColumns}`,
    `结构差异\t${outcome.summary.structureDifferences}`,
    `说明\t按显示值比较；NULL 与空串不同`,
  ];
}

export function compareSummaryMarkdown(
  outcome: CompareOutcome,
  baselineName: string,
  targetName: string,
): string {
  return [
    "| 项目 | 值 |",
    "| --- | --- |",
    `| 基准 | ${baselineName} |`,
    `| 目标 | ${targetName} |`,
    `| 键 | ${outcome.keyLabel}${outcome.rowMode === "row" ? "（顺序敏感）" : ""} |`,
    `| 基准行数 | ${outcome.summary.baselineRows} |`,
    `| 目标行数 | ${outcome.summary.targetRows} |`,
    `| 新增 | ${outcome.summary.added} |`,
    `| 删除 | ${outcome.summary.removed} |`,
    `| 修改 | ${outcome.summary.changed} |`,
    `| 未变 | ${outcome.summary.same} |`,
    `| 共同列 | ${outcome.summary.commonColumns} |`,
    `| 结构差异 | ${outcome.summary.structureDifferences} |`,
  ].join("\n");
}

const STATUS_LABELS: Record<CompareStatus, string> = {
  added: "新增",
  removed: "删除",
  changed: "修改",
  same: "未变",
};

export function compareRowsTsv(
  outcome: CompareOutcome,
  baseline: QueryResult,
  target: QueryResult,
): string {
  const common = outcome.structure.common;
  const targetIndexByKey = new Map(
    outcome.structure.target.map((column) => [column.key, column.index]),
  );
  const header = [
    "状态",
    "键",
    ...common.map((column) => `基准:${column.name}`),
    ...common.map((column) => `目标:${column.name}`),
  ];
  const lines = [header.join("\t")];
  for (const row of outcome.rows) {
    const left =
      row.baselineIndex === null
        ? common.map(() => "")
        : common.map((column) =>
            displayed(baseline.rows[row.baselineIndex!][column.index]),
          );
    const right =
      row.targetIndex === null
        ? common.map(() => "")
        : common.map((column) =>
            displayed(
              target.rows[row.targetIndex!][
                targetIndexByKey.get(column.key) ?? column.index
              ],
            ),
          );
    lines.push(
      [
        STATUS_LABELS[row.status],
        row.key ?? "",
        ...left,
        ...right,
      ].join("\t"),
    );
  }
  return lines.join("\n");
}

export function compareStatusLabel(status: CompareStatus): string {
  return STATUS_LABELS[status];
}
