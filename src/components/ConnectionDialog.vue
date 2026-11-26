<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { ConnectionConfig, Driver, SshAuth, SslMode } from "../lib/types";

const props = defineProps<{
  open: boolean;
  editing: ConnectionConfig | null;
  saveError?: string | null;
  defaultGroup?: string;
}>();

const emit = defineEmits<{
  close: [];
  save: [
    payload: {
      config: ConnectionConfig;
      password?: string;
      sshPassword?: string;
      sshKeyPassphrase?: string;
    },
  ];
}>();

const palette = ["#58a6ff", "#3fb950", "#d29922", "#f85149", "#bc8cff", "#39c5cf"];

function defaultPort(driver: Driver): number {
  if (driver === "mysql") return 3306;
  if (driver === "postgres") return 5432;
  if (driver === "redis") return 6379;
  if (driver === "mongodb") return 27017;
  return 0;
}

const form = reactive<{
  id: string;
  name: string;
  driver: Driver;
  group: string;
  color: string;
  file: string;
  host: string;
  port: number;
  username: string;
  password: string;
  database: string;
  mongoAuthSource: string;
  sslMode: SslMode;
  sslCaPath: string;
  sslClientCertPath: string;
  sslClientKeyPath: string;
  savePassword: boolean;
  sshEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuth: SshAuth;
  sshKeyPath: string;
  sshKeyPassphrase: string;
  saveSshKeyPassphrase: boolean;
  sshPassword: string;
  saveSshPassword: boolean;
  readOnly: boolean;
}>({
  id: "",
  name: "",
  driver: "mysql",
  group: "",
  color: palette[0],
  file: "",
  host: "127.0.0.1",
  port: 3306,
  username: "root",
  password: "",
  database: "",
  mongoAuthSource: "admin",
  sslMode: "disabled",
  sslCaPath: "",
  sslClientCertPath: "",
  sslClientKeyPath: "",
  savePassword: true,
  sshEnabled: false,
  sshHost: "",
  sshPort: 22,
  sshUsername: "",
  sshAuth: "key",
  sshKeyPath: "",
  sshKeyPassphrase: "",
  saveSshKeyPassphrase: true,
  sshPassword: "",
  saveSshPassword: true,
  readOnly: false,
});

const testing = ref(false);
const testMessage = ref("");
const error = ref("");
const keyPassphraseSaved = ref(false);

const passwordPlaceholder = computed(() => {
  if (props.editing?.hasPassword) return "已保存，留空保持不变";
  if (form.driver === "redis") return "Redis 密码（可选）";
  if (form.driver === "mongodb") return "MongoDB 密码（可选）";
  return "数据库密码";
});

const sshPasswordPlaceholder = computed(() =>
  props.editing?.hasSshPassword ? "已保存，留空保持不变" : "SSH 密码",
);

const sshKeyPassphrasePlaceholder = computed(() =>
  keyPassphraseSaved.value
    ? "已保存，留空保持不变"
    : "私钥口令（可选）",
);

const devLocalCredentials =
  import.meta.env.DEV &&
  navigator.platform.toUpperCase().includes("MAC");
const passwordStorageLabel = computed(() =>
  devLocalCredentials
    ? "保存密码到本机凭据文件（开发版，仅当前用户可读）"
    : "保存密码到系统钥匙串",
);
const sshPasswordStorageLabel = computed(() =>
  devLocalCredentials
    ? "保存 SSH 密码到本机凭据文件（开发版）"
    : "保存 SSH 密码到系统钥匙串",
);
const keyPassphraseStorageLabel = computed(() =>
  devLocalCredentials
    ? "保存私钥口令到本机凭据文件（开发版）"
    : "保存私钥口令到系统钥匙串",
);

watch(
  [() => props.open, () => props.editing],
  ([open, editing]) => {
    if (!open) return;
    error.value = "";
    testMessage.value = "";

    if (editing) {
      form.id = editing.id;
      form.name = editing.name;
      form.driver = editing.driver;
      form.group = editing.group;
      form.color = editing.color || palette[0];
      form.file = editing.file;
      form.host = editing.host || "127.0.0.1";
      form.port = editing.port || defaultPort(editing.driver);
      form.username = editing.username;
      form.password = "";
      form.database = editing.database;
      form.mongoAuthSource = editing.mongoAuthSource || "admin";
      form.sslMode = editing.sslMode;
      form.sslCaPath = editing.sslCaPath ?? "";
      form.sslClientCertPath = editing.sslClientCertPath ?? "";
      form.sslClientKeyPath = editing.sslClientKeyPath ?? "";
      form.savePassword = editing.savePassword;
      form.sshEnabled = editing.sshEnabled;
      form.sshHost = editing.sshHost;
      form.sshPort = editing.sshPort || 22;
      form.sshUsername = editing.sshUsername;
      form.sshAuth = editing.sshAuth;
      form.sshKeyPath = editing.sshKeyPath;
      form.sshKeyPassphrase = "";
      form.saveSshKeyPassphrase = editing.saveSshKeyPassphrase;
      keyPassphraseSaved.value = editing.hasSshKeyPassphrase;
      form.sshPassword = "";
      form.saveSshPassword = editing.saveSshPassword;
      form.readOnly = editing.readOnly;
    } else {
      form.id = "";
      form.name = "";
      form.driver = "mysql";
      form.group = props.defaultGroup ?? "";
      form.color = palette[0];
      form.file = "";
      form.host = "127.0.0.1";
      form.port = 3306;
      form.username = "root";
      form.password = "";
      form.database = "";
      form.mongoAuthSource = "admin";
      form.sslMode = "disabled";
      form.sslCaPath = "";
      form.sslClientCertPath = "";
      form.sslClientKeyPath = "";
      form.savePassword = true;
      form.sshEnabled = false;
      form.sshHost = "";
      form.sshPort = 22;
      form.sshUsername = "";
      form.sshAuth = "key";
      form.sshKeyPath = "";
      form.sshKeyPassphrase = "";
      form.saveSshKeyPassphrase = true;
      keyPassphraseSaved.value = false;
      form.sshPassword = "";
      form.saveSshPassword = true;
      form.readOnly = false;
    }
  },
  { immediate: true },
);

watch(
  () => form.driver,
  (driver) => {
    if (
      driver === "mysql" &&
      (!form.port || form.port === 0 || form.port === 6379 || form.port === 5432 || form.port === 27017)
    ) {
      form.port = 3306;
    }
    if (
      driver === "postgres" &&
      (!form.port || form.port === 0 || form.port === 3306 || form.port === 6379 || form.port === 27017)
    ) {
      form.port = 5432;
    }
    if (driver === "redis") {
      if (!form.port || form.port === 0 || form.port === 3306 || form.port === 5432 || form.port === 27017) {
        form.port = 6379;
      }
      if (!form.database.trim()) form.database = "0";
    }
    if (driver === "mongodb") {
      if (!form.port || form.port === 0 || form.port === 3306 || form.port === 5432 || form.port === 6379) {
        form.port = 27017;
      }
      if (!form.mongoAuthSource.trim()) form.mongoAuthSource = "admin";
    }
  },
);

watch(
  () => props.saveError,
  (value) => {
    if (value) error.value = value;
  },
);

watch(
  form,
  () => {
    testMessage.value = "";
  },
  { deep: true },
);

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") {
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
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

function toConfig(): ConnectionConfig {
  return {
    id: form.id,
    name: form.name.trim(),
    driver: form.driver,
    group: form.group.trim(),
    color: form.color,
    file: form.file.trim(),
    host: form.host.trim(),
    port: Number(form.port) || defaultPort(form.driver),
    username: form.username.trim(),
    database: form.database.trim(),
    mongoAuthSource: form.mongoAuthSource.trim(),
    sslMode: form.sslMode,
    sslCaPath: form.sslCaPath.trim(),
    sslClientCertPath: form.sslClientCertPath.trim(),
    sslClientKeyPath: form.sslClientKeyPath.trim(),
    sshEnabled: form.sshEnabled,
    sshHost: form.sshHost.trim(),
    sshPort: Number(form.sshPort) || 22,
    sshUsername: form.sshUsername.trim(),
    sshAuth: form.sshAuth,
    sshKeyPath: form.sshKeyPath.trim(),
    savePassword: form.driver === "sqlite" ? false : form.savePassword,
    saveSshPassword:
      form.sshEnabled && form.sshAuth === "password"
        ? form.saveSshPassword
        : false,
    saveSshKeyPassphrase:
      form.sshEnabled && form.sshAuth === "key"
        ? form.saveSshKeyPassphrase
        : false,
    readOnly: form.readOnly,
    hasPassword: props.editing?.hasPassword ?? false,
    hasSshPassword: props.editing?.hasSshPassword ?? false,
    hasSshKeyPassphrase: keyPassphraseSaved.value,
  };
}

async function browse() {
  error.value = "";
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "SQLite 数据库", extensions: ["db", "sqlite", "sqlite3"] }],
  });

  if (typeof selected !== "string") return;

  form.file = selected;
  if (!form.name) {
    const filename = selected.replace(/\\/g, "/").split("/").pop() ?? "";
    form.name = filename.replace(/\.(db|sqlite|sqlite3)$/i, "");
  }
}

async function browseKey() {
  error.value = "";
  const selected = await openDialog({
    multiple: false,
    directory: false,
    title: "选择 SSH 私钥",
  });

  if (typeof selected === "string") {
    form.sshKeyPath = selected;
  }
}

async function browseTlsFile(
  kind: "ca" | "cert" | "key",
) {
  error.value = "";
  const selected = await openDialog({
    multiple: false,
    directory: false,
    title:
      kind === "ca"
        ? "选择 CA 证书"
        : kind === "cert"
          ? "选择客户端证书"
          : "选择客户端私钥",
    filters: [
      { name: "证书 / 私钥", extensions: ["pem", "crt", "cer", "key"] },
    ],
  });
  if (typeof selected !== "string") return;
  if (kind === "ca") form.sslCaPath = selected;
  else if (kind === "cert") form.sslClientCertPath = selected;
  else form.sslClientKeyPath = selected;
}

function validate(): boolean {
  if (!form.name.trim()) {
    error.value = "请输入连接名称";
    return false;
  }
  if (form.driver === "sqlite" && !form.file.trim()) {
    error.value = "请选择 SQLite 文件";
    return false;
  }
  if (form.driver === "redis") {
    if (!form.host.trim()) {
      error.value = "请输入主机地址";
      return false;
    }
    const database = form.database.trim();
    if (database && !/^\d+$/.test(database)) {
      error.value = "Redis 数据库编号必须是非负整数";
      return false;
    }
  }
  if (form.driver === "mysql" || form.driver === "postgres") {
    if (!form.host.trim()) {
      error.value = "请输入主机地址";
      return false;
    }
    if (!form.username.trim()) {
      error.value = "请输入用户名";
      return false;
    }
  }
  if (form.driver === "mongodb" && !form.host.trim()) {
    error.value = "请输入主机地址";
    return false;
  }
  if (form.sshEnabled && form.driver !== "sqlite") {
    if (!form.sshHost.trim()) {
      error.value = "请输入 SSH 主机";
      return false;
    }
    if (!form.sshUsername.trim()) {
      error.value = "请输入 SSH 用户名";
      return false;
    }
    if (form.sshAuth === "key" && !form.sshKeyPath.trim()) {
      error.value = "请选择 SSH 私钥";
      return false;
    }
  }
  return true;
}

async function test() {
  error.value = "";
  testMessage.value = "";
  if (!validate()) return;

  testing.value = true;
  try {
    await api.testConnection(
      toConfig(),
      form.password || undefined,
      form.sshPassword || undefined,
      form.sshKeyPassphrase || undefined,
    );
    testMessage.value = "连接成功";
  } catch (reason) {
    error.value = String(reason);
  } finally {
    testing.value = false;
  }
}

async function clearKeyPassphrase() {
  const id = props.editing?.id;
  if (!id) return;
  try {
    await api.clearConnectionSecret(id, "ssh-key");
    form.sshKeyPassphrase = "";
    form.saveSshKeyPassphrase = false;
    keyPassphraseSaved.value = false;
    testMessage.value = "已清除保存的私钥口令";
  } catch (reason) {
    error.value = String(reason);
  }
}

function submit() {
  error.value = "";
  if (!validate()) return;

  emit("save", {
    config: toConfig(),
    password: form.password || undefined,
    sshPassword: form.sshPassword || undefined,
    sshKeyPassphrase: form.sshKeyPassphrase || undefined,
  });
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal connection-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="connection-dialog-title"
    >
      <header class="modal-header">
        <div>
          <h2 id="connection-dialog-title">{{ editing ? "编辑连接" : "新建连接" }}</h2>
          <p>
            {{
              form.driver === "sqlite"
                ? "连接本地 SQLite 数据库"
                : form.driver === "postgres"
                  ? "连接 PostgreSQL 数据库"
                  : form.driver === "redis"
                    ? "连接 Redis 服务器"
                    : form.driver === "mongodb"
                      ? "连接 MongoDB 服务器"
                      : "连接 MySQL / MariaDB 数据库"
            }}
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <div class="form-grid">
          <label>
            连接名称
            <input v-model="form.name" placeholder="例如：本地开发库" />
          </label>
          <label>
            数据库类型
            <select v-model="form.driver">
              <option value="mysql">🐬 MySQL / MariaDB</option>
              <option value="postgres">PostgreSQL</option>
              <option value="mongodb">MongoDB</option>
              <option value="redis">Redis</option>
              <option value="sqlite">SQLite</option>
            </select>
          </label>

          <label v-if="form.driver === 'sqlite'" class="span-2">
            数据库文件
            <div class="file-row">
              <input v-model="form.file" placeholder="/path/to/database.db" />
              <button class="button ghost" @click="browse">选择文件</button>
            </div>
          </label>

          <template v-else>
            <label>
              主机
              <input v-model="form.host" placeholder="127.0.0.1" />
            </label>
            <label>
              端口
              <input v-model.number="form.port" type="number" min="1" max="65535" />
            </label>
            <label>
              {{ form.driver === "redis" || form.driver === "mongodb" ? "用户名（可选）" : "用户名" }}
              <input
                v-model="form.username"
                autocomplete="username"
                :placeholder="form.driver === 'mongodb' ? '留空表示无认证' : form.driver === 'redis' ? '留空使用 default' : 'root'"
              />
            </label>
            <label>
              密码
              <input
                v-model="form.password"
                type="password"
                autocomplete="current-password"
                :placeholder="passwordPlaceholder"
              />
            </label>
            <label>
              {{
                form.driver === "redis"
                  ? "数据库编号"
                  : form.driver === "mongodb"
                    ? "默认数据库（可选）"
                    : "数据库（可选）"
              }}
              <input
                v-model="form.database"
                :placeholder="
                  form.driver === 'redis'
                    ? '0'
                    : form.driver === 'postgres'
                      ? '留空使用 postgres'
                      : form.driver === 'mongodb'
                        ? '留空使用 admin'
                        : '留空使用 mysql'
                "
              />
            </label>
            <label v-if="form.driver === 'mongodb'">
              认证数据库（authSource）
              <input v-model="form.mongoAuthSource" placeholder="admin" />
            </label>
            <label>
              {{ form.driver === "redis" || form.driver === "mongodb" ? "TLS" : "SSL" }}
              <select v-model="form.sslMode">
                <option value="disabled">禁用</option>
                <option v-if="form.driver !== 'redis' && form.driver !== 'mongodb'" value="preferred">
                  优先使用
                </option>
                <option value="required">
                  {{ form.driver === "redis" ? "必须使用（TLS）" : "必须使用" }}
                </option>
                <template v-if="form.driver !== 'redis' && form.driver !== 'mongodb'">
                  <option value="verify-ca">验证 CA（verify-ca）</option>
                  <option value="verify-full">
                    完整验证（verify-full，含主机名）
                  </option>
                </template>
              </select>
            </label>
            <template
              v-if="
                form.driver !== 'redis' &&
                form.driver !== 'mongodb' &&
                (form.sslMode === 'verify-ca' ||
                  form.sslMode === 'verify-full')
              "
            >
              <label class="span-2">
                CA 证书
                <div class="file-row">
                  <input
                    v-model="form.sslCaPath"
                    placeholder="/path/to/ca.pem"
                  />
                  <button
                    type="button"
                    class="button ghost"
                    @click="browseTlsFile('ca')"
                  >
                    选择文件
                  </button>
                </div>
              </label>
            </template>
            <template v-if="form.driver !== 'redis' && form.driver !== 'mongodb' && form.sslMode !== 'disabled'">
              <label>
                客户端证书（可选）
                <div class="file-row">
                  <input
                    v-model="form.sslClientCertPath"
                    placeholder="client-cert.pem"
                  />
                  <button
                    type="button"
                    class="button ghost"
                    @click="browseTlsFile('cert')"
                  >
                    选择
                  </button>
                </div>
              </label>
              <label>
                客户端私钥（可选）
                <div class="file-row">
                  <input
                    v-model="form.sslClientKeyPath"
                    placeholder="client-key.pem"
                  />
                  <button
                    type="button"
                    class="button ghost"
                    @click="browseTlsFile('key')"
                  >
                    选择
                  </button>
                </div>
              </label>
            </template>
            <p
              v-if="
                form.driver !== 'redis' &&
                form.sshEnabled &&
                form.sslMode === 'verify-full'
              "
              class="hint span-2"
            >
              SSH 隧道会把数据库主机名变成 127.0.0.1，verify-full 可能因主机名与证书不匹配而失败；这种情况可先用 verify-ca。
            </p>
            <label class="checkbox-row span-2">
              <input v-model="form.savePassword" type="checkbox" />
              {{ passwordStorageLabel }}
            </label>
          </template>

          <label class="checkbox-row span-2">
            <input v-model="form.readOnly" type="checkbox" />
            只读连接（阻止写入）
          </label>

          <label>
            分组
            <input v-model="form.group" placeholder="例如：测试 / 生产（可留空）" />
          </label>
          <label>
            颜色
            <div class="color-row">
              <button
                v-for="color in palette"
                :key="color"
                class="color-swatch"
                :class="{ selected: form.color === color }"
                :style="{ background: color }"
                :title="color"
                @click="form.color = color"
              ></button>
            </div>
          </label>

          <template v-if="form.driver === 'mysql' || form.driver === 'postgres' || form.driver === 'redis' || form.driver === 'mongodb'">
            <label class="checkbox-row span-2 ssh-toggle">
              <input v-model="form.sshEnabled" type="checkbox" />
              通过 SSH 隧道连接
            </label>

            <template v-if="form.sshEnabled">
              <label>
                SSH 主机
                <input v-model="form.sshHost" placeholder="bastion.example.com" />
              </label>
              <label>
                SSH 端口
                <input
                  v-model.number="form.sshPort"
                  type="number"
                  min="1"
                  max="65535"
                />
              </label>
              <label>
                SSH 用户名
                <input v-model="form.sshUsername" placeholder="root" />
              </label>
              <label>
                SSH 认证
                <select v-model="form.sshAuth">
                  <option value="key">私钥</option>
                  <option value="password">密码</option>
                </select>
              </label>
              <label v-if="form.sshAuth === 'key'" class="span-2">
                SSH 私钥
                <div class="file-row">
                  <input v-model="form.sshKeyPath" placeholder="~/.ssh/id_ed25519" />
                  <button class="button ghost" @click="browseKey">选择文件</button>
                </div>
              </label>
              <label v-else class="span-2">
                SSH 密码
                <input
                  v-model="form.sshPassword"
                  type="password"
                  autocomplete="current-password"
                  :placeholder="sshPasswordPlaceholder"
                />
              </label>
              <label v-if="form.sshAuth === 'key'" class="span-2">
                SSH 私钥口令
                <div class="file-row">
                  <input
                    v-model="form.sshKeyPassphrase"
                    type="password"
                    autocomplete="off"
                    :placeholder="sshKeyPassphrasePlaceholder"
                  />
                  <button
                    v-if="keyPassphraseSaved"
                    type="button"
                    class="button ghost"
                    @click="clearKeyPassphrase"
                  >
                    清除已保存
                  </button>
                </div>
              </label>
              <label
                v-if="form.sshAuth === 'key'"
                class="checkbox-row span-2"
              >
                <input v-model="form.saveSshKeyPassphrase" type="checkbox" />
                {{ keyPassphraseStorageLabel }}
              </label>
              <label
                v-if="form.sshAuth === 'password'"
                class="checkbox-row span-2"
              >
                <input v-model="form.saveSshPassword" type="checkbox" />
                {{ sshPasswordStorageLabel }}
              </label>
            </template>
          </template>
        </div>

        <p v-if="testMessage" class="message success">{{ testMessage }}</p>
        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <button class="button ghost" :disabled="testing" @click="test">
          {{ testing ? "测试中…" : "测试连接" }}
        </button>
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">取消</button>
        <button class="button primary" @click="submit">
          {{ editing ? "保存" : "创建连接" }}
        </button>
      </footer>
    </div>
  </div>
</template>
