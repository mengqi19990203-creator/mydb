<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import * as api from "../lib/api";
import {
  AI_PROVIDER_PRESETS,
  aiProviderLabel,
  loadAiSettings,
  saveAiSettings,
  type AiProvider,
  type AiSettings,
} from "../lib/ai-settings";

const settings = ref<AiSettings>(loadAiSettings());
const apiKey = ref("");
const hasKey = ref(false);
const saving = ref(false);
const testing = ref(false);
const message = ref("");
const error = ref("");

const providerOptions = Object.keys(AI_PROVIDER_PRESETS) as AiProvider[];
const needsKey = computed(() => settings.value.provider !== "ollama");

async function refreshKeyState() {
  try {
    hasKey.value = await api.aiHasApiKey();
  } catch {
    hasKey.value = false;
  }
}

function changeProvider() {
  settings.value = { ...AI_PROVIDER_PRESETS[settings.value.provider] };
  message.value = "";
  error.value = "";
}

async function save() {
  saving.value = true;
  message.value = "";
  error.value = "";
  try {
    if (apiKey.value.trim()) {
      await api.aiSaveApiKey(apiKey.value.trim());
      apiKey.value = "";
      await refreshKeyState();
    }
    saveAiSettings(settings.value);
    message.value = "AI 设置已保存";
  } catch (reason) {
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}

async function clearKey() {
  try {
    await api.aiSaveApiKey("");
    apiKey.value = "";
    await refreshKeyState();
    message.value = "已清除 AI API Key";
  } catch (reason) {
    error.value = String(reason);
  }
}

async function test() {
  testing.value = true;
  message.value = "";
  error.value = "";
  try {
    if (apiKey.value.trim()) {
      await api.aiSaveApiKey(apiKey.value.trim());
      apiKey.value = "";
      await refreshKeyState();
    }
    saveAiSettings(settings.value);
    const reply = await api.aiChat({
      provider: settings.value.provider,
      baseUrl: settings.value.baseUrl,
      model: settings.value.model,
      systemPrompt: "You are a connectivity test. Reply with exactly OK.",
      userPrompt: "Reply with exactly OK.",
      maxTokens: 16,
    });
    message.value = `连接成功：${reply.trim().slice(0, 80)}`;
  } catch (reason) {
    error.value = String(reason);
  } finally {
    testing.value = false;
  }
}

onMounted(() => void refreshKeyState());
</script>

<template>
  <section class="settings-section ai-settings-section">
    <div class="settings-section-head">
      <h3>AI SQL 助手</h3>
      <span class="hint">用户自带模型与 API Key；不自动执行 SQL</span>
    </div>
    <div class="ai-settings-grid">
      <label>
        供应商
        <select v-model="settings.provider" @change="changeProvider">
          <option v-for="provider in providerOptions" :key="provider" :value="provider">
            {{ aiProviderLabel(provider) }}
          </option>
        </select>
      </label>
      <label>
        Base URL
        <input v-model="settings.baseUrl" placeholder="https://…" spellcheck="false" />
      </label>
      <label>
        模型
        <input v-model="settings.model" placeholder="model-name" spellcheck="false" />
      </label>
      <label>
        API Key
        <input
          v-model="apiKey"
          type="password"
          autocomplete="new-password"
          :placeholder="hasKey ? '已保存，留空保持不变' : needsKey ? '填写 API Key' : '本地 Ollama 可留空'"
        />
      </label>
    </div>
    <p class="hint">
      Key 保存在系统钥匙串 / 开发版本机凭据文件；只发送当前 SQL、方言、库 / 模式与表结构，默认不发送结果行。
    </p>
    <div class="ai-settings-actions">
      <button class="button ghost compact" :disabled="saving" @click="save">
        {{ saving ? "保存中…" : "保存设置" }}
      </button>
      <button class="button ghost compact" :disabled="testing || !settings.baseUrl || !settings.model" @click="test">
        <Icon name="bolt" :size="12" />
        {{ testing ? "测试中…" : "测试连接" }}
      </button>
      <button v-if="hasKey" class="button ghost compact danger-text" @click="clearKey">
        清除 Key
      </button>
      <span v-if="message" class="hint">{{ message }}</span>
      <span v-if="error" class="data-message warning">{{ error }}</span>
    </div>
  </section>
</template>
