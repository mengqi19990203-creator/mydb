<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import type { MongoGridFsFile } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  database: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{ status: [message: string] }>();

const buckets = ref<string[]>([]);
const bucket = ref("fs");
const files = ref<MongoGridFsFile[]>([]);
const search = ref("");
const searchTerm = ref("");
const loading = ref(false);
const uploading = ref(false);
const error = ref("");
const deleteTarget = ref<MongoGridFsFile | null>(null);
const deleting = ref(false);

const canWrite = computed(() => !props.readOnly);

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(1)} ${units[index]}`;
}

function formatDate(value: string): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString("zh-CN", { hour12: false });
}

async function loadBuckets() {
  try {
    const result = await api.mongoListGridFsBuckets(
      props.connectionId,
      props.database,
    );
    buckets.value = result;
    if (!result.includes(bucket.value)) {
      bucket.value = result[0] ?? "fs";
    }
  } catch (reason) {
    error.value = String(reason);
  }
}

async function loadFiles() {
  if (!props.database || !bucket.value) {
    files.value = [];
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    files.value = await api.mongoListGridFsFiles(
      props.connectionId,
      props.database,
      bucket.value,
      searchTerm.value,
      0,
      200,
    );
  } catch (reason) {
    files.value = [];
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

async function refresh() {
  await loadBuckets();
  await loadFiles();
}

async function applySearch() {
  searchTerm.value = search.value.trim();
  await loadFiles();
}

async function upload() {
  if (!canWrite.value || uploading.value) return;
  const selected = await openDialog({ multiple: false, directory: false });
  if (typeof selected !== "string") return;
  uploading.value = true;
  error.value = "";
  try {
    const file = await api.mongoUploadGridFsFile(
      props.connectionId,
      props.database,
      bucket.value || "fs",
      selected,
    );
    emit("status", `已上传 GridFS 文件：${file.filename}`);
    await loadBuckets();
    await loadFiles();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    uploading.value = false;
  }
}

async function download(file: MongoGridFsFile) {
  const destination = await saveDialog({
    defaultPath: file.filename || "gridfs-file",
  });
  if (typeof destination !== "string") return;
  error.value = "";
  try {
    await api.mongoDownloadGridFsFile(
      props.connectionId,
      props.database,
      bucket.value,
      file.id,
      destination,
    );
    emit("status", `已下载 GridFS 文件到 ${destination}`);
  } catch (reason) {
    error.value = String(reason);
  }
}

async function confirmDelete() {
  const file = deleteTarget.value;
  deleteTarget.value = null;
  if (!file) return;
  deleting.value = true;
  error.value = "";
  try {
    await api.mongoDeleteGridFsFile(
      props.connectionId,
      props.database,
      bucket.value,
      file.id,
    );
    emit("status", `已删除 GridFS 文件：${file.filename}`);
    await loadFiles();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    deleting.value = false;
  }
}

watch(
  () => props.database,
  () => {
    bucket.value = "fs";
    search.value = "";
    searchTerm.value = "";
    void refresh();
  },
);

onMounted(() => void refresh());
</script>

<template>
  <div class="mongo-gridfs">
    <div class="mongo-gridfs-toolbar">
      <label class="mongo-gridfs-bucket">
        <span>Bucket</span>
        <select v-model="bucket" :disabled="loading" @change="loadFiles">
          <option v-for="item in buckets.length ? buckets : ['fs']" :key="item" :value="item">
            {{ item }}
          </option>
        </select>
      </label>
      <div class="mongo-gridfs-search">
        <Icon name="search" :size="12" />
        <input
          v-model="search"
          placeholder="搜索文件名"
          @keydown.enter.prevent="applySearch"
        />
      </div>
      <button class="button ghost compact" :disabled="loading" @click="refresh">
        <Icon name="refresh" :size="13" /> 刷新
      </button>
      <span class="spacer"></span>
      <button
        class="button primary compact"
        :disabled="!canWrite || uploading"
        :title="canWrite ? '上传文件到 GridFS' : '只读连接不能上传'"
        @click="upload"
      >
        <Icon name="upload" :size="13" />
        {{ uploading ? "上传中…" : "上传文件" }}
      </button>
    </div>

    <p v-if="readOnly" class="hint">只读连接：可以浏览与下载，不能上传或删除。</p>
    <p v-if="error" class="data-message warning">{{ error }}</p>

    <div v-if="loading" class="mongo-gridfs-empty">读取 GridFS 文件…</div>
    <div v-else-if="!files.length" class="mongo-gridfs-empty">
      <Icon name="upload" :size="24" />
      <strong>没有 GridFS 文件</strong>
      <span>{{ searchTerm ? "没有匹配的文件名" : "上传第一个文件后会在这里显示。" }}</span>
    </div>
    <div v-else class="mongo-gridfs-table-wrap">
      <table class="mongo-gridfs-table">
        <thead>
          <tr>
            <th>文件名</th>
            <th>大小</th>
            <th>块大小</th>
            <th>上传时间</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="file in files" :key="JSON.stringify(file.id)">
            <td class="mongo-gridfs-name" :title="file.filename">
              <Icon name="upload" :size="12" />
              {{ file.filename || "(未命名)" }}
            </td>
            <td>{{ formatBytes(file.length) }}</td>
            <td>{{ formatBytes(file.chunkSize) }}</td>
            <td>{{ formatDate(file.uploadDate) }}</td>
            <td class="mongo-gridfs-actions">
              <button class="button ghost compact" @click="download(file)">
                <Icon name="download" :size="12" /> 下载
              </button>
              <button
                class="button ghost compact danger-text"
                :disabled="!canWrite || deleting"
                @click="deleteTarget = file"
              >
                <Icon name="trash" :size="12" /> 删除
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <p class="hint">显示最近 200 个文件；搜索支持文件名子串。</p>
    </div>

    <ConfirmDialog
      :open="Boolean(deleteTarget)"
      title="删除 GridFS 文件"
      :message="`确定删除文件「${deleteTarget?.filename ?? ''}」？`"
      detail="文件及其 chunks 会从 MongoDB GridFS bucket 中删除，此操作不可撤销。"
      confirm-text="删除文件"
      danger
      @confirm="confirmDelete"
      @cancel="deleteTarget = null"
    />
  </div>
</template>
