<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type {
  ConnectionConfig,
  ConnectionGroupInfo,
  Driver,
  SslMode,
} from "../lib/types";

const props = defineProps<{
  open: boolean;
  connections: ConnectionConfig[];
  groups: ConnectionGroupInfo[];
}>();

const emit = defineEmits<{
  close: [];
  imported: [count: number];
}>();

interface ImportedConnection {
  name: string;
  driver: Driver;
  group: string;
  color: string;
  file: string;
  host: string;
  port: number;
  username: string;
  database: string;
  mongoAuthSource?: string;
  sslMode: SslMode;
  sshEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuth: "key" | "password";
  sshKeyPath: string;
  readOnly: boolean;
}

const selectedExport = ref<Set<string>>(new Set());
const importOpen = ref(false);
const imported = ref<ImportedConnection[]>([]);
const selectedImport = ref<Set<number>>(new Set());
const importing = ref(false);
const importPath = ref("");
const message = ref("");
const error = ref("");

const selectable = computed(() =>
  props.connections.filter((connection) => connection.driver !== "redis" || true),
);
const allExportSelected = computed(
  () =>
    selectable.value.length > 0 &&
    selectable.value.every((connection) =>
      selectedExport.value.has(connection.id),
    ),
);
const allImportSelected = computed(
  () =>
    imported.value.length > 0 &&
    imported.value.every((_, index) => selectedImport.value.has(index)),
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    selectedExport.value = new Set(selectable.value.map((item) => item.id));
    importOpen.value = false;
    imported.value = [];
    selectedImport.value = new Set();
    importPath.value = "";
    message.value = "";
    error.value = "";
  },
  { immediate: true },
);

function toggleExport(id: string) {
  const next = new Set(selectedExport.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedExport.value = next;
}

function toggleAllExport() {
  selectedExport.value = allExportSelected.value
    ? new Set()
    : new Set(selectable.value.map((item) => item.id));
}

function exportPayload() {
  const connections = props.connections
    .filter((connection) => selectedExport.value.has(connection.id))
    .map((connection) => ({
      name: connection.name,
      driver: connection.driver,
      group: connection.group,
      color: connection.color,
      file: connection.file,
      host: connection.host,
      port: connection.port,
      username: connection.username,
      database: connection.database,
      mongoAuthSource: connection.mongoAuthSource ?? "",
      sslMode: connection.sslMode,
      sshEnabled: connection.sshEnabled,
      sshHost: connection.sshHost,
      sshPort: connection.sshPort,
      sshUsername: connection.sshUsername,
      sshAuth: connection.sshAuth,
      sshKeyPath: connection.sshKeyPath,
      readOnly: connection.readOnly,
    }));
  return {
    format: "mydb-connections",
    version: 1,
    exportedAt: new Date().toISOString(),
    groups: props.groups,
    connections,
  };
}

async function exportConnections() {
  if (!selectedExport.value.size) return;
  const path = await saveDialog({
    defaultPath: "mydb-connections.json",
    filters: [{ name: "mydb 连接", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(
      path,
      JSON.stringify(exportPayload(), null, 2),
    );
    message.value = `已导出 ${selectedExport.value.size} 个连接（不含密码）到 ${path}`;
  } catch (reason) {
    error.value = String(reason);
  }
}

function driverFromProvider(provider: string): Driver | null {
  const value = provider.toLowerCase();
  if (value.includes("mysql") || value.includes("maria")) return "mysql";
  if (value.includes("postgres")) return "postgres";
  if (value.includes("sqlite")) return "sqlite";
  if (value.includes("mongo")) return "mongodb";
  return null;
}

function emptyConnection(
  name: string,
  driver: Driver,
): ImportedConnection {
  return {
    name,
    driver,
    group: "",
    color: "#58a6ff",
    file: "",
    host: "127.0.0.1",
    port: driver === "mysql" ? 3306 : driver === "postgres" ? 5432 : driver === "mongodb" ? 27017 : 0,
    username: driver === "postgres" ? "postgres" : "root",
    database: driver === "postgres" ? "postgres" : driver === "mysql" ? "mysql" : driver === "mongodb" ? "admin" : "",
    mongoAuthSource: driver === "mongodb" ? "admin" : "",
    sslMode: "disabled",
    sshEnabled: false,
    sshHost: "",
    sshPort: 22,
    sshUsername: "",
    sshAuth: "key",
    sshKeyPath: "",
    readOnly: false,
  };
}

function parseMydb(raw: Record<string, unknown>): ImportedConnection[] {
  const list = Array.isArray(raw.connections) ? raw.connections : [];
  return list.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const record = item as Record<string, unknown>;
    const driver = String(record.driver ?? "");
    if (!["mysql", "postgres", "sqlite", "redis"].includes(driver)) return [];
    const base = emptyConnection(
      String(record.name ?? "导入连接"),
      driver as Driver,
    );
    return [
      {
        ...base,
        group: String(record.group ?? ""),
        color: String(record.color ?? base.color),
        file: String(record.file ?? ""),
        host: String(record.host ?? base.host),
        port: Number(record.port ?? base.port) || base.port,
        username: String(record.username ?? base.username),
        database: String(record.database ?? base.database),
        sslMode: (String(record.sslMode ?? "disabled") as SslMode) ?? "disabled",
        sshEnabled: Boolean(record.sshEnabled),
        sshHost: String(record.sshHost ?? ""),
        sshPort: Number(record.sshPort ?? 22) || 22,
        sshUsername: String(record.sshUsername ?? ""),
        sshAuth: record.sshAuth === "password" ? "password" : "key",
        sshKeyPath: String(record.sshKeyPath ?? ""),
        readOnly: Boolean(record.readOnly),
      },
    ];
  });
}

function parseDbeaver(raw: Record<string, unknown>): ImportedConnection[] {
  const connections = raw.connections;
  if (!connections || typeof connections !== "object") return [];
  return Object.values(connections as Record<string, unknown>).flatMap(
    (item) => {
      if (!item || typeof item !== "object") return [];
      const record = item as Record<string, unknown>;
      const driver = driverFromProvider(
        String(record.provider ?? record.driver ?? ""),
      );
      if (!driver) return [];
      const configuration =
        record.configuration && typeof record.configuration === "object"
          ? (record.configuration as Record<string, unknown>)
          : {};
      const name = String(
        record.name ?? configuration.name ?? "DBeaver 连接",
      );
      const base = emptyConnection(name, driver);
      const url = String(configuration.url ?? "");
      let host = String(configuration.host ?? base.host);
      let port = Number(configuration.port ?? base.port) || base.port;
      let database = String(configuration.database ?? base.database);
      if (url.startsWith("jdbc:")) {
        const parsed = new URL(url.replace(/^jdbc:/, ""));
        host = parsed.hostname || host;
        port = Number(parsed.port) || port;
        database =
          parsed.pathname.replace(/^\//, "") || database;
      }
      const file =
        driver === "sqlite"
          ? String(configuration.database ?? configuration.path ?? "")
          : "";
      return [
        {
          ...base,
          host,
          port,
          database,
          file,
          username: String(configuration.user ?? base.username),
        },
      ];
    },
  );
}

function parseNavicat(text: string): ImportedConnection[] {
  const xml = new DOMParser().parseFromString(text, "text/xml");
  if (xml.querySelector("parsererror")) {
    throw new Error("Navicat XML 解析失败");
  }
  const nodes = Array.from(xml.querySelectorAll("Connection"));
  return nodes.flatMap((node) => {
    const attributes: Record<string, string> = {};
    for (const attribute of Array.from(node.attributes)) {
      attributes[attribute.name.toLowerCase()] = attribute.value;
    }
    const driver = driverFromProvider(
      attributes.conntype ?? attributes.type ?? "",
    );
    if (!driver) return [];
    const base = emptyConnection(
      attributes.connectionname ?? attributes.name ?? "Navicat 连接",
      driver,
    );
    return [
      {
        ...base,
        host: attributes.host ?? base.host,
        port: Number(attributes.port ?? base.port) || base.port,
        username: attributes.username ?? base.username,
        database: attributes.database ?? base.database,
        file:
          driver === "sqlite"
            ? attributes.database ?? attributes.path ?? ""
            : "",
      },
    ];
  });
}

async function chooseImportFile() {
  const selected = await openDialog({
    multiple: false,
    filters: [
      { name: "连接配置", extensions: ["json", "ncx", "xml"] },
    ],
  });
  if (typeof selected !== "string") return;
  error.value = "";
  message.value = "";
  try {
    const text = await api.readTextFile(selected);
    const trimmed = text.trim();
    let parsed: ImportedConnection[] = [];
    if (trimmed.startsWith("<")) {
      parsed = parseNavicat(trimmed);
    } else {
      const raw = JSON.parse(trimmed) as Record<string, unknown>;
      if (raw.format === "mydb-connections") {
        parsed = parseMydb(raw);
      } else if (raw.connections) {
        parsed = parseDbeaver(raw);
      }
    }
    if (!parsed.length) {
      throw new Error(
        "没有识别到连接；支持 mydb JSON、DBeaver data-sources.json、Navicat .ncx",
      );
    }
    importPath.value = selected;
    imported.value = parsed;
    selectedImport.value = new Set(parsed.map((_, index) => index));
    importOpen.value = true;
  } catch (reason) {
    error.value = String(reason);
  }
}

function toggleImport(index: number) {
  const next = new Set(selectedImport.value);
  if (next.has(index)) next.delete(index);
  else next.add(index);
  selectedImport.value = next;
}

function toggleAllImport() {
  selectedImport.value = allImportSelected.value
    ? new Set()
    : new Set(imported.value.map((_, index) => index));
}

async function importConnections() {
  if (!selectedImport.value.size || importing.value) return;
  importing.value = true;
  error.value = "";
  message.value = "";
  let count = 0;
  try {
    const groupNames = new Set(props.groups.map((group) => group.name));
    for (const index of selectedImport.value) {
      const item = imported.value[index];
      if (!item) continue;
      if (item.group && !groupNames.has(item.group)) {
        await api.saveConnectionGroup({
          name: item.group,
          color: "#58a6ff",
          order: groupNames.size,
          collapsed: false,
        });
        groupNames.add(item.group);
      }
      const existingNames = new Set(
        props.connections.map((connection) => connection.name),
      );
      const name = existingNames.has(item.name)
        ? `${item.name}（导入）`
        : item.name;
      const config: ConnectionConfig = {
        ...item,
        id: crypto.randomUUID(),
        name,
        savePassword: false,
        saveSshPassword: false,
        saveSshKeyPassphrase: false,
        sslCaPath: "",
        sslClientCertPath: "",
        sslClientKeyPath: "",
        hasPassword: false,
        hasSshPassword: false,
        hasSshKeyPassphrase: false,
      };
      await api.saveConnection(config);
      count += 1;
    }
    message.value = `已导入 ${count} 个连接；密码未导入，请手动补充`;
    imported.value = [];
    selectedImport.value = new Set();
    importOpen.value = false;
    emit("imported", count);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    importing.value = false;
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div
      class="modal connection-import-modal"
      role="dialog"
      aria-modal="true"
      aria-label="导入导出连接"
    >
      <header class="modal-header">
        <div>
          <h2>导入 / 导出连接</h2>
          <p>支持 mydb JSON、DBeaver data-sources.json、Navicat .ncx；密码不会导出或导入</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="connection-form">
        <section class="connection-transfer-section">
          <div class="connection-transfer-head">
            <strong>导出连接</strong>
            <button
              class="button ghost compact"
              :disabled="!selectable.length"
              @click="toggleAllExport"
            >
              {{ allExportSelected ? "全不选" : "全选" }}
            </button>
            <button
              class="button primary compact"
              :disabled="!selectedExport.size"
              @click="exportConnections"
            >
              导出到 JSON…
            </button>
          </div>
          <div class="connection-import-list">
            <label
              v-for="connection in selectable"
              :key="connection.id"
              class="connection-import-row"
            >
              <input
                type="checkbox"
                :checked="selectedExport.has(connection.id)"
                @change="toggleExport(connection.id)"
              />
              <span>{{ connection.name }}</span>
              <span class="hint">{{ connection.driver }} · {{ connection.group || "未分组" }}</span>
            </label>
          </div>
        </section>

        <section class="connection-transfer-section">
          <div class="connection-transfer-head">
            <strong>导入连接</strong>
            <button class="button ghost compact" @click="chooseImportFile">
              选择文件…
            </button>
          </div>
          <p v-if="importPath" class="hint">{{ importPath }}</p>
          <div v-if="importOpen" class="connection-import-list">
            <label class="connection-import-row header">
              <input
                type="checkbox"
                :checked="allImportSelected"
                @change="toggleAllImport"
              />
              <span>全选</span>
              <span class="hint">{{ imported.length }} 个连接</span>
            </label>
            <label
              v-for="(connection, index) in imported"
              :key="index"
              class="connection-import-row"
            >
              <input
                type="checkbox"
                :checked="selectedImport.has(index)"
                @change="toggleImport(index)"
              />
              <span>{{ connection.name }}</span>
              <span class="hint">
                {{ connection.driver }} · {{ connection.host || connection.file }}
              </span>
            </label>
          </div>
          <button
            v-if="importOpen"
            class="button primary compact"
            :disabled="!selectedImport.size || importing"
            @click="importConnections"
          >
            {{ importing ? "导入中…" : `导入 ${selectedImport.size} 个连接` }}
          </button>
        </section>

        <p class="draft-hint">
          导入不会包含密码、SSH 密码；建议导入后在连接编辑框中重新填写并保存到钥匙串。
        </p>
        <p v-if="message" class="message success">{{ message }}</p>
        <p v-if="error" class="message error">{{ error }}</p>
      </div>

      <footer class="modal-footer">
        <div class="spacer"></div>
        <button class="button ghost" @click="emit('close')">关闭</button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.connection-import-modal {
  width: min(760px, calc(100vw - 48px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
}

.connection-transfer-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--panel-2);
}

.connection-transfer-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.connection-transfer-head strong {
  flex: 1;
}

.connection-import-list {
  max-height: 220px;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: var(--surface-well);
}

.connection-import-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 12px;
}

.connection-import-row:last-child {
  border-bottom: none;
}

.connection-import-row .hint {
  margin-left: auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>