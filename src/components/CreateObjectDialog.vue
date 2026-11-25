<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { writeClipboard } from "../lib/clipboard";
import type { Driver, TableInfo } from "../lib/types";

type ObjectKind = "view" | "procedure" | "function" | "trigger";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  kind: ObjectKind;
  scopeLabel: string;
  tables: TableInfo[];
}>();

const emit = defineEmits<{
  close: [];
  created: [payload: { kind: ObjectKind; name: string }];
}>();

const name = ref("");
const table = ref("");
const params = ref("");
const returns = ref("integer");
const body = ref("");
const orReplace = ref(true);
const timing = ref("BEFORE");
const event = ref("INSERT");
const error = ref("");
const executing = ref(false);
const message = ref("");

const kindLabel = computed(() => {
  switch (props.kind) {
    case "view":
      return "视图";
    case "procedure":
      return "存储过程";
    case "function":
      return "函数";
    default:
      return "触发器";
  }
});

function quote(identifier: string): string {
  if (props.driver === "mysql") {
    return `\`${identifier.replaceAll("`", "``")}\``;
  }
  return `"${identifier.replaceAll('"', '""')}"`;
}

function reset() {
  name.value = "";
  table.value = props.tables[0]?.name ?? "";
  params.value = "";
  returns.value = "integer";
  orReplace.value = true;
  timing.value = "BEFORE";
  event.value = "INSERT";
  error.value = "";
  message.value = "";
  body.value =
    props.kind === "view"
      ? "SELECT *\nFROM "
      : "-- 在这里编写语句；每句以分号结尾\n";
}

watch(
  () => props.open,
  (open) => {
    if (open) reset();
  },
  { immediate: true },
);

function buildStatements(): string[] {
  const objectName = name.value.trim();
  if (!objectName) return [];
  const quoted = quote(objectName);
  const tableRef = quote(table.value.trim() || "table");
  const paramsText = params.value.trim();
  const bodyText = body.value.trim();
  const replace =
    orReplace.value && props.driver !== "sqlite" ? "CREATE OR REPLACE" : "CREATE";

  if (props.kind === "view") {
    return [
      `${replace} VIEW ${quoted} AS\n${bodyText || "SELECT 1"};`,
    ];
  }
  if (props.kind === "procedure") {
    if (props.driver === "postgres") {
      return [
        `CREATE OR REPLACE PROCEDURE ${quoted}(${paramsText})\nLANGUAGE plpgsql\nAS $$\nBEGIN\n${bodyText}\nEND;\n$$;`,
      ];
    }
    return [
      `CREATE PROCEDURE ${quoted}(${paramsText})\nBEGIN\n${bodyText}\nEND;`,
    ];
  }
  if (props.kind === "function") {
    if (props.driver === "postgres") {
      return [
        `CREATE OR REPLACE FUNCTION ${quoted}(${paramsText}) RETURNS ${returns.value.trim() || "integer"}\nLANGUAGE plpgsql\nAS $$\nBEGIN\n${bodyText}\nEND;\n$$;`,
      ];
    }
    return [
      `CREATE FUNCTION ${quoted}(${paramsText}) RETURNS ${returns.value.trim() || "INT"}\nBEGIN\n${bodyText}\nEND;`,
    ];
  }

  if (props.driver === "postgres") {
    const functionName = `${objectName}_fn`;
    return [
      `CREATE OR REPLACE FUNCTION ${quote(functionName)}() RETURNS trigger\nLANGUAGE plpgsql\nAS $$\nBEGIN\n${bodyText}\nRETURN NEW;\nEND;\n$$;`,
      `DROP TRIGGER IF EXISTS ${quoted} ON ${tableRef};`,
      `CREATE TRIGGER ${quoted} ${timing.value} ${event.value} ON ${tableRef}\nFOR EACH ROW EXECUTE FUNCTION ${quote(functionName)}();`,
    ];
  }
  return [
    `CREATE TRIGGER ${quoted} ${timing.value} ${event.value} ON ${tableRef}\nFOR EACH ROW\nBEGIN\n${bodyText}\nEND;`,
  ];
}

const statements = computed(() => buildStatements());
const ddl = computed(() =>
  statements.value.length
    ? statements.value.join("\n")
    : "-- 填写对象名称后预览 DDL",
);

async function copyDdl() {
  try {
    await writeClipboard(ddl.value);
    message.value = "DDL 已复制";
  } catch (reason) {
    error.value = String(reason);
  }
}

async function execute() {
  if (executing.value) return;
  error.value = "";
  message.value = "";
  if (!name.value.trim()) {
    error.value = "请输入对象名称";
    return;
  }
  if (props.kind === "trigger" && !table.value.trim()) {
    error.value = "请选择触发器所属表";
    return;
  }
  const sql = buildStatements();
  if (!sql.length) return;
  executing.value = true;
  try {
    if (sql.length === 1) {
      await api.executeQuery(props.connectionId, sql[0], 1);
    } else {
      const sessionId = `create-object-${Date.now()}`;
      await api.beginTransaction(props.connectionId, sessionId);
      try {
        for (const statement of sql) {
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
          // Keep the original failure.
        }
        throw reason;
      }
    }
    emit("created", { kind: props.kind, name: name.value.trim() });
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
      :aria-label="`新建${kindLabel}`"
    >
      <header class="modal-header">
        <div>
          <h2>新建{{ kindLabel }}</h2>
          <p>{{ scopeLabel }} · {{ driver === "mysql" ? "MySQL" : driver === "postgres" ? "PostgreSQL" : "SQLite" }}</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label>
            {{ kindLabel }}名称
            <input v-model="name" placeholder="例如：active_users" />
          </label>
          <label v-if="kind === 'procedure' || kind === 'function'">
            参数
            <input v-model="params" placeholder="例如：p_id integer" />
          </label>
          <label v-if="kind === 'function'">
            返回类型
            <input v-model="returns" placeholder="例如：integer" />
          </label>
          <template v-if="kind === 'trigger'">
            <label>
              所属表
              <select v-model="table">
                <option
                  v-for="item in tables.filter((entry) => entry.kind === 'table')"
                  :key="item.name"
                  :value="item.name"
                >
                  {{ item.name }}
                </option>
              </select>
            </label>
            <label>
              触发时机
              <select v-model="timing">
                <option value="BEFORE">BEFORE</option>
                <option value="AFTER">AFTER</option>
                <option v-if="driver !== 'sqlite'" value="INSTEAD OF">
                  INSTEAD OF
                </option>
              </select>
            </label>
            <label>
              触发事件
              <select v-model="event">
                <option value="INSERT">INSERT</option>
                <option value="UPDATE">UPDATE</option>
                <option value="DELETE">DELETE</option>
              </select>
            </label>
          </template>
          <label
            v-if="kind === 'view' || kind === 'procedure' || kind === 'function'"
            class="checkbox-row span-2"
          >
            <input
              v-model="orReplace"
              type="checkbox"
              :disabled="driver === 'sqlite'"
            />
            使用 CREATE OR REPLACE（SQLite 不支持）
          </label>
          <label class="span-2">
            定义体
            <textarea
              v-model="body"
              class="object-body-input"
              rows="10"
              spellcheck="false"
              placeholder="SQL 定义体"
            ></textarea>
          </label>
        </div>
        <p class="draft-hint">
          定义体会由工具按驱动模板包裹；MySQL 存储过程 / 函数 / 触发器作为单条 CREATE 语句发送，不需要手动写 DELIMITER。
        </p>
        <p v-if="driver === 'postgres' && kind === 'trigger'" class="draft-hint">
          PostgreSQL 会同时创建 `{{ name || "对象" }}_fn` 触发器函数，并在一个事务中创建触发器。
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
        <button
          class="button primary"
          :disabled="executing || !name.trim()"
          @click="execute"
        >
          {{ executing ? "创建中…" : "执行创建" }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.object-body-input {
  width: 100%;
  min-height: 180px;
  padding: 10px;
  resize: vertical;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.55;
}
</style>