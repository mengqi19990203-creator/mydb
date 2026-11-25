<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { Driver, TableInfo } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
}>();

const emit = defineEmits<{
  close: [];
  created: [table: string, warning?: string];
}>();

interface DraftColumn {
  name: string;
  type: string;
  nullable: boolean;
  primaryKey: boolean;
  autoIncrement: boolean;
  defaultValue: string;
  comment: string;
}

interface DraftIndex {
  name: string;
  columns: string;
  unique: boolean;
}

interface DraftForeignKey {
  name: string;
  columns: string;
  referencedTable: string;
  referencedColumns: string;
  onDelete: string;
  onUpdate: string;
}

const REFERENTIAL_ACTIONS = [
  "NO ACTION",
  "RESTRICT",
  "CASCADE",
  "SET NULL",
] as const;

const sqliteTypes = ["TEXT", "INTEGER", "REAL", "BLOB", "NUMERIC"];
const mysqlTypes = [
  "INT",
  "BIGINT",
  "VARCHAR(255)",
  "TEXT",
  "DECIMAL(10,2)",
  "DATETIME",
  "DATE",
  "BOOLEAN",
  "JSON",
];
const postgresTypes = [
  "INTEGER",
  "BIGINT",
  "VARCHAR(255)",
  "TEXT",
  "NUMERIC(10,2)",
  "TIMESTAMP",
  "DATE",
  "BOOLEAN",
  "JSONB",
  "UUID",
];

const tableName = ref("");
const columns = ref<DraftColumn[]>([]);
const indexes = ref<DraftIndex[]>([]);
const foreignKeys = ref<DraftForeignKey[]>([]);
const tableComment = ref("");
const availableTables = ref<TableInfo[]>([]);
const error = ref("");
const message = ref("");
const executing = ref(false);

const typeOptions = computed(() => {
  if (props.driver === "mysql") return mysqlTypes;
  if (props.driver === "postgres") return postgresTypes;
  return sqliteTypes;
});

function defaultColumn(index: number): DraftColumn {
  if (index === 0) {
    return reactive({
      name: "id",
      type:
        props.driver === "mysql"
          ? "BIGINT"
          : props.driver === "postgres"
            ? "BIGINT"
            : "INTEGER",
      nullable: false,
      primaryKey: true,
      autoIncrement: true,
      defaultValue: "",
      comment: "",
    });
  }
  return reactive({
    name: "",
    type:
      props.driver === "mysql"
        ? "VARCHAR(255)"
        : props.driver === "postgres"
          ? "VARCHAR(255)"
          : "TEXT",
    nullable: true,
    primaryKey: false,
    autoIncrement: false,
    defaultValue: "",
    comment: "",
  });
}

function reset() {
  tableName.value = "";
  const primary = defaultColumn(0);
  const label = defaultColumn(1);
  label.name = "name";
  columns.value = [primary, label];
  indexes.value = [];
  foreignKeys.value = [];
  tableComment.value = "";
  error.value = "";
  message.value = "";
}

async function loadReferencedTables() {
  try {
    availableTables.value = (await api.listTables(props.connectionId)).filter(
      (table) => table.kind === "table",
    );
  } catch {
    availableTables.value = [];
  }
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") {
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      void loadReferencedTables();
      document.addEventListener("keydown", onKeydown);
    } else {
      document.removeEventListener("keydown", onKeydown);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown);
});

function addColumn() {
  columns.value.push(defaultColumn(columns.value.length));
}

function addIndex() {
  indexes.value.push({ name: "", columns: "", unique: false });
}

function removeIndex(index: number) {
  indexes.value.splice(index, 1);
}

function addForeignKey() {
  foreignKeys.value.push({
    name: "",
    columns: "",
    referencedTable: availableTables.value[0]?.name ?? "",
    referencedColumns: "",
    onDelete: "NO ACTION",
    onUpdate: "NO ACTION",
  });
}

function removeForeignKey(index: number) {
  foreignKeys.value.splice(index, 1);
}

function removeColumn(index: number) {
  if (columns.value.length <= 1) {
    error.value = "至少保留一个字段";
    return;
  }
  columns.value.splice(index, 1);
}

function togglePrimary(index: number, event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  columns.value[index].primaryKey = checked;
  if (!checked) {
    columns.value[index].autoIncrement = false;
  }
}

function toggleAutoIncrement(index: number, event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  columns.value[index].autoIncrement = checked;
  if (checked) {
    columns.value[index].primaryKey = true;
    columns.value[index].nullable = false;
  }
}

function quote(name: string): string {
  if (props.driver === "mysql") {
    return `\`${name.replaceAll("`", "``")}\``;
  }
  return `"${name.replaceAll('"', '""')}"`;
}

function isIntegerType(type: string): boolean {
  return /^(TINYINT|SMALLINT|MEDIUMINT|INT|BIGINT|INTEGER)/i.test(type);
}

function splitColumns(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function generatedIndexName(columnsList: string[]): string {
  return `idx_${tableName.value.trim()}_${columnsList.join("_")}`;
}

function generatedForeignKeyName(columnsList: string[]): string {
  return `fk_${tableName.value.trim()}_${columnsList.join("_")}`;
}

function validate(): boolean {
  error.value = "";
  message.value = "";

  if (!tableName.value.trim()) {
    error.value = "请输入表名";
    return false;
  }

  const named = columns.value.filter((column) => column.name.trim());
  if (!named.length) {
    error.value = "至少填写一个字段";
    return false;
  }

  const names = new Set<string>();
  for (const column of named) {
    if (names.has(column.name.trim())) {
      error.value = `字段名重复: ${column.name.trim()}`;
      return false;
    }
    names.add(column.name.trim());
  }

  const primaryKeys = named.filter((column) => column.primaryKey);
  for (const column of named) {
    if (column.autoIncrement) {
      if (!column.primaryKey) {
        error.value = `自增字段 ${column.name} 必须同时是主键`;
        return false;
      }
      if (!isIntegerType(column.type)) {
        error.value = `自增字段 ${column.name} 必须是整数类型`;
        return false;
      }
      if (column.defaultValue.trim()) {
        error.value = `自增字段 ${column.name} 不能再设置默认值`;
        return false;
      }
      if (props.driver === "sqlite" && primaryKeys.length > 1) {
        error.value = "SQLite 的 AUTOINCREMENT 只能用于单字段主键";
        return false;
      }
      if (
        props.driver === "mysql" &&
        primaryKeys.length > 1 &&
        primaryKeys[0] !== column
      ) {
        error.value = `MySQL 自增字段 ${column.name} 必须是复合主键的第一列`;
        return false;
      }
    }
  }

  const indexNames = new Set<string>();
  for (const index of indexes.value) {
    const indexColumns = splitColumns(index.columns);
    if (!indexColumns.length) {
      error.value = "索引列不能为空";
      return false;
    }
    for (const column of indexColumns) {
      if (!names.has(column)) {
        error.value = `索引引用了不存在的字段: ${column}`;
        return false;
      }
    }
    const indexName =
      index.name.trim() || generatedIndexName(indexColumns);
    if (indexNames.has(indexName)) {
      error.value = `索引名重复: ${indexName}`;
      return false;
    }
    indexNames.add(indexName);
  }

  const foreignKeyNames = new Set<string>();
  for (const foreignKey of foreignKeys.value) {
    const localColumns = splitColumns(foreignKey.columns);
    const referencedColumns = splitColumns(foreignKey.referencedColumns);
    if (!localColumns.length || !referencedColumns.length) {
      error.value = "外键本地列和引用列都不能为空";
      return false;
    }
    if (localColumns.length !== referencedColumns.length) {
      error.value = "外键本地列与引用列数量必须一致";
      return false;
    }
    for (const column of localColumns) {
      if (!names.has(column)) {
        error.value = `外键引用了不存在的本地字段: ${column}`;
        return false;
      }
    }
    if (!foreignKey.referencedTable.trim()) {
      error.value = "请选择外键引用表";
      return false;
    }
    if (
      !availableTables.value.some(
        (table) => table.name === foreignKey.referencedTable,
      )
    ) {
      error.value = `外键引用表不存在: ${foreignKey.referencedTable}`;
      return false;
    }
    const foreignKeyName =
      foreignKey.name.trim() || generatedForeignKeyName(localColumns);
    if (foreignKeyNames.has(foreignKeyName)) {
      error.value = `外键名重复: ${foreignKeyName}`;
      return false;
    }
    foreignKeyNames.add(foreignKeyName);
  }

  return true;
}

function buildDdl(): string {
  const name = tableName.value.trim();
  if (!name) return "-- 请输入表名后预览 DDL";

  const named = columns.value.filter((column) => column.name.trim());
  if (!named.length) return "-- 请填写至少一个字段";

  const primaryKeys = named.filter((column) => column.primaryKey);
  const primaryColumns = primaryKeys
    .map((column) => quote(column.name.trim()))
    .join(", ");
  const sqliteInlinePrimary =
    props.driver === "sqlite" &&
    primaryKeys.length === 1 &&
    primaryKeys[0].autoIncrement;

  if (props.driver === "mysql") {
    const definitions = named.map((column) => {
      let definition = `${quote(column.name.trim())} ${column.type}`;
      if (!column.nullable) definition += " NOT NULL";
      if (column.defaultValue.trim()) {
        definition += ` DEFAULT ${column.defaultValue.trim()}`;
      }
      if (column.autoIncrement) definition += " AUTO_INCREMENT";
      if (column.comment.trim()) {
        definition += ` COMMENT '${column.comment
          .replaceAll("\\", "\\\\")
          .replaceAll("'", "''")}'`;
      }
      return definition;
    });
    if (primaryKeys.length) {
      definitions.push(`PRIMARY KEY (${primaryColumns})`);
    }
    for (const index of indexes.value) {
      const indexColumns = splitColumns(index.columns);
      if (!indexColumns.length) continue;
      const indexName =
        index.name.trim() || generatedIndexName(indexColumns);
      definitions.push(
        `${index.unique ? "UNIQUE " : ""}KEY ${quote(indexName)} (${indexColumns
          .map(quote)
          .join(", ")})`,
      );
    }
    for (const foreignKey of foreignKeys.value) {
      const localColumns = splitColumns(foreignKey.columns);
      const referencedColumns = splitColumns(foreignKey.referencedColumns);
      if (!localColumns.length || !referencedColumns.length) continue;
      const foreignKeyName =
        foreignKey.name.trim() || generatedForeignKeyName(localColumns);
      let definition = `CONSTRAINT ${quote(foreignKeyName)} FOREIGN KEY (${localColumns
        .map(quote)
        .join(", ")}) REFERENCES ${quote(foreignKey.referencedTable)} (${referencedColumns
        .map(quote)
        .join(", ")})`;
      if (foreignKey.onDelete !== "NO ACTION") {
        definition += ` ON DELETE ${foreignKey.onDelete}`;
      }
      if (foreignKey.onUpdate !== "NO ACTION") {
        definition += ` ON UPDATE ${foreignKey.onUpdate}`;
      }
      definitions.push(definition);
    }
    const comment = tableComment.value.trim()
      ? ` COMMENT='${tableComment.value
          .trim()
          .replaceAll("\\", "\\\\")
          .replaceAll("'", "''")}'`
      : "";
    return `CREATE TABLE ${quote(name)} (\n  ${definitions.join(
      ",\n  ",
    )}\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4${comment};`;
  }

  if (props.driver === "postgres") {
    const definitions = named.map((column) => {
      let definition = `${quote(column.name.trim())} ${column.type}`;
      if (column.autoIncrement) {
        definition += " GENERATED BY DEFAULT AS IDENTITY";
      }
      if (!column.nullable) definition += " NOT NULL";
      if (column.defaultValue.trim() && !column.autoIncrement) {
        definition += ` DEFAULT ${column.defaultValue.trim()}`;
      }
      return definition;
    });
    if (primaryKeys.length) {
      definitions.push(`PRIMARY KEY (${primaryColumns})`);
    }
    return `CREATE TABLE ${quote(name)} (\n  ${definitions.join(
      ",\n  ",
    )}\n);`;
  }

  const definitions = named.map((column) => {
    if (sqliteInlinePrimary && column === primaryKeys[0]) {
      return `${quote(column.name.trim())} INTEGER PRIMARY KEY AUTOINCREMENT`;
    }
    let definition = `${quote(column.name.trim())} ${column.type}`;
    if (!column.nullable) definition += " NOT NULL";
    if (column.defaultValue.trim()) {
      definition += ` DEFAULT ${column.defaultValue.trim()}`;
    }
    return definition;
  });
  if (!sqliteInlinePrimary && primaryKeys.length) {
    definitions.push(`PRIMARY KEY (${primaryColumns})`);
  }
  for (const foreignKey of foreignKeys.value) {
    const localColumns = splitColumns(foreignKey.columns);
    const referencedColumns = splitColumns(foreignKey.referencedColumns);
    if (!localColumns.length || !referencedColumns.length) continue;
    const foreignKeyName =
      foreignKey.name.trim() || generatedForeignKeyName(localColumns);
    let definition = `CONSTRAINT ${quote(foreignKeyName)} FOREIGN KEY (${localColumns
      .map(quote)
      .join(", ")}) REFERENCES ${quote(foreignKey.referencedTable)} (${referencedColumns
      .map(quote)
      .join(", ")})`;
    if (foreignKey.onDelete !== "NO ACTION") {
      definition += ` ON DELETE ${foreignKey.onDelete}`;
    }
    if (foreignKey.onUpdate !== "NO ACTION") {
      definition += ` ON UPDATE ${foreignKey.onUpdate}`;
    }
    definitions.push(definition);
  }
  return `CREATE TABLE ${quote(name)} (\n  ${definitions.join(
    ",\n  ",
  )}\n);`;
}

function buildCommentStatements(): string[] {
  if (props.driver !== "postgres") return [];
  const name = tableName.value.trim();
  const statements: string[] = [];
  if (tableComment.value.trim()) {
    statements.push(
      `COMMENT ON TABLE ${quote(name)} IS '${tableComment.value
        .trim()
        .replaceAll("'", "''")}';`,
    );
  }
  for (const column of columns.value) {
    if (column.name.trim() && column.comment.trim()) {
      statements.push(
        `COMMENT ON COLUMN ${quote(name)}.${quote(
          column.name.trim(),
        )} IS '${column.comment.replaceAll("'", "''")}';`,
      );
    }
  }
  return statements;
}

function buildIndexStatements(): string[] {
  if (props.driver === "mysql") return [];
  const name = tableName.value.trim();
  return indexes.value.flatMap((index) => {
    const indexColumns = splitColumns(index.columns);
    if (!indexColumns.length) return [];
    const indexName =
      index.name.trim() || generatedIndexName(indexColumns);
    return [
      `CREATE ${index.unique ? "UNIQUE " : ""}INDEX ${quote(
        indexName,
      )} ON ${quote(name)} (${indexColumns.map(quote).join(", ")});`,
    ];
  });
}

function buildForeignKeyStatements(): string[] {
  if (props.driver !== "postgres") return [];
  const name = tableName.value.trim();
  return foreignKeys.value.flatMap((foreignKey) => {
    const localColumns = splitColumns(foreignKey.columns);
    const referencedColumns = splitColumns(foreignKey.referencedColumns);
    if (!localColumns.length || !referencedColumns.length) return [];
    const foreignKeyName =
      foreignKey.name.trim() || generatedForeignKeyName(localColumns);
    let statement = `ALTER TABLE ${quote(
      name,
    )} ADD CONSTRAINT ${quote(foreignKeyName)} FOREIGN KEY (${localColumns
      .map(quote)
      .join(", ")}) REFERENCES ${quote(
      foreignKey.referencedTable,
    )} (${referencedColumns.map(quote).join(", ")})`;
    if (foreignKey.onDelete !== "NO ACTION") {
      statement += ` ON DELETE ${foreignKey.onDelete}`;
    }
    if (foreignKey.onUpdate !== "NO ACTION") {
      statement += ` ON UPDATE ${foreignKey.onUpdate}`;
    }
    return [`${statement};`];
  });
}

function buildStatements(): string[] {
  if (!tableName.value.trim()) return [];
  return [
    buildDdl(),
    ...buildCommentStatements(),
    ...buildIndexStatements(),
    ...buildForeignKeyStatements(),
  ];
}

const ddl = computed(() => {
  const statements = buildStatements();
  return statements.length
    ? statements.join("\n")
    : "-- 请输入表名后预览 DDL";
});

async function copyDdl() {
  try {
    await writeClipboard(ddl.value);
    message.value = "DDL 已复制到剪贴板";
  } catch (reason) {
    error.value = `复制失败: ${reason}`;
  }
}

async function execute() {
  if (!validate() || executing.value) return;

  executing.value = true;
  const statements = buildStatements();
  try {
    if (props.driver === "mysql") {
      for (const statement of statements) {
        await api.executeQuery(props.connectionId, statement, 1);
      }
    } else {
      const sessionId = `create-table-${Date.now()}`;
      await api.beginTransaction(props.connectionId, sessionId);
      try {
        for (const statement of statements) {
          await api.executeTransactionQuery(
            props.connectionId,
            sessionId,
            statement,
          );
        }
        await api.commitTransaction(props.connectionId, sessionId);
      } catch (reason) {
        try {
          await api.rollbackTransaction(props.connectionId, sessionId);
        } catch {
          // The original error is more useful than a rollback error.
        }
        throw reason;
      }
    }

    let warning = "";
    if (
      props.driver === "sqlite" &&
      (tableComment.value.trim() ||
        columns.value.some(
          (column) => column.name.trim() && column.comment.trim(),
        ))
    ) {
      warning = "SQLite 不支持表 / 字段注释，已忽略";
    }
    emit("created", tableName.value.trim(), warning || undefined);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    executing.value = false;
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal create-table-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="create-table-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="create-table-title">新建表</h2>
          <p>
            {{
              driver === "mysql"
                ? "MySQL / MariaDB"
                : driver === "postgres"
                  ? "PostgreSQL"
                  : "SQLite"
            }}
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label class="span-2">
            表名
            <input v-model="tableName" placeholder="例如：users" />
          </label>
        </div>

        <div class="draft-columns">
          <div class="draft-columns-head">
            <span>字段</span>
            <button class="button ghost compact" @click="addColumn">
              <Icon name="plus" :size="14" />
              添加字段
            </button>
          </div>

          <div class="column-editor-head">
            <span>字段名</span>
            <span>类型</span>
            <span>默认值</span>
            <span>注释</span>
            <span>可空</span>
            <span>主键</span>
            <span>自增</span>
            <span></span>
          </div>

          <div
            v-for="(column, index) in columns"
            :key="index"
            class="column-editor-row"
          >
            <input v-model="column.name" placeholder="字段名" />
            <select v-model="column.type">
              <option v-for="type in typeOptions" :key="type" :value="type">
                {{ type }}
              </option>
            </select>
            <input
              v-model="column.defaultValue"
              placeholder="默认值表达式"
              title="按 SQL 表达式填写；字符串请带单引号，例如 'abc'"
            />
            <input
              v-model="column.comment"
              placeholder="注释"
              title="MySQL 写入建表语句；PostgreSQL 建表后写入；SQLite 忽略"
            />
            <label class="inline-check">
              <input v-model="column.nullable" type="checkbox" />
              可空
            </label>
            <label class="inline-check">
              <input
                type="checkbox"
                :checked="column.primaryKey"
                @change="togglePrimary(index, $event)"
              />
              主键
            </label>
            <label class="inline-check">
              <input
                type="checkbox"
                :checked="column.autoIncrement"
                @change="toggleAutoIncrement(index, $event)"
              />
              自增
            </label>
            <button
              class="icon-button"
              title="删除字段"
              aria-label="删除字段"
              @click="removeColumn(index)"
            >
              <Icon name="trash" :size="14" />
            </button>
          </div>
        </div>

        <p class="draft-hint">
          默认值按 SQL 表达式填写（字符串请带单引号）；主键可多选组成复合主键。
          自增字段会同时设为主键；SQLite 自增主键固定使用 INTEGER PRIMARY KEY
          AUTOINCREMENT，PostgreSQL 使用 GENERATED BY DEFAULT AS IDENTITY。
          注释在 MySQL 写入建表语句，PostgreSQL 建表后执行 COMMENT ON，SQLite 忽略。
        </p>

        <label class="span-2 table-comment-field">
          表注释
          <input
            v-model="tableComment"
            placeholder="可选；MySQL 写入 COMMENT，PostgreSQL 生成 COMMENT ON TABLE"
          />
        </label>

        <div class="draft-columns">
          <div class="draft-columns-head">
            <span>索引</span>
            <button class="button ghost compact" @click="addIndex">
              <Icon name="plus" :size="14" />
              添加索引
            </button>
          </div>
          <div v-if="!indexes.length" class="draft-hint">
            可选；索引名留空时自动生成 idx_表名_列名。
          </div>
          <div
            v-for="(index, indexPosition) in indexes"
            :key="indexPosition"
            class="designer-row index-row"
          >
            <input v-model="index.name" placeholder="索引名（可留空）" />
            <input
              v-model="index.columns"
              placeholder="列，按顺序逗号分隔"
            />
            <label class="inline-check">
              <input v-model="index.unique" type="checkbox" />
              唯一
            </label>
            <button
              class="icon-button"
              title="删除索引"
              aria-label="删除索引"
              @click="removeIndex(indexPosition)"
            >
              <Icon name="trash" :size="14" />
            </button>
          </div>
        </div>

        <div class="draft-columns">
          <div class="draft-columns-head">
            <span>外键</span>
            <button
              class="button ghost compact"
              :disabled="!availableTables.length"
              @click="addForeignKey"
            >
              <Icon name="plus" :size="14" />
              添加外键
            </button>
          </div>
          <div v-if="!availableTables.length" class="draft-hint">
            当前范围没有可引用的表，或无法读取表列表。
          </div>
          <div v-else-if="!foreignKeys.length" class="draft-hint">
            可选；SQLite 会写入建表语句，PostgreSQL 建表后 ALTER TABLE 添加。
          </div>
          <div
            v-for="(foreignKey, foreignKeyPosition) in foreignKeys"
            :key="foreignKeyPosition"
            class="designer-row foreign-key-row"
          >
            <input v-model="foreignKey.name" placeholder="约束名（可留空）" />
            <input
              v-model="foreignKey.columns"
              placeholder="本地列，逗号分隔"
            />
            <select v-model="foreignKey.referencedTable">
              <option
                v-for="table in availableTables"
                :key="table.name"
                :value="table.name"
              >
                {{ table.name }}
              </option>
            </select>
            <input
              v-model="foreignKey.referencedColumns"
              placeholder="引用列，逗号分隔"
            />
            <select v-model="foreignKey.onDelete" title="ON DELETE">
              <option
                v-for="action in REFERENTIAL_ACTIONS"
                :key="`delete-${action}`"
                :value="action"
              >
                DELETE {{ action }}
              </option>
            </select>
            <select v-model="foreignKey.onUpdate" title="ON UPDATE">
              <option
                v-for="action in REFERENTIAL_ACTIONS"
                :key="`update-${action}`"
                :value="action"
              >
                UPDATE {{ action }}
              </option>
            </select>
            <button
              class="icon-button"
              title="删除外键"
              aria-label="删除外键"
              @click="removeForeignKey(foreignKeyPosition)"
            >
              <Icon name="trash" :size="14" />
            </button>
          </div>
          <p v-if="foreignKeys.length && driver === 'sqlite'" class="draft-hint">
            SQLite 要求被引用列具有唯一约束；这里只提示，不阻断创建。
          </p>
        </div>

        <section class="ddl-section">
          <h3>DDL 预览</h3>
          <pre class="ddl-preview">{{ ddl }}</pre>
        </section>

        <p v-if="message" class="message success">{{ message }}</p>
        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" @click="copyDdl">复制 DDL</button>
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button class="button primary" :disabled="executing" @click="execute">
          {{ executing ? "创建中…" : "执行创建" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.table-comment-field {
  margin-top: 10px;
}

.designer-row {
  display: grid;
  gap: 8px;
  align-items: center;
  margin-top: 8px;
}

.index-row {
  grid-template-columns: minmax(130px, 1fr) minmax(160px, 1.6fr) 70px 32px;
}

.foreign-key-row {
  grid-template-columns:
    minmax(110px, 0.9fr) minmax(100px, 0.8fr) minmax(110px, 0.9fr)
    minmax(100px, 0.8fr) minmax(110px, 0.8fr) minmax(110px, 0.8fr) 32px;
}

.designer-row input,
.designer-row select {
  min-width: 0;
}
</style>
