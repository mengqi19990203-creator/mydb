import { formatGridCell } from "./grid-search";

export type ResultFilterOperator =
  | "contains"
  | "not-contains"
  | "equals"
  | "not-equals"
  | "gt"
  | "gte"
  | "lt"
  | "lte"
  | "is-null"
  | "not-null";

export interface ResultFilterCondition {
  id: number;
  columnIndex: number;
  operator: ResultFilterOperator;
  value: string;
}

export const RESULT_FILTER_OPERATORS: Array<{
  value: ResultFilterOperator;
  label: string;
  needsValue: boolean;
  numeric?: boolean;
}> = [
  { value: "contains", label: "包含", needsValue: true },
  { value: "not-contains", label: "不包含", needsValue: true },
  { value: "equals", label: "等于", needsValue: true },
  { value: "not-equals", label: "不等于", needsValue: true },
  { value: "gt", label: "大于", needsValue: true, numeric: true },
  { value: "gte", label: "大于等于", needsValue: true, numeric: true },
  { value: "lt", label: "小于", needsValue: true, numeric: true },
  { value: "lte", label: "小于等于", needsValue: true, numeric: true },
  { value: "is-null", label: "为空", needsValue: false },
  { value: "not-null", label: "不为空", needsValue: false },
];

export const RESULT_FILTER_LIMIT = 5;

function operatorInfo(operator: ResultFilterOperator) {
  return RESULT_FILTER_OPERATORS.find((item) => item.value === operator);
}

export function isFilterConditionActive(
  condition: ResultFilterCondition,
): boolean {
  const info = operatorInfo(condition.operator);
  if (!info) return false;
  if (!info.needsValue) return true;
  return condition.value.trim().length > 0;
}

function textOf(value: unknown): string {
  return formatGridCell(value);
}

function equalsValue(
  cell: unknown,
  value: string,
  caseSensitive: boolean,
): boolean {
  if (cell === null || cell === undefined) return false;
  if (typeof cell === "number") {
    const number = Number(value);
    return Number.isFinite(number) && cell === number;
  }
  if (typeof cell === "boolean") {
    return value.trim().toLowerCase() === String(cell);
  }
  const left = textOf(cell);
  return caseSensitive
    ? left === value
    : left.toLowerCase() === value.toLowerCase();
}

export function matchesResultFilter(
  row: readonly unknown[],
  conditions: readonly ResultFilterCondition[],
  caseSensitive: boolean,
): boolean {
  for (const condition of conditions) {
    if (!isFilterConditionActive(condition)) continue;
    const cell = row[condition.columnIndex];
    const value = condition.value.trim();
    switch (condition.operator) {
      case "contains":
      case "not-contains": {
        if (cell === null || cell === undefined) return false;
        const haystack = textOf(cell);
        const hit = caseSensitive
          ? haystack.includes(condition.value)
          : haystack.toLowerCase().includes(condition.value.toLowerCase());
        if (condition.operator === "contains" ? !hit : hit) return false;
        break;
      }
      case "equals":
      case "not-equals": {
        const hit = equalsValue(cell, condition.value, caseSensitive);
        if (condition.operator === "equals" ? !hit : hit) return false;
        break;
      }
      case "gt":
      case "gte":
      case "lt":
      case "lte": {
        const left = Number(cell);
        const right = Number(value);
        if (
          cell === null ||
          cell === undefined ||
          !Number.isFinite(left) ||
          !Number.isFinite(right)
        ) {
          return false;
        }
        const hit =
          condition.operator === "gt"
            ? left > right
            : condition.operator === "gte"
              ? left >= right
              : condition.operator === "lt"
                ? left < right
                : left <= right;
        if (!hit) return false;
        break;
      }
      case "is-null":
        if (cell !== null && cell !== undefined) return false;
        break;
      case "not-null":
        if (cell === null || cell === undefined) return false;
        break;
    }
  }
  return true;
}

export function filterResultRows(
  rows: readonly unknown[][],
  conditions: readonly ResultFilterCondition[],
  caseSensitive: boolean,
): unknown[][] {
  if (!conditions.some(isFilterConditionActive)) return [...rows];
  return rows.filter((row) => matchesResultFilter(row, conditions, caseSensitive));
}

export function describeFilterCondition(
  condition: ResultFilterCondition,
  columns: readonly string[],
): string {
  const label =
    RESULT_FILTER_OPERATORS.find((item) => item.value === condition.operator)
      ?.label ?? condition.operator;
  const name = columns[condition.columnIndex] ?? `列 ${condition.columnIndex + 1}`;
  const info = RESULT_FILTER_OPERATORS.find(
    (item) => item.value === condition.operator,
  );
  return info?.needsValue
    ? `${name} ${label} ${condition.value.trim()}`
    : `${name} ${label}`;
}

export function describeFilter(
  conditions: readonly ResultFilterCondition[],
  columns: readonly string[],
): string | null {
  const active = conditions.filter(isFilterConditionActive);
  if (!active.length) return null;
  return active
    .map((condition) => describeFilterCondition(condition, columns))
    .join(" AND ");
}