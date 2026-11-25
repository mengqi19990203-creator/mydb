import { quoteIdentifier, sqlLiteral } from "./sql-literal";
import type { ColumnInfo, Driver, GeneratorColumn } from "./types";

export const GENERATOR_OPTIONS: { value: string; label: string; hint?: string }[] = [
  { value: "sequence", label: "序列", hint: "起始值 + 步长" },
  { value: "integer", label: "整数", hint: "最小 / 最大" },
  { value: "decimal", label: "小数", hint: "范围与小数位" },
  { value: "text", label: "文本", hint: "长度与字符集" },
  { value: "regex", label: "正则", hint: "有限正则" },
  { value: "email", label: "邮箱", hint: "合成地址" },
  { value: "name", label: "姓名", hint: "合成姓名" },
  { value: "phone", label: "电话", hint: "合成号码" },
  { value: "company", label: "公司", hint: "合成公司名" },
  { value: "date", label: "日期", hint: "起止日期" },
  { value: "datetime", label: "日期时间", hint: "起止日期" },
  { value: "boolean", label: "布尔", hint: "true 比例" },
  { value: "uuid", label: "UUID", hint: "v4 格式" },
  { value: "fixed", label: "固定值", hint: "每行相同" },
  { value: "null", label: "始终 NULL", hint: "仅可空列" },
];

export function isBinaryType(dataType: string): boolean {
  return /(BLOB|BINARY|VARBINARY|BYTEA|BIT|IMAGE|RAW|GEOMETRY)/i.test(dataType);
}

function hasType(dataType: string, needles: string[]): boolean {
  const upper = dataType.toUpperCase();
  return needles.some((needle) => upper.includes(needle));
}

export function defaultGenerator(column: ColumnInfo): GeneratorColumn {
  const base: GeneratorColumn = {
    name: column.name,
    generator: "text",
    enabled: true,
    nullPercent: 0,
    sequenceStart: 1,
    sequenceStep: 1,
    minNumber: 1,
    maxNumber: 1_000_000,
    decimals: 2,
    textMin: 4,
    textMax: 16,
    charset: "alphanumeric",
    regex: "",
    dateFrom: "2024-01-01",
    dateTo: "2026-12-31",
    boolPercent: 50,
    fixedValue: "",
  };

  if (column.primaryKey && !column.autoIncrement) {
    return { ...base, generator: "sequence" };
  }
  const lowerName = column.name.toLowerCase();
  if (/email|mail/.test(lowerName)) return { ...base, generator: "email" };
  if (/phone|mobile|tel/.test(lowerName)) return { ...base, generator: "phone" };
  if (/company|org/.test(lowerName)) return { ...base, generator: "company" };
  if (/name/.test(lowerName)) return { ...base, generator: nameLike(column) };

  if (hasType(column.dataType, ["UUID"])) return { ...base, generator: "uuid" };
  if (hasType(column.dataType, ["BOOL"])) return { ...base, generator: "boolean" };
  if (hasType(column.dataType, ["TIMESTAMP", "DATETIME"])) {
    return { ...base, generator: "datetime", dateFrom: "2024-01-01 00:00:00", dateTo: "2026-12-31 23:59:59" };
  }
  if (hasType(column.dataType, ["DATE"])) return { ...base, generator: "date" };
  if (hasType(column.dataType, ["DECIMAL", "NUMERIC", "FLOAT", "DOUBLE", "REAL"])) {
    return { ...base, generator: "decimal", minNumber: 0, maxNumber: 10_000 };
  }
  if (hasType(column.dataType, ["INT", "SERIAL", "NUMBER"])) {
    return { ...base, generator: "integer" };
  }
  if (hasType(column.dataType, ["JSON"])) {
    return { ...base, generator: "fixed", fixedValue: "{}" };
  }
  return base;
}

function nameLike(column: ColumnInfo): string {
  return column.dataType.toUpperCase().includes("COMPANY") ? "company" : "name";
}

export function visibleGeneratorColumns(columns: ColumnInfo[]): ColumnInfo[] {
  return columns.filter(
    (column) =>
      !column.autoIncrement &&
      !isBinaryType(column.dataType) &&
      !/GENERATED/i.test(column.dataType),
  );
}

export function buildPreviewSql(
  table: string,
  driver: Driver,
  columns: string[],
  rows: unknown[][],
): string {
  const target = quoteIdentifier(table, driver);
  const fields = columns.map((column) => quoteIdentifier(column, driver)).join(", ");
  return rows
    .slice(0, 20)
    .map((row) => {
      const values = row
        .map((value) => sqlLiteral(value, driver))
        .join(", ");
      return `INSERT INTO ${target} (${fields}) VALUES (${values});`;
    })
    .join("\n");
}

export function generatorLabel(generator: string): string {
  return GENERATOR_OPTIONS.find((item) => item.value === generator)?.label ?? generator;
}