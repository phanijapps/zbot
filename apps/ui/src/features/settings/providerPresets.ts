// ============================================================================
// PROVIDER PRESETS
// Pre-configured provider templates for quick setup
// ============================================================================

export interface ProviderPreset {
  name: string;
  baseUrl: string;
  models: string;
  apiKeyHint: string;
  apiKeyPlaceholder: string;
  /** If true, no API key is required (e.g., local Ollama) */
  noApiKey?: boolean;
  /** If true, shown in the top-3 prominent cards */
  featured?: boolean;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    models: "gpt-4o, gpt-4o-mini, o4-mini, gpt-4.1",
    apiKeyHint: "platform.openai.com/api-keys",
    apiKeyPlaceholder: "sk-...",
    featured: true,
  },
  {
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    models: "claude-sonnet-4-20250514, claude-opus-4-20250514",
    apiKeyHint: "console.anthropic.com/settings/keys",
    apiKeyPlaceholder: "sk-ant-...",
    featured: true,
  },
  {
    name: "Ollama",
    baseUrl: "http://localhost:11434/v1",
    models: "llama3.3, qwen2.5-coder, deepseek-r1, gemma3",
    apiKeyHint: "",
    apiKeyPlaceholder: "",
    noApiKey: true,
    featured: true,
  },
  {
    name: "Google Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    models: "gemini-2.5-pro, gemini-2.5-flash, gemini-2.0-flash",
    apiKeyHint: "aistudio.google.com/apikey",
    apiKeyPlaceholder: "AIza...",
  },
  {
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    models: "deepseek-chat, deepseek-reasoner",
    apiKeyHint: "platform.deepseek.com/api_keys",
    apiKeyPlaceholder: "sk-...",
  },
  {
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    models: "anthropic/claude-opus, openai/gpt-4-turbo, google/gemini-pro",
    apiKeyHint: "openrouter.ai/keys",
    apiKeyPlaceholder: "sk-or-...",
  },
  {
    name: "Z.AI",
    baseUrl: "https://api.z.ai/api/coding/paas/v4",
    models: "glm-5.1, glm-5, glm-4.7",
    apiKeyHint: "z.ai dashboard",
    apiKeyPlaceholder: "your-api-key",
  },
  {
    name: "Mistral",
    baseUrl: "https://api.mistral.ai/v1",
    models: "mistral-large-latest, mistral-small-latest, codestral-latest",
    apiKeyHint: "console.mistral.ai/api-keys",
    apiKeyPlaceholder: "your-api-key",
  },
  {
    name: "Ollama Cloud",
    baseUrl: "https://api.ollama.com/v1",
    models: "llama3.3, qwen2.5-coder, deepseek-r1",
    apiKeyHint: "ollama.com/settings",
    apiKeyPlaceholder: "your-api-key",
  },
];

/** Filter out presets where a provider with the same base URL already exists */
export function getAvailablePresets(
  existingProviders: { baseUrl: string; name: string }[]
): ProviderPreset[] {
  return PROVIDER_PRESETS.filter(
    (preset) =>
      !existingProviders.some(
        (p) =>
          p.baseUrl.replace(/\/+$/, "") === preset.baseUrl.replace(/\/+$/, "") ||
          p.name.toLowerCase() === preset.name.toLowerCase()
      )
  );
}

// Re-export from shared utility for backwards compat
export { formatContextWindow } from "@/shared/utils/format";
