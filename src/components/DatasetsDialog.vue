<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  datasetHtml,
  datasetScopeLabel,
  deleteDataset,
  listDatasets,
  saveDataset,
  type DatasetSource,
  type SavedDataset,
} from "../lib/datasets";

const props = defineProps<{
  open: boolean;
  current: DatasetSource | null;
}>();

const emit = defineEmits<{
  close: [];
  openDataset: [dataset: SavedDataset];
}>();

const datasets = ref<SavedDataset[]>([]);
const name = ref("");
const error = ref("");
const message = ref("");
const deleteTarget = ref<SavedDataset | null>(null);
const saving = ref(false);

const canSave = computed(() => Boolean(props.current?.result?.isQuery));

function refresh() {
  datasets.value = listDatasets();
}

function resetMessage() {
  window.setTimeout(() => {
    message.value = "";
  }, 2600);
}

function saveCurrent() {
  if (!props.current || !canSave.value || saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    saveDataset({
      ...props.current,
      name: name.value.trim() || props.current.name,
    });
    name.value = "";
    refresh();
    message.value = "已保存数据集";
    resetMessage();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}

async function exportDataset(dataset: SavedDataset) {
  const path = await saveDialog({
    defaultPath: `${dataset.name.replace(/[\\/:*?"<>|]/g, "_") || "report"}.html`,
    filters: [{ name: "HTML 报表", extensions: ["html"] }],
  });
  if (typeof path !== "string") return;
  try {
    await api.writeTextFile(path, datasetHtml(dataset));
    message.value = `已导出报表到 ${path}`;
    resetMessage();
  } catch (reason) {
    error.value = String(reason);
  }
}

function removeDataset() {
  const target = deleteTarget.value;
  deleteTarget.value = null;
  if (!target) return;
  deleteDataset(target.id);
  refresh();
  message.value = `已删除数据集 ${target.name}`;
  resetMessage();
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      refresh();
      name.value = props.current?.name ?? "";
      error.value = "";
      message.value = "";
      document.addEventListener("keydown", onKeydown, true);
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
    <div class="modal datasets-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>数据集与报表</h2>
          <p>保存查询结果快照，打开原 SQL 重跑，或导出可打印的 HTML 报表。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="datasets-body">
        <section class="datasets-save">
          <div>
            <strong>保存当前结果</strong>
            <p v-if="canSave" class="hint">
              {{ current?.result?.columns.length ?? 0 }} 列 ·
              {{ current?.result?.rows.length ?? 0 }} 行 ·
              {{ current?.connectionName }}
            </p>
            <p v-else class="hint">当前没有可保存的查询结果。</p>
          </div>
          <div class="datasets-save-actions">
            <input v-model="name" :disabled="!canSave" placeholder="数据集名称" />
            <button class="button primary" :disabled="!canSave || saving" @click="saveCurrent">
              {{ saving ? "保存中…" : "保存快照" }}
            </button>
          </div>
        </section>
        <p class="hint">
          快照最多保存 2,000 行 / 单个约 2 MB；报表是保存时的数据，不代表数据库当前状态。
        </p>
        <p v-if="error" class="data-message warning">{{ error }}</p>
        <p v-if="message" class="data-message success">{{ message }}</p>

        <div v-if="!datasets.length" class="mongo-gridfs-empty">
          <Icon name="list" :size="22" />
          <strong>还没有数据集</strong>
          <span>保存当前查询结果后，可在这里打开 SQL 或导出 HTML 报表。</span>
        </div>
        <div v-else class="datasets-list">
          <article v-for="dataset in datasets" :key="dataset.id" class="dataset-card">
            <div class="dataset-card-main">
              <strong>{{ dataset.name }}</strong>
              <p class="hint">
                {{ dataset.connectionName }} · {{ datasetScopeLabel(dataset) || "—" }}
                · {{ dataset.rows.length }} / {{ dataset.rowCount }} 行
                <template v-if="dataset.truncated">（快照已截断）</template>
              </p>
              <pre class="dataset-sql">{{ dataset.sql }}</pre>
            </div>
            <div class="dataset-card-actions">
              <button class="button ghost compact" @click="emit('openDataset', dataset)">
                <Icon name="code" :size="12" /> 打开 SQL
              </button>
              <button class="button ghost compact" @click="exportDataset(dataset)">
                <Icon name="download" :size="12" /> 导出 HTML 报表
              </button>
              <button class="button ghost compact danger-text" @click="deleteTarget = dataset">
                <Icon name="trash" :size="12" /> 删除
              </button>
            </div>
          </article>
        </div>
      </div>

      <footer class="modal-actions">
        <span class="hint">共 {{ datasets.length }} 个数据集</span>
        <span class="spacer"></span>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>

      <ConfirmDialog
        :open="Boolean(deleteTarget)"
        title="删除数据集"
        :message="`确定删除数据集「${deleteTarget?.name ?? ''}」？`"
        detail="只删除本地保存的快照和 SQL，不会影响数据库。"
        confirm-text="删除数据集"
        danger
        @confirm="removeDataset"
        @cancel="deleteTarget = null"
      />
    </div>
  </div>
</template>
