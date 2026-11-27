<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { MongoRoleInfo, MongoUserInfo } from "../lib/types";

const props = defineProps<{
  open: boolean;
  connectionId: string;
  database: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  status: [message: string];
}>();

const users = ref<MongoUserInfo[]>([]);
const roles = ref<MongoRoleInfo[]>([]);
const loading = ref(false);
const error = ref("");
const editing = ref<MongoUserInfo | null>(null);
const username = ref("");
const password = ref("");
const roleText = ref("read");
const saving = ref(false);
const deleteTarget = ref<MongoUserInfo | null>(null);
const showRoles = ref(false);

const canWrite = computed(() => !props.readOnly);
const roleSuggestions = computed(() =>
  roles.value
    .map((role) => `${role.name}@${role.database}`)
    .slice(0, 100)
    .join(", "),
);

function resetForm() {
  editing.value = null;
  username.value = "";
  password.value = "";
  roleText.value = "read";
  error.value = "";
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    const [userList, roleList] = await Promise.all([
      api.mongoListUsers(props.connectionId, props.database),
      api.mongoListRoles(props.connectionId, props.database).catch(() => []),
    ]);
    users.value = userList;
    roles.value = roleList;
  } catch (reason) {
    users.value = [];
    roles.value = [];
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function beginCreate() {
  resetForm();
  editing.value = { username: "", database: props.database, roles: [] };
}

function beginEdit(user: MongoUserInfo) {
  editing.value = user;
  username.value = user.username;
  password.value = "";
  roleText.value = user.roles.join(", ");
  error.value = "";
}

async function save() {
  if (!canWrite.value || saving.value) return;
  const name = username.value.trim();
  if (!name) {
    error.value = "用户名不能为空";
    return;
  }
  const roleList = roleText.value
    .split(",")
    .map((role) => role.trim())
    .filter(Boolean);
  if (!roleList.length) {
    error.value = "至少选择一个角色";
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    if (editing.value?.username) {
      await api.mongoUpdateUserRoles(
        props.connectionId,
        props.database,
        editing.value.username,
        roleList,
      );
      emit("status", `已更新用户 ${editing.value.username} 的角色`);
    } else {
      if (!password.value) {
        error.value = "新用户密码不能为空";
        return;
      }
      await api.mongoCreateUser(
        props.connectionId,
        props.database,
        name,
        password.value,
        roleList,
      );
      emit("status", `已创建 MongoDB 用户 ${name}`);
    }
    resetForm();
    await load();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}

async function confirmDelete() {
  const user = deleteTarget.value;
  deleteTarget.value = null;
  if (!user || !canWrite.value) return;
  try {
    await api.mongoDropUser(props.connectionId, props.database, user.username);
    emit("status", `已删除 MongoDB 用户 ${user.username}`);
    if (editing.value?.username === user.username) resetForm();
    await load();
  } catch (reason) {
    error.value = String(reason);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("close");
}

watch(
  () => [props.open, props.connectionId, props.database],
  ([open]) => {
    if (open) {
      resetForm();
      showRoles.value = false;
      document.addEventListener("keydown", onKeydown, true);
      void load();
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal mongo-users-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>MongoDB 用户与角色</h2>
          <p>{{ database }} · 当前数据库范围内的用户</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="mongo-users-body">
        <p class="hint">
          不显示密码散列；创建 / 修改 / 删除需要 userAdmin 权限。只读连接只能查看。
        </p>
        <p v-if="error" class="data-message warning">{{ error }}</p>
        <div v-if="loading" class="mongo-gridfs-empty">读取用户…</div>
        <div v-else-if="!users.length" class="mongo-gridfs-empty">
          <Icon name="list" :size="22" />
          <strong>当前数据库没有用户</strong>
          <span v-if="canWrite">点击下方「新建用户」创建。</span>
        </div>
        <table v-else class="mongo-users-table">
          <thead><tr><th>用户</th><th>角色</th><th></th></tr></thead>
          <tbody>
            <tr v-for="user in users" :key="user.username">
              <td><code>{{ user.username }}</code></td>
              <td class="mongo-user-roles">
                <span v-for="role in user.roles" :key="role" class="mongo-role-chip">
                  {{ role }}
                </span>
                <span v-if="!user.roles.length" class="hint">无角色</span>
              </td>
              <td class="mongo-gridfs-actions">
                <button
                  class="button ghost compact"
                  :disabled="!canWrite"
                  @click="beginEdit(user)"
                >
                  编辑角色
                </button>
                <button
                  class="button ghost compact danger-text"
                  :disabled="!canWrite"
                  @click="deleteTarget = user"
                >
                  删除
                </button>
              </td>
            </tr>
          </tbody>
        </table>

        <div v-if="canWrite" class="mongo-user-form">
          <h3>{{ editing?.username ? `编辑用户 ${editing.username}` : "新建用户" }}</h3>
          <div class="mongo-manage-form">
            <label>
              用户名
              <input
                v-model="username"
                :disabled="Boolean(editing?.username)"
                placeholder="例如：report_reader"
                autocomplete="off"
              />
            </label>
            <label v-if="!editing?.username">
              密码
              <input
                v-model="password"
                type="password"
                placeholder="新用户密码"
                autocomplete="new-password"
              />
            </label>
            <label class="span-2">
              角色（逗号分隔，支持 role@db）
              <input v-model="roleText" placeholder="read, readWrite@otherdb" />
            </label>
            <p class="hint span-2">
              常用角色：read、readWrite、dbAdmin、userAdmin、clusterMonitor、root。
              留空会阻止保存。
            </p>
            <p v-if="roleSuggestions" class="hint span-2">
              当前数据库可用角色：{{ roleSuggestions }}…
            </p>
          </div>
          <div class="mongo-user-form-actions">
            <button v-if="editing?.username" class="button ghost" @click="resetForm">
              取消编辑
            </button>
            <button class="button ghost" @click="beginCreate">新建用户</button>
            <button class="button primary" :disabled="saving" @click="save">
              {{ saving ? "保存中…" : editing?.username ? "保存角色" : "创建用户" }}
            </button>
          </div>
        </div>
        <div v-else class="hint">只读连接不能创建或修改用户。</div>

        <button class="button ghost compact" @click="showRoles = !showRoles">
          {{ showRoles ? "收起角色列表" : `查看角色列表（${roles.length}）` }}
        </button>
        <div v-if="showRoles" class="mongo-role-list">
          <span v-for="role in roles" :key="`${role.database}.${role.name}`" class="mongo-role-chip">
            {{ role.name }}@{{ role.database }}{{ role.builtin ? "" : " · 自定义" }}
          </span>
        </div>
      </div>

      <footer class="modal-actions">
        <button class="button ghost" :disabled="loading" @click="load">刷新</button>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>

      <ConfirmDialog
        :open="Boolean(deleteTarget)"
        title="删除 MongoDB 用户"
        :message="`确定删除用户「${deleteTarget?.username ?? ''}」？`"
        detail="该用户的授权会一并删除，此操作不可撤销。"
        confirm-text="删除用户"
        danger
        @confirm="confirmDelete"
        @cancel="deleteTarget = null"
      />
    </div>
  </div>
</template>
