<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import { quoteIdentifier } from "../lib/sql-literal";
import type { Driver, ServerUser } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  driver: Driver;
  user: ServerUser | null;
  database?: string | null;
  schema?: string | null;
  readOnly?: boolean;
  initialAction?: "grant" | "revoke";
}>();

const emit = defineEmits<{
  close: [];
  done: [message: string];
}>();

const action = ref<"grant" | "revoke">("grant");
const mysqlScope = ref<"all" | "database" | "table">("database");
const postgresScope = ref<"database" | "schema" | "table" | "all-tables">("schema");
const privileges = reactive<Set<string>>(new Set(["SELECT"]));
const database = ref("");
const schema = ref("");
const table = ref("");
const running = ref(false);
const error = ref("");

const mysqlPrivileges = [
  "SELECT",
  "INSERT",
  "UPDATE",
  "DELETE",
  "CREATE",
  "DROP",
  "ALTER",
  "INDEX",
  "EXECUTE",
  "ALL PRIVILEGES",
];
const postgresPrivileges = [
  "SELECT",
  "INSERT",
  "UPDATE",
  "DELETE",
  "TRUNCATE",
  "REFERENCES",
  "TRIGGER",
  "USAGE",
  "CREATE",
  "CONNECT",
  "EXECUTE",
  "ALL PRIVILEGES",
];
const availablePrivileges = computed(() =>
  props.driver === "mysql" ? mysqlPrivileges : postgresPrivileges,
);

const mysqlName = (name: string) => `\`${name.replaceAll("`", "``")}\``;

const target = computed(() => {
  if (!props.user) return "";
  if (props.driver === "mysql") {
    return `'${props.user.name.replaceAll("'", "''")}'@'${props.user.host.replaceAll("'", "''")}'`;
  }
  return quoteIdentifier(props.user.name, "postgres");
});

const scopeSql = computed(() => {
  if (props.driver === "mysql") {
    if (mysqlScope.value === "all") return "*.*";
    if (mysqlScope.value === "database") return `${mysqlName(database.value)}.*`;
    return `${mysqlName(database.value)}.${mysqlName(table.value)}`;
  }
  if (postgresScope.value === "database") {
    return `DATABASE ${quoteIdentifier(database.value, "postgres")}`;
  }
  if (postgresScope.value === "schema") {
    return `SCHEMA ${quoteIdentifier(schema.value, "postgres")}`;
  }
  if (postgresScope.value === "table") {
    return `TABLE ${quoteIdentifier(schema.value, "postgres")}.${quoteIdentifier(table.value, "postgres")}`;
  }
  return `ALL TABLES IN SCHEMA ${quoteIdentifier(schema.value, "postgres")}`;
});

const sql = computed(() => {
  const list = [...privileges].join(", ");
  if (!props.user || !list) return "";
  if (action.value === "grant") {
    return `GRANT ${list} ON ${scopeSql.value} TO ${target.value}`;
  }
  return `REVOKE ${list} ON ${scopeSql.value} FROM ${target.value}`;
});

const validationError = computed(() => {
  if (props.readOnly) return "只读连接不能执行授权 / 回收";
  if (!props.user) return "请选择用户 / 角色";
  if (!privileges.size) return "至少选择一个权限";
  if (props.driver === "mysql" && mysqlScope.value !== "all" && !database.value.trim()) {
    return "数据库名不能为空";
  }
  if (props.driver === "postgres" && postgresScope.value === "database" && !database.value.trim()) {
    return "数据库名不能为空";
  }
  if (props.driver === "postgres" && ["schema", "table", "all-tables"].includes(postgresScope.value) && !schema.value.trim()) {
    return "模式名不能为空";
  }
  if ((props.driver === "mysql" && mysqlScope.value === "table") || (props.driver === "postgres" && postgresScope.value === "table")) {
    if (!table.value.trim()) return "表名不能为空";
  }
  return null;
});

function togglePrivilege(privilege: string) {
  if (privileges.has(privilege)) privileges.delete(privilege);
  else privileges.add(privilege);
}

async function execute() {
  if (running.value || validationError.value) return;
  if (action.value === "revoke") {
    const confirmed = window.confirm(
      `确定回收「${props.user?.name}」的 ${[...privileges].join(", ")} 权限？`,
    );
    if (!confirmed) return;
  }
  running.value = true;
  error.value = "";
  try {
    await api.executeQuery(props.connectionId, sql.value, 1000);
    emit("done", action.value === "grant" ? "已执行授权" : "已执行权限回收");
    emit("close");
  } catch (reason) {
    error.value = String(reason);
  } finally {
    running.value = false;
  }
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (!open) {
      document.removeEventListener("keydown", onKeydown, true);
      return;
    }
    action.value = props.initialAction ?? "grant";
    mysqlScope.value = "database";
    postgresScope.value = "schema";
    privileges.clear();
    privileges.add("SELECT");
    database.value = props.database ?? "";
    schema.value = props.schema ?? "";
    table.value = "";
    error.value = "";
    document.addEventListener("keydown", onKeydown, true);
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal server-grant-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>{{ action === "grant" ? "授权" : "回收权限" }}</h2>
          <p v-if="user">
            {{ driver === "mysql" ? `${user.name}@${user.host}` : user.name }}
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="server-grant-body">
        <div class="segmented compact-segmented">
          <button class="segment" :class="{ active: action === 'grant' }" @click="action = 'grant'">
            授权
          </button>
          <button class="segment" :class="{ active: action === 'revoke' }" @click="action = 'revoke'">
            回收
          </button>
        </div>

        <div class="form-grid">
          <label v-if="driver === 'mysql'">
            范围
            <select v-model="mysqlScope">
              <option value="all">全局 *.*</option>
              <option value="database">当前数据库.*</option>
              <option value="table">指定表</option>
            </select>
          </label>
          <label v-else>
            范围
            <select v-model="postgresScope">
              <option value="database">DATABASE</option>
              <option value="schema">SCHEMA</option>
              <option value="table">TABLE</option>
              <option value="all-tables">ALL TABLES IN SCHEMA</option>
            </select>
          </label>
          <label v-if="(driver === 'mysql' && mysqlScope !== 'all') || (driver === 'postgres' && postgresScope === 'database')">
            数据库
            <input v-model="database" placeholder="当前数据库" />
          </label>
          <label v-if="driver === 'postgres' && ['schema', 'table', 'all-tables'].includes(postgresScope)">
            模式
            <input v-model="schema" placeholder="public" />
          </label>
          <label v-if="(driver === 'mysql' && mysqlScope === 'table') || (driver === 'postgres' && postgresScope === 'table')">
            表
            <input v-model="table" placeholder="表名" />
          </label>
        </div>

        <div class="server-grant-privileges">
          <span>权限</span>
          <label v-for="privilege in availablePrivileges" :key="privilege">
            <input
              type="checkbox"
              :checked="privileges.has(privilege)"
              @change="togglePrivilege(privilege)"
            />
            {{ privilege }}
          </label>
        </div>

        <div class="server-grant-preview">
          <span>将执行</span>
          <pre>{{ sql || "—" }}</pre>
          <p class="hint">授权 / 回收会立即执行并写入 SQL 历史；需要相应管理员权限。</p>
        </div>
        <p v-if="validationError || error" class="data-message warning">
          {{ validationError || error }}
        </p>
      </div>

      <footer class="modal-actions">
        <button class="button ghost" @click="emit('close')">取消</button>
        <button class="button primary" :disabled="Boolean(validationError) || running" @click="execute">
          {{ running ? "执行中…" : action === "grant" ? "执行授权" : "执行回收" }}
        </button>
      </footer>
    </div>
  </div>
</template>
