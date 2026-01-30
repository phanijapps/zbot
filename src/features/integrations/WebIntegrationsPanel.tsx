// ============================================================================
// WEB INTEGRATIONS PANEL
// Provider management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { getTransport, type ProviderResponse, type CreateProviderRequest } from "@/services/transport";

// ============================================================================
// Component
// ============================================================================

export function WebIntegrationsPanel() {
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
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
      const result = await transport.listProviders();
      if (result.success && result.data) {
        setProviders(result.data);
      } else {
        setError(result.error || "Failed to load providers");
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
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500" />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Providers List */}
      <div className="w-80 border-r border-gray-800 flex flex-col">
        <div className="p-4 border-b border-gray-800 flex items-center justify-between">
          <h1 className="text-lg font-bold">Providers</h1>
          <button
            onClick={() => setIsCreating(true)}
            className="bg-violet-600 hover:bg-violet-700 text-white px-3 py-1 rounded text-sm transition-colors"
          >
            New
          </button>
        </div>

        {/* Current Default */}
        {defaultProvider && (
          <div className="p-3 bg-violet-900/20 border-b border-violet-800">
            <div className="text-xs text-violet-400 mb-1">Active Provider</div>
            <div className="font-medium">{defaultProvider.name}</div>
          </div>
        )}

        {error && (
          <div className="p-3 bg-red-900/30 border-b border-red-800 text-red-200 text-sm">
            {error}
            <button onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-300">
              Dismiss
            </button>
          </div>
        )}

        <div className="flex-1 overflow-auto">
          {providers.length === 0 ? (
            <div className="p-4 text-center text-gray-500">
              <p>No providers configured</p>
              <p className="text-sm mt-1">Add an LLM provider to get started</p>
            </div>
          ) : (
            providers.map((provider) => (
              <button
                key={provider.id}
                onClick={() => {
                  setSelectedProvider(provider);
                  setTestResult(null);
                }}
                className={`w-full text-left px-4 py-3 border-b border-gray-800 hover:bg-gray-800/50 transition-colors ${
                  selectedProvider?.id === provider.id
                    ? "bg-violet-500/10 border-l-2 border-l-violet-500"
                    : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="font-medium">{provider.name}</div>
                  {provider.id === defaultProviderId && (
                    <span className="text-xs bg-violet-600 text-white px-1.5 py-0.5 rounded">
                      Active
                    </span>
                  )}
                </div>
                <div className="text-sm text-gray-500 truncate">{provider.baseUrl}</div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Provider Detail / Create Form */}
      <div className="flex-1 overflow-auto">
        {isCreating ? (
          <div className="p-6">
            <h2 className="text-xl font-bold mb-4">Add Provider</h2>

            <div className="space-y-4 max-w-lg">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Name</label>
                <input
                  type="text"
                  value={newProvider.name}
                  onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })}
                  placeholder="OpenAI, Anthropic, etc."
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Description</label>
                <input
                  type="text"
                  value={newProvider.description}
                  onChange={(e) => setNewProvider({ ...newProvider, description: e.target.value })}
                  placeholder="Optional description"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">API Key</label>
                <input
                  type="password"
                  value={newProvider.apiKey}
                  onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })}
                  placeholder="sk-..."
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Base URL</label>
                <input
                  type="text"
                  value={newProvider.baseUrl}
                  onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })}
                  placeholder="https://api.openai.com/v1"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Models (comma-separated)</label>
                <input
                  type="text"
                  value={modelsInput}
                  onChange={(e) => setModelsInput(e.target.value)}
                  placeholder="gpt-4, gpt-3.5-turbo"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div className="flex gap-3">
                <button
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateProvider}
                  disabled={!newProvider.name || !newProvider.apiKey || !newProvider.baseUrl}
                  className="bg-violet-600 hover:bg-violet-700 disabled:opacity-50 text-white px-4 py-2 rounded-lg transition-colors"
                >
                  Add Provider
                </button>
              </div>
            </div>
          </div>
        ) : selectedProvider ? (
          <div className="p-6">
            <div className="flex items-start justify-between mb-6">
              <div>
                <h2 className="text-xl font-bold">{selectedProvider.name}</h2>
                <p className="text-gray-500">{selectedProvider.id}</p>
              </div>
              <div className="flex gap-2">
                {selectedProvider.id !== defaultProviderId && (
                  <button
                    onClick={() => handleSetDefault(selectedProvider.id!)}
                    className="bg-violet-600 hover:bg-violet-700 text-white px-3 py-1 rounded transition-colors"
                  >
                    Set as Active
                  </button>
                )}
                <button
                  onClick={() => handleTestProvider(selectedProvider)}
                  disabled={isTesting}
                  className="text-violet-400 hover:text-violet-300 px-3 py-1 border border-violet-600 rounded transition-colors disabled:opacity-50"
                >
                  {isTesting ? "Testing..." : "Test"}
                </button>
                <button
                  onClick={() => handleDeleteProvider(selectedProvider.id!)}
                  className="text-gray-500 hover:text-red-400 transition-colors"
                >
                  Delete
                </button>
              </div>
            </div>

            {selectedProvider.id === defaultProviderId && (
              <div className="mb-4 p-3 bg-violet-900/20 border border-violet-800 rounded-lg text-violet-200 text-sm">
                This is the active provider used by the agent.
              </div>
            )}

            {testResult && (
              <div
                className={`mb-4 p-3 rounded-lg ${
                  testResult.success
                    ? "bg-green-900/30 border border-green-800 text-green-200"
                    : "bg-red-900/30 border border-red-800 text-red-200"
                }`}
              >
                {testResult.message}
              </div>
            )}

            <div className="space-y-4 max-w-lg">
              <div>
                <label className="block text-sm text-gray-500 mb-1">Description</label>
                <p className="text-gray-300">{selectedProvider.description || "No description"}</p>
              </div>

              <div>
                <label className="block text-sm text-gray-500 mb-1">Base URL</label>
                <code className="block bg-gray-900 rounded px-3 py-2 text-sm text-gray-300">
                  {selectedProvider.baseUrl}
                </code>
              </div>

              <div>
                <label className="block text-sm text-gray-500 mb-1">API Key</label>
                <code className="block bg-gray-900 rounded px-3 py-2 text-sm text-gray-300">
                  {selectedProvider.apiKey.slice(0, 8)}...{selectedProvider.apiKey.slice(-4)}
                </code>
              </div>

              <div>
                <label className="block text-sm text-gray-500 mb-1">Models</label>
                <div className="flex flex-wrap gap-2">
                  {selectedProvider.models.map((model) => (
                    <span
                      key={model}
                      className="bg-gray-800 px-2 py-1 rounded text-sm text-gray-300"
                    >
                      {model}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>Select a provider to view details</p>
          </div>
        )}
      </div>
    </div>
  );
}
