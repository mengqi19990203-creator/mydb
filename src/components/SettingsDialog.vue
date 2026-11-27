<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import AiSettingsSection from "./AiSettingsSection.vue";
import Icon from "./Icon.vue";
import {
  applyEditorFontSize,
  applyTheme,
  loadEditorFontSize,
  loadThemeMode,
  saveThemeMode,
  type ThemeMode,
} from "../lib/theme";
import {
  loadDangerConfirm,
  saveDangerConfirm,
} from "../lib/dangerous-sql";
import {
  loadQueryRunMode,
  saveQueryRunMode,
  type QueryRunMode,
} from "../lib/query-run-mode";
import {
  loadAutoReconnect,
  saveAutoReconnect,
} from "../lib/connection-health";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  "open-format-settings": [];
  "run-mode-change": [mode: QueryRunMode];
  "auto-reconnect-change": [enabled: boolean];
}>();

const theme = ref<ThemeMode>("dark");
const fontSize = ref(13);
const dangerConfirm = ref(true);
const runMode = ref<QueryRunMode>("statement");
const autoReconnect = ref(true);
const saved = ref(false);
const themeOptions: { value: ThemeMode; label: string; hint: string }[] = [
  { value: "dark", label: "深色", hint: "当前默认" },
  { value: "light", label: "浅色", hint: "适合明亮环境" },
  { value: "system", label: "跟随系统", hint: "自动切换" },
];

function load() {
  theme.value = loadThemeMode();
  fontSize.value = loadEditorFontSize();
  dangerConfirm.value = loadDangerConfirm();
  runMode.value = loadQueryRunMode();
  autoReconnect.value = loadAutoReconnect();
  saved.value = false;
}

function selectTheme(value: ThemeMode) {
  theme.value = value;
  applyTheme(value);
  saveThemeMode(value);
  saved.value = true;
}

function changeFontSize() {
  applyEditorFontSize(fontSize.value);
  saved.value = true;
}

function toggleDanger() {
  dangerConfirm.value = !dangerConfirm.value;
  saveDangerConfirm(dangerConfirm.value);
  saved.value = true;
}

function changeRunMode() {
  saveQueryRunMode(runMode.value);
  emit("run-mode-change", runMode.value);
  saved.value = true;
}

function toggleAutoReconnect() {
  autoReconnect.value = !autoReconnect.value;
  saveAutoReconnect(autoReconnect.value);
  emit("auto-reconnect-change", autoReconnect.value);
  saved.value = true;
}

function openFormatSettings() {
  emit("open-format-settings");
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      load();
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal settings-modal" role="dialog" aria-modal="true" aria-label="设置">
      <header class="modal-header">
        <div>
          <h2>设置</h2>
          <p>外观、编辑器与安全选项；所有设置保存在本机。</p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <Icon name="close" />
        </button>
      </header>

      <div class="settings-body">
        <section class="settings-section">
          <h3>外观</h3>
          <p class="hint">「跟随系统」会随 macOS 外观自动切换。</p>
          <div class="settings-theme-grid">
            <button
              v-for="option in themeOptions"
              :key="option.value"
              class="settings-theme-card"
              :class="{ active: theme === option.value }"
              @click="selectTheme(option.value)"
            >
              <span class="settings-theme-preview" :class="option.value"></span>
              <strong>{{ option.label }}</strong>
              <small>{{ option.hint }}</small>
            </button>
          </div>
        </section>

        <section class="settings-section">
          <h3>编辑器</h3>
          <label class="settings-row">
            <span>SQL 编辑器字号</span>
            <div class="settings-inline">
              <input
                v-model.number="fontSize"
                type="range"
                min="11"
                max="20"
                step="1"
                @change="changeFontSize"
              />
              <strong>{{ fontSize }} px</strong>
            </div>
          </label>
          <div class="settings-row">
            <span>格式化选项</span>
            <button class="button ghost compact" @click="openFormatSettings">
              打开格式化设置…
            </button>
          </div>
        </section>

        <section class="settings-section">
          <h3>查询</h3>
          <label class="settings-row">
            <span>
              运行按钮默认范围
              <small class="settings-hint">
                有选区时始终执行选中 SQL；无选区时按此设置
              </small>
            </span>
            <select v-model="runMode" @change="changeRunMode">
              <option value="statement">当前语句</option>
              <option value="tab">整个标签</option>
            </select>
          </label>
        </section>

        <section class="settings-section">
          <h3>连接</h3>
          <label class="settings-row">
            <span>
              断线后自动重连
              <small class="settings-hint">
                仅恢复数据库 / SSH / Redis 连接，不会重新执行 SQL，也不会回滚事务
              </small>
            </span>
            <input
              :checked="autoReconnect"
              type="checkbox"
              @change="toggleAutoReconnect"
            />
          </label>
        </section>

        <section class="settings-section">
          <h3>安全</h3>
          <label class="settings-row">
            <span>
              危险 SQL 二次确认
              <small class="settings-hint">
                缺少 WHERE 的 UPDATE / DELETE、DROP、TRUNCATE 等执行前确认
              </small>
            </span>
            <input
              :checked="dangerConfirm"
              type="checkbox"
              @change="toggleDanger"
            />
          </label>
        </section>

        <section class="settings-section settings-about">
          <h3>关于</h3>
          <p>mydb 0.11.0 · 蒙其同学</p>
          <p class="hint">
            轻量跨平台数据库客户端。设置与连接信息仅保存在本机，不上传云端。
          </p>
        </section>

        <AiSettingsSection />
      </div>

      <footer class="modal-footer">
        <span v-if="saved" class="hint">已自动保存</span>
        <div class="spacer"></div>
        <button class="button primary" @click="emit('close')">完成</button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.settings-modal {
  width: min(760px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
}

.settings-body {
  min-height: 0;
  overflow: auto;
  padding: 12px 20px 18px;
}

.settings-section {
  padding: 12px 0;
  border-bottom: 1px solid var(--border-subtle);
}

.settings-section:last-child {
  border-bottom: 0;
}

.settings-section h3 {
  margin: 0 0 6px;
  font-size: 12px;
  font-weight: 600;
}

.settings-theme-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin-top: 8px;
}

.settings-theme-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  padding: 9px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--panel-2);
  color: var(--text-secondary);
  text-align: left;
}

.settings-theme-card.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

.settings-theme-card strong {
  font-size: 11px;
}

.settings-theme-card small {
  color: var(--faint);
  font-size: 9px;
}

.settings-theme-preview {
  width: 100%;
  height: 34px;
  margin-bottom: 4px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background:
    linear-gradient(90deg, #202024 0 28%, #18181b 28% 100%);
}

.settings-theme-preview.light {
  background:
    linear-gradient(90deg, #ffffff 0 28%, #f4f6f8 28% 100%);
}

.settings-theme-preview.system {
  background:
    linear-gradient(90deg, #202024 0 50%, #f4f6f8 50% 100%);
}

.settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 6px 0;
  color: var(--text-secondary);
  font-size: 11px;
}

.settings-inline {
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-inline input[type="range"] {
  width: 180px;
}

.settings-hint {
  display: block;
  margin-top: 2px;
  color: var(--faint);
  font-size: 9px;
}

.settings-about p {
  margin: 4px 0;
  font-size: 11px;
}
</style>