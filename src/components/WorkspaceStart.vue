<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import type {
  ConnectionConfig,
  HistoryEntry,
  TableInfo,
} from "../lib/types";

type ConnectionStatus = "idle" | "connecting" | "connected" | "failed";

const props = defineProps<{
  connectionsCount: number;
  selected: ConnectionConfig | null;
  target: string;
  scope: string;
  connectionStatus: ConnectionStatus;
  connectionStatusLabel: string;
  tables: TableInfo[];
  tablesLoading: boolean;
  history: HistoryEntry[];
  historyLoading: boolean;
  commandKey: string;
  redis?: boolean;
}>();

const emit = defineEmits<{
  "new-connection": [];
  "new-query": [];
  "select-history": [entry: HistoryEntry];
  "open-table": [table: TableInfo];
  "refresh-tables": [];
  "refresh-history": [];
  "open-redis": [];
}>();

const KIND_LABELS: Record<string, string> = {
  table: "表",
  view: "视图",
  matview: "物化视图",
};

const viewableObjects = computed(() =>
  props.tables.filter(
    (table) =>
      table.kind === "table" ||
      table.kind === "view" ||
      table.kind === "matview",
  ),
);

const visibleObjects = computed(() => viewableObjects.value.slice(0, 7));

const objectTotal = computed(() => viewableObjects.value.length);

const recentEntries = computed(() => props.history.slice(0, 6));

function objectIcon(kind: string): "table" | "view" {
  return kind === "table" ? "table" : "view";
}

function objectKindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? "对象";
}

function historyPreview(sql: string): string {
  const compact = sql.replace(/\s+/g, " ").trim();
  return compact.length > 150 ? `${compact.slice(0, 150)}…` : compact;
}

function formatHistoryTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}
</script>

<template>
  <section
    class="start-page"
    :class="{ 'start-page-welcome': connectionsCount === 0 }"
    :aria-labelledby="connectionsCount === 0 ? 'welcome-title' : 'start-title'"
  >
    <div class="start-canvas">
      <header class="start-head">
        <div class="start-kicker-row">
          <span class="start-kicker">
            {{ connectionsCount === 0 ? "欢迎" : "主工作区" }}
          </span>
          <span class="start-meta">
            {{
              connectionsCount === 0
                ? "0 个连接"
                : redis
                  ? "Redis"
                  : "0 个标签"
            }}
          </span>
        </div>

        <template v-if="connectionsCount === 0">
          <h2 id="welcome-title">连接你的第一个数据库</h2>
          <p class="start-desc">
            选择驱动并填写连接信息。配置与密码只保存在本机，随时可以编辑。
          </p>
          <div class="start-actions">
            <button
              class="start-command primary"
              @click="emit('new-connection')"
            >
              <span class="start-command-icon">
                <Icon name="plus" :size="15" />
              </span>
              <span>新建连接</span>
              <kbd class="start-kbd">{{ commandKey }}N</kbd>
            </button>
          </div>
        </template>

        <template v-else-if="redis">
          <h2 id="start-title">Redis 控制台已关闭</h2>
          <p class="start-desc">
            重新打开控制台，浏览键、切换逻辑库或执行 Redis 命令。
          </p>
          <div class="start-actions">
            <button
              class="start-command primary"
              @click="emit('open-redis')"
            >
              <span class="start-command-icon">
                <Icon name="database" :size="15" />
              </span>
              <span>打开 Redis 控制台</span>
            </button>
            <button class="start-command" @click="emit('new-connection')">
              <span class="start-command-icon">
                <Icon name="plus" :size="15" />
              </span>
              <span>新建连接</span>
              <kbd class="start-kbd">{{ commandKey }}N</kbd>
            </button>
          </div>
        </template>

        <template v-else>
          <h2 id="start-title">从一条查询开始</h2>
          <p class="start-desc">
            <template v-if="selected">
              当前连接
              <span class="start-inline-name">{{ selected.name }}</span>。
              新建查询继续，或从左侧对象树打开表。
            </template>
            <template v-else>
              新建查询开始工作，或从左侧对象树打开表。
            </template>
          </p>
          <div class="start-actions">
            <button class="start-command primary" @click="emit('new-query')">
              <span class="start-command-icon">
                <Icon name="code" :size="15" />
              </span>
              <span>新建查询</span>
              <kbd class="start-kbd">{{ commandKey }}T</kbd>
            </button>
            <button class="start-command" @click="emit('new-connection')">
              <span class="start-command-icon">
                <Icon name="plus" :size="15" />
              </span>
              <span>新建连接</span>
              <kbd class="start-kbd">{{ commandKey }}N</kbd>
            </button>
          </div>
        </template>
      </header>

      <div v-if="connectionsCount === 0" class="start-notes">
        <div class="start-note">
          <span class="start-note-label">本地优先</span>
          <span class="start-note-value">
            连接配置保存在本机，密码可选择写入系统钥匙串。
          </span>
        </div>
        <div class="start-note">
          <span class="start-note-label">远程访问</span>
          <span class="start-note-value">
            MySQL / PostgreSQL 支持 SSL 与 SSH 隧道。
          </span>
        </div>
        <div class="start-note">
          <span class="start-note-label">支持驱动</span>
          <span class="start-note-value">
            SQLite、MySQL / MariaDB、PostgreSQL 与 Redis。
          </span>
        </div>
      </div>

      <div v-else class="start-grid">
        <section
          v-if="redis"
          class="start-recents"
          aria-labelledby="start-redis-title"
        >
          <header class="start-section-head">
            <h3 id="start-redis-title" class="start-section-title">
              Redis 控制台
            </h3>
            <span class="start-section-count">{{ scope || "db00" }}</span>
          </header>
          <div class="start-empty">
            会话已关闭。重新打开后可扫描键、查看类型详情，或执行任意 Redis 命令。
          </div>
          <div class="start-notes start-redis-notes">
            <div class="start-note">
              <span class="start-note-label">键浏览</span>
              <span class="start-note-value">
                SCAN、类型详情和删除键。
              </span>
            </div>
            <div class="start-note">
              <span class="start-note-label">命令控制台</span>
              <span class="start-note-value">
                直接执行任意 Redis 命令。
              </span>
            </div>
            <div class="start-note">
              <span class="start-note-label">逻辑库</span>
              <span class="start-note-value">
                切换 db，重连后保持选择。
              </span>
            </div>
          </div>
        </section>
        <section
          v-else
          class="start-recents"
          aria-labelledby="start-recent-title"
        >
          <header class="start-section-head">
            <h3 id="start-recent-title" class="start-section-title">
              最近查询
            </h3>
            <span class="start-section-count">{{ history.length }}</span>
            <button
              class="start-section-action"
              title="刷新最近查询"
              aria-label="刷新最近查询"
              :disabled="historyLoading"
              @click="emit('refresh-history')"
            >
              <Icon name="refresh" :size="13" />
            </button>
          </header>

          <div v-if="historyLoading && !history.length" class="start-empty">
            正在读取最近查询…
          </div>
          <div v-else-if="!recentEntries.length" class="start-empty">
            还没有执行记录。运行第一条查询后，最近 SQL 会出现在这里。
          </div>
          <div v-else class="start-recent-list">
            <button
              v-for="entry in recentEntries"
              :key="entry.id"
              class="start-recent-row"
              :title="entry.sql"
              @click="emit('select-history', entry)"
            >
              <span
                class="start-recent-mark"
                :class="{ failed: !entry.success }"
                aria-hidden="true"
              ></span>
              <span class="start-recent-body">
                <code class="start-recent-sql">{{
                  historyPreview(entry.sql)
                }}</code>
                <span class="start-recent-meta">
                  <span class="start-recent-conn">
                    {{ entry.connectionName || "未知连接" }}
                  </span>
                  <span>{{ formatHistoryTime(entry.executedAt) }}</span>
                  <span v-if="entry.success">{{ entry.elapsedMs }} ms</span>
                  <span v-else class="start-recent-failed">失败</span>
                </span>
              </span>
              <Icon
                class="start-recent-go"
                name="chevron-right"
                :size="13"
              />
            </button>
          </div>
          <div
            v-if="history.length > recentEntries.length"
            class="start-footnote"
          >
            共 {{ history.length }} 条执行记录，这里显示最近
            {{ recentEntries.length }} 条。
          </div>
        </section>

        <aside class="start-rail">
          <section aria-labelledby="start-connection-title">
            <header class="start-section-head">
              <h3 id="start-connection-title" class="start-section-title">
                当前连接
              </h3>
              <span v-if="selected" class="start-section-count">
                {{ selected.driver }}
              </span>
            </header>

            <div v-if="selected" class="start-connection">
              <div class="start-connection-head">
                <span
                  class="start-connection-color"
                  :style="{ background: selected.color }"
                  aria-hidden="true"
                ></span>
                <span
                  class="start-connection-name"
                  :title="selected.name"
                >
                  {{ selected.name }}
                </span>
                <span v-if="selected.readOnly" class="readonly-badge">
                  只读
                </span>
                <span class="start-connection-state">
                  <span
                    class="status-dot"
                    :class="connectionStatus"
                  ></span>
                  {{ connectionStatusLabel }}
                </span>
              </div>
              <span
                class="start-connection-target"
                :title="target"
              >
                {{ target }}
              </span>
              <span v-if="scope" class="start-connection-scope">
                当前库
                <span>{{ scope }}</span>
              </span>
            </div>
            <div v-else class="start-empty">
              尚未选择连接。从左侧连接列表选择一个开始。
            </div>
          </section>

          <section
            v-if="selected && !redis"
            class="start-objects"
            aria-labelledby="start-objects-title"
          >
            <header class="start-section-head">
              <h3 id="start-objects-title" class="start-section-title">
                当前库对象
              </h3>
              <span class="start-section-count">{{ objectTotal }}</span>
              <button
                class="start-section-action"
                title="刷新对象"
                aria-label="刷新对象"
                :disabled="tablesLoading"
                @click="emit('refresh-tables')"
              >
                <Icon name="refresh" :size="13" />
              </button>
            </header>

            <div
              v-if="tablesLoading && !visibleObjects.length"
              class="start-empty"
            >
              正在读取对象…
            </div>
            <div v-else-if="!visibleObjects.length" class="start-empty">
              当前库还没有表或视图。
            </div>
            <div v-else class="start-object-list">
              <button
                v-for="table in visibleObjects"
                :key="`${table.kind}:${table.name}`"
                class="start-object-row"
                :title="`打开 ${table.name}`"
                @click="emit('open-table', table)"
              >
                <Icon :name="objectIcon(table.kind)" :size="13" />
                <span class="start-object-name">{{ table.name }}</span>
                <span class="start-object-kind">
                  {{ objectKindLabel(table.kind) }}
                </span>
              </button>
            </div>
            <div
              v-if="objectTotal > visibleObjects.length"
              class="start-footnote"
            >
              共 {{ objectTotal }} 个对象，完整列表在左侧对象树。
            </div>
          </section>
        </aside>
      </div>
    </div>
  </section>
</template>