<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { Driver } from "../lib/types";

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
  error.value = "";
  message.value = "";
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
    return `CREATE TABLE ${quote(name)} (\n  ${definitions.join(
      ",\n  ",
    )}\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;`;
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
  return `CREATE TABLE ${quote(name)} (\n  ${definitions.join(
    ",\n  ",
  )}\n);`;
}

function buildCommentStatements(): string[] {
  if (props.driver !== "postgres") return [];
  const name = tableName.value.trim();
  return columns.value
    .filter((column) => column.name.trim() && column.comment.trim())
    .map(
      (column) =>
        `COMMENT ON COLUMN ${quote(name)}.${quote(column.name.trim())} IS '${column.comment.replaceAll("'", "''")}';`,
    );
}

const ddl = computed(() => buildDdl());

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
  try {
    await api.executeQuery(props.connectionId, ddl.value, 1);
    let warning = "";
    const comments = buildCommentStatements();
    for (const statement of comments) {
      try {
        await api.executeQuery(props.connectionId, statement, 1);
      } catch (reason) {
        warning = `表已创建，但写入注释失败: ${reason}`;
        break;
      }
    }
    if (
      props.driver === "sqlite" &&
      columns.value.some(
        (column) => column.name.trim() && column.comment.trim(),
      )
    ) {
      warning ||= "SQLite 不支持字段注释，已忽略";
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
