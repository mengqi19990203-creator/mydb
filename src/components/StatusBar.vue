<script setup lang="ts">
import { computed } from "vue";
import type { ConnectionConfig, QueryResult } from "../lib/types";

const props = defineProps<{
  status: string;
  connection: ConnectionConfig | null;
  result: QueryResult | null;
  database?: string | null;
  schema?: string | null;
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
  const user = connection.username ? `${connection.username}@` : "";
  return `${user}${connection.host}:${connection.port}`;
}
</script>

<template>
  <footer class="status-bar">
    <span class="status-message" role="status" aria-live="polite">
      {{ status }}
    </span>
    <span class="status-meta">
      <span v-if="connection">
        {{ connection.driver }} · {{ connectionTarget(connection) }}
      </span>
      <span v-if="scope" class="status-scope" :title="scope">{{ scope }}</span>
      <span v-if="result && result.rows.length">{{ result.rows.length }} 行</span>
      <span v-if="result">{{ result.elapsedMs }} ms</span>
    </span>
  </footer>
</template>