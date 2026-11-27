<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import ServerGrantDialog from "./ServerGrantDialog.vue";
import * as api from "../lib/api";
import type { Driver, ServerUser, ServerUserDetail } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  driver: Driver;
  active: boolean;
  readOnly?: boolean;
}>();

const users = ref<ServerUser[]>([]);
const detail = ref<ServerUserDetail | null>(null);
const selectedKey = ref("");
const search = ref("");
const loading = ref(false);
const detailLoading = ref(false);
const error = ref("");
const detailError = ref("");
const currentDatabase = ref("");
const currentSchema = ref("");
const grantOpen = ref(false);
const grantAction = ref<"grant" | "revoke">("grant");

const selectedUser = computed(
  () => users.value.find((user) => keyOf(user) === selectedKey.value) ?? null,
);

const filtered = computed(() => {
  const term = search.value.trim().toLowerCase();
  if (!term) return users.value;
  return users.value.filter((user) =>
    `${user.name}@${user.host}`.toLowerCase().includes(term),
  );
});

function keyOf(user: ServerUser): string {
  return `${user.name}\u0000${user.host}`;
}

function attribute(user: ServerUser, label: string): string {
  return user.attributes.find((item) => item.label === label)?.value ?? "";
}

const summaryColumns = computed(() =>
  props.driver === "mysql"
    ? ["插件", "账号锁定", "密码过期"]
    : ["可登录", "超级用户", "当前库 CONNECT"],
);

async function refresh() {
  if (!props.active) return;
  loading.value = true;
  error.value = "";
  detailError.value = "";
  try {
    const [list, context] = await Promise.all([
      api.serverUsers(props.connectionId),
      api.getConnectionContext(props.connectionId).catch(() => null),
    ]);
    users.value = list;
    currentDatabase.value = context?.database ?? "";
    currentSchema.value = context?.schema ?? "";
    if (!list.length) {
      selectedKey.value = "";
      detail.value = null;
      return;
    }
    const still = list.find((user) => keyOf(user) === selectedKey.value);
    const next = still ?? list[0];
    selectedKey.value = keyOf(next);
    await loadDetail(next);
  } catch (reason) {
    users.value = [];
    detail.value = null;
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function loadDetail(user: ServerUser) {
  detailLoading.value = true;
  detailError.value = "";
  try {
    detail.value = await api.serverUserGrants(
      props.connectionId,
      user.name,
      user.host,
    );
  } catch (reason) {
    detail.value = null;
    detailError.value = String(reason);
  } finally {
    detailLoading.value = false;
  }
}

function select(user: ServerUser) {
  if (selectedKey.value === keyOf(user)) return;
  selectedKey.value = keyOf(user);
  void loadDetail(user);
}

function openGrant(action: "grant" | "revoke") {
  grantAction.value = action;
  grantOpen.value = true;
}

function afterGrant(message: string) {
  error.value = "";
  detailError.value = "";
  void refresh().then(() => {
    if (selectedUser.value) void loadDetail(selectedUser.value);
  });
  statusMessage.value = message;
}

const statusMessage = ref("");

watch(
  () => [props.active, props.connectionId] as const,
  ([active]) => {
    if (!active) return;
    void refresh();
  },
  { immediate: true },
);

defineExpose({ refresh });
</script>

<template>
  <section class="server-users">
    <p class="server-users-note">
      只读查看；授权与回收请使用 SQL 或专用工具。不显示密码散列、私钥或令牌。
    </p>

    <div class="server-users-toolbar">
      <div class="server-users-search">
        <Icon name="search" :size="14" />
        <input
          v-model="search"
          placeholder="搜索用户 / 角色"
          spellcheck="false"
        />
      </div>
      <span class="hint">
        {{ filtered.length }} / {{ users.length }}
        <template v-if="users.length >= 500"> · 已限制为前 500 条</template>
      </span>
      <button
        class="button ghost compact"
        :disabled="loading"
        @click="refresh"
      >
        <Icon name="refresh" :size="14" />
        {{ loading ? "读取中…" : "刷新" }}
      </button>
    </div>

    <p v-if="error" class="data-message error">{{ error }}</p>
    <div v-else-if="loading && !users.length" class="panel-empty">
      正在读取用户 / 角色…
    </div>
    <div v-else-if="!users.length" class="panel-empty">
      没有可显示的用户或角色。
    </div>

    <div v-else class="server-users-layout">
      <div class="server-users-list">
        <table class="structure-table">
          <thead>
            <tr>
              <th>{{ driver === "mysql" ? "用户" : "角色" }}</th>
              <th v-if="driver === 'mysql'">Host</th>
              <th
                v-for="column in summaryColumns"
                :key="column"
                class="server-user-flag"
              >
                {{ column }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="user in filtered"
              :key="`${user.name}:${user.host}`"
              class="server-user-row"
              :class="{ selected: keyOf(user) === selectedKey }"
              tabindex="0"
              @click="select(user)"
              @keydown.enter.prevent="select(user)"
            >
              <td class="mono">{{ user.name }}</td>
              <td v-if="driver === 'mysql'" class="mono">{{ user.host || "—" }}</td>
              <td
                v-for="column in summaryColumns"
                :key="column"
                class="server-user-flag"
              >
                {{ attribute(user, column) || "—" }}
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="!filtered.length" class="panel-empty compact">
          没有匹配的用户 / 角色。
        </div>
      </div>

      <aside class="server-user-detail">
        <div v-if="detailLoading" class="panel-empty compact">读取详情…</div>
        <p v-else-if="detailError" class="data-message error">
          {{ detailError }}
        </p>
        <template v-else-if="detail">
          <h3>
            {{ detail.name }}
            <span v-if="detail.host" class="server-user-host">@{{ detail.host }}</span>
          </h3>
          <div v-if="!readOnly && selectedUser" class="server-user-grant-actions">
            <button class="button ghost compact" @click="openGrant('grant')">授权…</button>
            <button class="button ghost compact danger-text" @click="openGrant('revoke')">回收权限…</button>
          </div>
          <p v-if="statusMessage" class="hint">{{ statusMessage }}</p>
          <p v-for="note in detail.notes" :key="note" class="hint">
            {{ note }}
          </p>

          <template v-if="detail.grants.length">
            <h4>授权</h4>
            <pre class="server-user-grants">{{ detail.grants.join("\n") }}</pre>
          </template>

          <template v-if="detail.memberships.length">
            <h4>角色成员关系</h4>
            <div class="server-user-chips">
              <span v-for="role in detail.memberships" :key="role">
                {{ role }}
              </span>
            </div>
          </template>

          <template v-if="detail.privileges.length">
            <h4>当前数据库权限</h4>
            <div class="server-user-privileges">
              <div
                v-for="privilege in detail.privileges"
                :key="privilege.label"
                class="server-user-privilege"
              >
                <span>{{ privilege.label }}</span>
                <strong>{{ privilege.value }}</strong>
              </div>
            </div>
          </template>

          <p
            v-if="
              !detail.grants.length &&
              !detail.memberships.length &&
              !detail.privileges.length
            "
            class="hint"
          >
            没有可显示的授权或成员关系。
          </p>
        </template>
        <div v-else class="panel-empty compact">选择一个用户查看详情。</div>
      </aside>
    </div>

    <ServerGrantDialog
      :open="grantOpen"
      :connection-id="connectionId"
      :driver="driver"
      :user="selectedUser"
      :database="currentDatabase"
      :schema="currentSchema"
      :read-only="readOnly"
      :initial-action="grantAction"
      @close="grantOpen = false"
      @done="afterGrant"
    />
  </section>
</template>

<style scoped>
.server-users {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.server-users-note {
  margin: 0;
  padding: 6px 9px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--text-secondary);
  font-size: 11px;
}

.server-users-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.server-users-search {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  max-width: 320px;
  padding: 5px 9px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--surface-well);
}

.server-users-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  outline: none;
  font-size: 12px;
}

.server-users-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(280px, 0.85fr);
  gap: 12px;
  min-height: 0;
}

.server-users-list {
  max-height: 380px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
}

.server-users-list .structure-table th {
  position: sticky;
  top: 0;
  z-index: 1;
}

.server-user-row {
  cursor: pointer;
}

.server-user-row.selected,
.server-user-row:hover {
  background: var(--bg-hover);
}

.server-user-flag {
  white-space: nowrap;
}

.server-user-detail {
  max-height: 380px;
  overflow: auto;
  padding: 10px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.server-user-detail h3 {
  margin: 0;
  font-size: 13px;
  font-family: var(--font-mono);
}

.server-user-detail h4 {
  margin: 12px 0 6px;
  color: var(--muted);
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.server-user-host {
  color: var(--faint);
  font-weight: 400;
}

.server-user-grants {
  margin: 0;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--surface-well);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.6;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.server-user-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.server-user-chips span {
  padding: 2px 6px;
  border-radius: 999px;
  background: var(--panel-3);
  color: var(--text-secondary);
  font-size: 10px;
}

.server-user-privileges {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.server-user-privilege {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 7px 9px;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
}

.server-user-privilege span {
  color: var(--muted);
  font-size: 9px;
}

.server-user-privilege strong {
  font-size: 12px;
}
</style>