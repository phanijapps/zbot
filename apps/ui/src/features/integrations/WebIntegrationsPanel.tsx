// ============================================================================
// WEB INTEGRATIONS PANEL
// Provider management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { Plug, Plus, Trash2, Key, Globe, Cpu, Check, X, Play, Loader2, Wrench, Eye, Brain, Volume2 } from "lucide-react";
import { getTransport, type ProviderResponse, type CreateProviderRequest, type ModelRegistryResponse, type ModelProfile } from "@/services/transport";

// ============================================================================
// Provider Presets
// ============================================================================

interface ProviderPreset {
  name: string;
  baseUrl: string;
  models: string;
  placeholder: string;
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  { name: "OpenAI", baseUrl: "https://api.openai.com/v1", models: "gpt-4o, gpt-4o-mini, o4-mini, gpt-4.1", placeholder: "sk-..." },
  { name: "Anthropic", baseUrl: "https://api.anthropic.com/v1", models: "claude-sonnet-4-20250514, claude-opus-4-20250514", placeholder: "sk-ant-..." },
  { name: "DeepSeek", baseUrl: "https://api.deepseek.com/v1", models: "deepseek-chat, deepseek-reasoner", placeholder: "sk-..." },
  { name: "Google Gemini", baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai", models: "gemini-2.5-pro, gemini-2.5-flash, gemini-2.0-flash", placeholder: "AIza..." },
  { name: "Ollama", baseUrl: "http://localhost:11434/v1", models: "llama3.3, qwen2.5-coder, deepseek-r1, gemma3", placeholder: "ollama (no key needed)" },
  { name: "Ollama Cloud", baseUrl: "https://api.ollama.com/v1", models: "llama3.3, qwen2.5-coder, deepseek-r1", placeholder: "your-api-key" },
  { name: "OpenRouter", baseUrl: "https://openrouter.ai/api/v1", models: "anthropic/claude-opus, openai/gpt-4-turbo, google/gemini-pro", placeholder: "sk-or-..." },
  { name: "Z.AI", baseUrl: "https://api.z.ai/api/coding/paas/v4", models: "glm-5.1, glm-5, glm-4.7", placeholder: "your-api-key" },
  { name: "Mistral", baseUrl: "https://api.mistral.ai/v1", models: "mistral-large-latest, mistral-small-latest, codestral-latest", placeholder: "your-api-key" },
];

/** Small capability badges for a model */
function ModelBadges({ profile }: { profile?: ModelProfile }) {
  if (!profile) return null;
  const caps = profile.capabilities;
  return (
    <span className="inline-flex gap-0.5">
      {caps.tools && <Wrench className="w-3 h-3 text-[var(--muted-foreground)]" />}
      {caps.vision && <Eye className="w-3 h-3 text-[var(--muted-foreground)]" />}
      {caps.thinking && <Brain className="w-3 h-3 text-[var(--muted-foreground)]" />}
      {caps.voice && <Volume2 className="w-3 h-3 text-[var(--muted-foreground)]" />}
    </span>
  );
}

// ============================================================================
// Component
// ============================================================================

export function WebIntegrationsPanel() {
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [modelRegistry, setModelRegistry] = useState<ModelRegistryResponse>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState<ProviderResponse | null>(null);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [isTesting, setIsTesting] = useState(false);

  const [newProvider, setNewProvider] = useState<Partial<CreateProviderRequest>>({
    name: "",
    description: "",
    apiKey: "",
    baseUrl: "https://api.openai.com/v1",
    models: [],
  });
  const [modelsInput, setModelsInput] = useState("");

  useEffect(() => {
    loadProviders();
  }, []);

  const loadProviders = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const [result, modelsResult] = await Promise.all([
        transport.listProviders(),
        transport.listModels(),
      ]);
      if (result.success && result.data) {
        setProviders(result.data);
      } else {
        setError(result.error || "Failed to load providers");
      }
      if (modelsResult.success && modelsResult.data) {
        setModelRegistry(modelsResult.data);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  };

  const handleSetDefault = async (providerId: string) => {
    try {
      const transport = await getTransport();
      const result = await transport.setDefaultProvider(providerId);
      if (result.success) {
        // Reload to get updated isDefault flags
        await loadProviders();
      } else {
        setError(result.error || "Failed to set default provider");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  // Get the current default provider (marked with isDefault)
  const defaultProvider = providers.find(p => p.isDefault);
  const defaultProviderId = defaultProvider?.id;

  const handleCreateProvider = async () => {
    if (!newProvider.name || !newProvider.apiKey || !newProvider.baseUrl) return;

    try {
      const transport = await getTransport();
      const models = modelsInput.split(",").map((m) => m.trim()).filter(Boolean);
      const result = await transport.createProvider({
        name: newProvider.name,
        description: newProvider.description || "",
        apiKey: newProvider.apiKey,
        baseUrl: newProvider.baseUrl,
        models: models.length > 0 ? models : ["gpt-4", "gpt-3.5-turbo"],
      });

      if (result.success && result.data) {
        // If this is the first provider, set it as default
        if (providers.length === 0 && result.data.id) {
          await handleSetDefault(result.data.id);
        } else {
          await loadProviders();
        }
        setIsCreating(false);
        setNewProvider({
          name: "",
          description: "",
          apiKey: "",
          baseUrl: "https://api.openai.com/v1",
          models: [],
        });
        setModelsInput("");
      } else {
        setError(result.error || "Failed to create provider");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleDeleteProvider = async (id: string) => {
    if (!confirm("Are you sure you want to delete this provider?")) return;

    try {
      const transport = await getTransport();
      const result = await transport.deleteProvider(id);
      if (result.success) {
        setSelectedProvider(null);
        await loadProviders();
      } else {
        setError(result.error || "Failed to delete provider");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleTestProvider = async (provider: ProviderResponse) => {
    setIsTesting(true);
    setTestResult(null);
    try {
      const transport = await getTransport();
      const result = await transport.testProvider({
        name: provider.name,
        description: provider.description,
        apiKey: provider.apiKey,
        baseUrl: provider.baseUrl,
        models: provider.models,
      });

      if (result.success && result.data) {
        setTestResult(result.data);
      } else {
        setTestResult({ success: false, message: result.error || "Test failed" });
      }
    } catch (err) {
      setTestResult({ success: false, message: err instanceof Error ? err.message : "Unknown error" });
    } finally {
      setIsTesting(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-[var(--background)]">
        <Loader2 className="w-6 h-6 text-[var(--primary)] animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex h-full bg-[var(--background)]">
      {/* Providers List */}
      <div className="w-72 bg-[var(--card)] border-r border-[var(--border)] flex flex-col">
        <div className="p-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Plug className="w-4 h-4 text-[var(--primary)]" />
            <h1 className="text-sm font-semibold text-[var(--foreground)]">Providers</h1>
          </div>
          <button
            onClick={() => setIsCreating(true)}
            className="inline-flex items-center gap-1 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-[var(--primary-foreground)] px-2.5 py-1.5 rounded-lg text-xs transition-colors font-medium"
          >
            <Plus className="w-3.5 h-3.5" />
            New
          </button>
        </div>

        {/* Current Default */}
        {defaultProvider && (
          <div className="px-3 py-2 bg-[var(--accent)]">
            <div className="text-xs text-[var(--primary)] mb-0.5 flex items-center gap-1">
              <Check className="w-3 h-3" />
              Active Provider
            </div>
            <div className="text-sm font-medium text-[var(--foreground)]">{defaultProvider.name}</div>
          </div>
        )}

        {error && (
          <div className="px-3 py-2 bg-[var(--destructive)]/10 text-[var(--destructive)] text-xs flex items-center justify-between">
            <span className="truncate">{error}</span>
            <button onClick={() => setError(null)} className="hover:opacity-70 ml-2">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        <div className="flex-1 overflow-auto">
          {providers.length === 0 ? (
            <div className="p-6 text-center">
              <div className="w-10 h-10 rounded-lg bg-[var(--primary)]/10 flex items-center justify-center mx-auto mb-3">
                <Plug className="w-5 h-5 text-[var(--primary)]" />
              </div>
              <p className="text-sm font-medium text-[var(--foreground)]">No providers configured</p>
              <p className="text-xs text-[var(--muted-foreground)] mt-1">Add an LLM provider to get started</p>
            </div>
          ) : (
            providers.map((provider) => (
              <button
                key={provider.id}
                onClick={() => {
                  setSelectedProvider(provider);
                  setTestResult(null);
                }}
                className={`w-full text-left px-3 py-2.5 hover:bg-[var(--muted)] transition-colors ${
                  selectedProvider?.id === provider.id
                    ? "bg-[var(--accent)] border-l-2 border-l-[var(--primary)]"
                    : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="text-sm font-medium text-[var(--foreground)]">{provider.name}</div>
                  {provider.id === defaultProviderId && (
                    <span className="text-[10px] bg-[var(--primary)] text-[var(--primary-foreground)] px-1.5 py-0.5 rounded font-medium">
                      Active
                    </span>
                  )}
                </div>
                <div className="text-xs text-[var(--muted-foreground)] truncate">{provider.baseUrl}</div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Provider Detail / Create Form */}
      <div className="flex-1 overflow-auto">
        {isCreating ? (
          <div className="p-8 max-w-lg">
            <div className="flex items-center gap-3 mb-5">
              <div className="w-9 h-9 rounded-lg bg-[var(--primary)]/10 flex items-center justify-center">
                <Plug className="w-4.5 h-4.5 text-[var(--primary)]" />
              </div>
              <h2 className="text-lg font-semibold text-[var(--foreground)]">Add Provider</h2>
            </div>

            {/* Quick presets */}
            <div className="mb-5">
              <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-2">Quick Setup</label>
              <div className="flex flex-wrap gap-1.5">
                {PROVIDER_PRESETS.map((preset) => (
                  <button
                    key={preset.name}
                    onClick={() => {
                      setNewProvider({
                        name: preset.name,
                        description: `${preset.name} API`,
                        apiKey: "",
                        baseUrl: preset.baseUrl,
                        models: [],
                      });
                      setModelsInput(preset.models);
                    }}
                    className="px-2.5 py-1 bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] rounded-lg text-xs font-medium transition-colors border border-[var(--border)]"
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
            </div>

            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Name</label>
                  <input
                    type="text"
                    value={newProvider.name}
                    onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })}
                    placeholder="OpenAI, Anthropic, etc."
                    className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Description</label>
                  <input
                    type="text"
                    value={newProvider.description}
                    onChange={(e) => setNewProvider({ ...newProvider, description: e.target.value })}
                    placeholder="Optional description"
                    className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">API Key</label>
                <input
                  type="password"
                  value={newProvider.apiKey}
                  onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })}
                  placeholder={PROVIDER_PRESETS.find(p => p.name === newProvider.name)?.placeholder || "sk-..."}
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Base URL</label>
                <input
                  type="text"
                  value={newProvider.baseUrl}
                  onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })}
                  placeholder="https://api.openai.com/v1"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Models (comma-separated)</label>
                <input
                  type="text"
                  value={modelsInput}
                  onChange={(e) => setModelsInput(e.target.value)}
                  placeholder="gpt-4o, gpt-4o-mini"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
                {modelsInput && (
                  <div className="flex flex-wrap gap-1.5 mt-2">
                    {modelsInput.split(",").map(m => m.trim()).filter(Boolean).map((model) => (
                      <span
                        key={model}
                        className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]"
                      >
                        {model}
                        <ModelBadges profile={modelRegistry[model]} />
                      </span>
                    ))}
                  </div>
                )}
              </div>

              <div className="flex gap-2 pt-2">
                <button
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors text-sm font-medium"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateProvider}
                  disabled={!newProvider.name || !newProvider.apiKey || !newProvider.baseUrl}
                  className="bg-[var(--primary)] hover:bg-[var(--primary)]/90 disabled:opacity-50 text-[var(--primary-foreground)] px-4 py-2 rounded-lg transition-colors text-sm font-medium"
                >
                  Add Provider
                </button>
              </div>
            </div>
          </div>
        ) : selectedProvider ? (
          <div className="p-8 max-w-md">
            <div className="flex items-start justify-between mb-5">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-lg bg-[var(--primary)]/10 flex items-center justify-center">
                  <Plug className="w-4.5 h-4.5 text-[var(--primary)]" />
                </div>
                <div>
                  <h2 className="text-lg font-semibold text-[var(--foreground)]">{selectedProvider.name}</h2>
                  <p className="text-xs text-[var(--muted-foreground)]">{selectedProvider.id}</p>
                </div>
              </div>
              <div className="flex gap-1.5">
                {selectedProvider.id !== defaultProviderId && (
                  <button
                    onClick={() => handleSetDefault(selectedProvider.id!)}
                    className="inline-flex items-center gap-1 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-[var(--primary-foreground)] px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors"
                  >
                    <Check className="w-3.5 h-3.5" />
                    Set Active
                  </button>
                )}
                <button
                  onClick={() => handleTestProvider(selectedProvider)}
                  disabled={isTesting}
                  className="inline-flex items-center gap-1 text-[var(--primary)] border border-[var(--primary)] hover:bg-[var(--accent)] px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors disabled:opacity-50"
                >
                  {isTesting ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Play className="w-3.5 h-3.5" />
                  )}
                  Test
                </button>
                <button
                  onClick={() => handleDeleteProvider(selectedProvider.id!)}
                  className="text-[var(--muted-foreground)] hover:text-[var(--destructive)] transition-colors p-1.5 hover:bg-[var(--destructive)]/10 rounded-lg"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>

            {selectedProvider.id === defaultProviderId && (
              <div className="mb-4 px-3 py-2 bg-[var(--primary)]/10 rounded-lg text-[var(--primary)] text-xs flex items-center gap-1.5">
                <Check className="w-3.5 h-3.5" />
                This is the active provider used by agents.
              </div>
            )}

            {testResult && (
              <div
                className={`mb-4 px-3 py-2 rounded-lg flex items-start gap-2 text-xs ${
                  testResult.success
                    ? "bg-[var(--success)]/10 text-[var(--success)]"
                    : "bg-[var(--destructive)]/10 text-[var(--destructive)]"
                }`}
              >
                {testResult.success ? (
                  <Check className="w-3.5 h-3.5 flex-shrink-0 mt-0.5" />
                ) : (
                  <X className="w-3.5 h-3.5 flex-shrink-0 mt-0.5" />
                )}
                {testResult.message}
              </div>
            )}

            <div className="space-y-3">
              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Description</label>
                <p className="text-sm text-[var(--foreground)]">{selectedProvider.description || "No description"}</p>
              </div>

              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-1 flex items-center gap-1">
                  <Globe className="w-3 h-3" />
                  Base URL
                </label>
                <code className="block bg-[var(--muted)] rounded-lg px-3 py-2 text-xs text-[var(--foreground)] font-mono">
                  {selectedProvider.baseUrl}
                </code>
              </div>

              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-1 flex items-center gap-1">
                  <Key className="w-3 h-3" />
                  API Key
                </label>
                <code className="block bg-[var(--muted)] rounded-lg px-3 py-2 text-xs text-[var(--foreground)] font-mono">
                  {selectedProvider.apiKey.slice(0, 8)}...{selectedProvider.apiKey.slice(-4)}
                </code>
              </div>

              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-2 flex items-center gap-1">
                  <Cpu className="w-3 h-3" />
                  Models
                </label>
                <div className="flex flex-wrap gap-1.5">
                  {selectedProvider.models.map((model) => (
                    <span
                      key={model}
                      className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]"
                    >
                      {model}
                      <ModelBadges profile={modelRegistry[model]} />
                    </span>
                  ))}
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-12 h-12 rounded-xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-3">
                <Plug className="w-6 h-6 text-[var(--muted-foreground)]" />
              </div>
              <p className="text-sm text-[var(--muted-foreground)]">Select a provider to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
