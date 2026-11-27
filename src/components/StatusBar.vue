<script setup lang="ts">
import { computed } from "vue";
import type { ConnectionConfig, QueryResult } from "../lib/types";

type ConnectionStatus =
  | "idle"
  | "connecting"
  | "connected"
  | "failed"
  | "lost";

const props = defineProps<{
  status: string;
  connection: ConnectionConfig | null;
  result: QueryResult | null;
  database?: string | null;
  schema?: string | null;
  activeLabel?: string;
  activeKind?: "query" | "table" | "er" | null;
  connectionStatus?: ConnectionStatus;
  connectionStatusLabel?: string;
  readOnly?: boolean;
  inTransaction?: boolean;
  running?: boolean;
  error?: string | null;
}>();

function redisDatabaseLabel(value: string): string {
  const database = value.trim() || "0";
  return /^\d+$/.test(database)
    ? `db${database.padStart(2, "0")}`
    : `db${database}`;
}

const scope = computed(() => {
  const connection = props.connection;
  if (!connection) return "";
  if (connection.driver === "sqlite") return "main";
  if (connection.driver === "redis") {
    return redisDatabaseLabel(props.database ?? connection.database);
  }
  if (connection.driver === "mongodb") {
    return (props.database ?? connection.database ?? "").trim();
  }
  const database = (props.database ?? connection.database ?? "").trim();
  if (connection.driver === "postgres" && props.schema) {
    return `${database} · ${props.schema}`;
  }
  return database;
});

function connectionTarget(connection: ConnectionConfig): string {
  if (connection.driver === "sqlite") return connection.file;
  if (connection.driver === "redis") {
    return `${connection.host}:${connection.port}`;
  }
  if (connection.driver === "mongodb" && connection.mongoUriMode) {
    const uri = connection.mongoUri ?? "";
    const rest = uri.includes("://") ? uri.split("://")[1] ?? "" : uri;
    const authority = rest.split(/[/?#]/)[0] ?? "";
    return authority.includes("@")
      ? authority.slice(authority.lastIndexOf("@") + 1)
      : authority;
  }
  const user = connection.username ? `${connection.username}@` : "";
  return `${user}${connection.host}:${connection.port}`;
}

const activeLabel = computed(() => {
  if (props.activeLabel) return props.activeLabel;
  if (!props.connection) return "未连接";
  return scope.value
    ? `${props.connection.name} · ${scope.value}`
    : props.connection.name;
});

const resultSummary = computed(() => {
  if (props.running) return "";
  if (props.error) return "查询失败";
  const result = props.result;
  if (!result) return "";
  const elapsed = `${result.elapsedMs} ms`;
  if (result.isQuery) {
    return `${result.rows.length} 行 · ${elapsed}${
      result.truncated ? " · 已截断" : ""
    }`;
  }
  return `影响 ${result.rowsAffected} 行 · ${elapsed}`;
});

const stateLabel = computed(() => {
  if (!props.connection) return "未连接";
  return props.connectionStatusLabel ?? props.connectionStatus ?? "idle";
});
</script>

<template>
  <footer class="status-bar">
    <span class="status-message" role="status" aria-live="polite">
      {{ status }}
    </span>
    <span class="status-meta">
      <span
        v-if="connection"
        class="status-active"
        :class="{ 'status-active-table': activeKind === 'table' }"
        :title="activeLabel"
      >
        {{ activeLabel }}
      </span>
      <span v-else class="status-active">未连接</span>
      <span v-if="readOnly" class="status-readonly">只读</span>
      <span
        v-if="connection"
        class="status-connection"
        :title="`${connection.driver} · ${connectionTarget(connection)}`"
      >
        <span class="status-dot" :class="connectionStatus ?? 'idle'"></span>
        {{ stateLabel }}
      </span>
      <span v-if="inTransaction" class="status-transaction">
        事务中 · 未提交
      </span>
      <span v-if="running" class="status-running">
        <span class="spinner" aria-hidden="true"></span>
        执行中…
      </span>
      <span
        v-else-if="resultSummary"
        class="status-result"
        :class="{ 'status-result-error': !!error }"
        :title="error || resultSummary"
      >
        {{ resultSummary }}
      </span>
    </span>
  </footer>
</template>