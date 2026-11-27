export type AiProvider = "anthropic" | "openai" | "deepseek" | "ollama" | "custom";

export interface AiSettings {
  provider: AiProvider;
  baseUrl: string;
  model: string;
}

const STORAGE_KEY = "mydb.aiSettings.v1";

export const AI_PROVIDER_PRESETS: Record<AiProvider, AiSettings> = {
  anthropic: {
    provider: "anthropic",
    baseUrl: "https://api.anthropic.com",
    model: "claude-sonnet-5-5",
  },
  openai: {
    provider: "openai",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
  },
  deepseek: {
    provider: "deepseek",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  ollama: {
    provider: "ollama",
    baseUrl: "http://127.0.0.1:11434/v1",
    model: "llama3.1",
  },
  custom: {
    provider: "custom",
    baseUrl: "",
    model: "",
  },
};

export function loadAiSettings(): AiSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...AI_PROVIDER_PRESETS.anthropic };
    const parsed = JSON.parse(raw) as Partial<AiSettings>;
    const provider = parsed.provider;
    if (!provider || !(provider in AI_PROVIDER_PRESETS)) {
      return { ...AI_PROVIDER_PRESETS.anthropic };
    }
    return {
      provider,
      baseUrl: String(parsed.baseUrl ?? AI_PROVIDER_PRESETS[provider].baseUrl),
      model: String(parsed.model ?? AI_PROVIDER_PRESETS[provider].model),
    };
  } catch {
    return { ...AI_PROVIDER_PRESETS.anthropic };
  }
}

export function saveAiSettings(settings: AiSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Keep the current session setting when persistence is unavailable.
  }
}

export function aiProviderLabel(provider: AiProvider): string {
  return {
    anthropic: "Claude",
    openai: "OpenAI",
    deepseek: "DeepSeek",
    ollama: "Ollama（本地）",
    custom: "自定义 OpenAI 兼容",
  }[provider];
}
