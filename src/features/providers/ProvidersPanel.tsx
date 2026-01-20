// ============================================================================
// PROVIDERS FEATURE
// OpenAI-compatible provider management
// ============================================================================

import { useState, useEffect } from "react";
import { Brain, Plus, Trash2, Check, Loader2, RefreshCw, Edit } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { AddProviderDialog } from "./AddProviderDialog";
import * as providerService from "@/services/provider";
import type { Provider } from "@/shared/types";

export function ProvidersPanel() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // Load providers on mount
  useEffect(() => {
    loadProviders();
  }, []);

  const loadProviders = async () => {
    setLoading(true);
    try {
      const loaded = await providerService.listProviders();
      setProviders(loaded);
    } catch (error) {
      console.error("Failed to load providers:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadProviders();
    setRefreshing(false);
  };

  const handleOpenCreateDialog = () => {
    setEditingProvider(null);
    setShowAddDialog(true);
  };

  const handleOpenEditDialog = (provider: Provider) => {
    setEditingProvider(provider);
    setShowAddDialog(true);
  };

  const handleSaveProvider = async (provider: Omit<Provider, "id" | "createdAt">) => {
    if (editingProvider) {
      await providerService.updateProvider(editingProvider.id, provider);
    } else {
      await providerService.createProvider(provider);
    }
    await loadProviders();
  };

  const handleDeleteProvider = async (id: string) => {
    if (confirm("Are you sure you want to delete this provider?")) {
      try {
        await providerService.deleteProvider(id);
        await loadProviders();
      } catch (error) {
        console.error("Failed to delete provider:", error);
      }
    }
  };

  return (
    <>
      <div className="p-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-white">Providers</h2>
            <p className="text-gray-400 text-sm mt-1">
              OpenAI-compatible API providers
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              className="border-white/20 text-white hover:bg-white/5"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              <RefreshCw className={`size-4 ${refreshing ? "animate-spin" : ""}`} />
            </Button>
            <Button
              className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
              onClick={handleOpenCreateDialog}
            >
              <Plus className="size-4 mr-2" />
              Add Provider
            </Button>
          </div>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="size-8 text-white animate-spin" />
          </div>
        ) : providers.length === 0 ? (
          <div className="text-center py-20">
            <Brain className="size-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-xl font-medium text-white mb-2">No Providers</h3>
            <p className="text-gray-400">Add your first API provider to get started</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {providers.map((provider) => (
              <div
                key={provider.id}
                className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-xl p-5 border border-white/10 hover:border-white/20 transition-all"
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-start gap-3">
                    <div className="p-2.5 rounded-xl bg-gradient-to-br from-blue-500 to-purple-600">
                      <Brain className="size-4 text-white" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2 mb-1">
                        <h3 className="text-white font-semibold">{provider.name}</h3>
                        {provider.verified && (
                          <Badge className="bg-green-500/20 text-green-300 border-green-500/30 text-xs">
                            <Check className="size-3 mr-1" />
                            Verified
                          </Badge>
                        )}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleOpenEditDialog(provider)}
                      className="text-gray-400 hover:text-white h-7 w-7 p-0"
                    >
                      <Edit className="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleDeleteProvider(provider.id)}
                      className="text-gray-400 hover:text-red-400 h-7 w-7 p-0"
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                </div>

                <p className="text-gray-400 text-sm mb-3">{provider.description}</p>

                {/* Base URL */}
                <div className="bg-black/30 rounded-lg p-2.5 mb-3 border border-white/5">
                  <p className="text-xs text-gray-500 mb-1">Base URL</p>
                  <code className="text-xs text-gray-300 font-mono block truncate">
                    {provider.baseUrl}
                  </code>
                </div>

                {/* Models */}
                <div className="flex flex-wrap gap-1.5 mb-2">
                  {provider.models.slice(0, 4).map((model) => (
                    <span
                      key={model}
                      className="px-2 py-0.5 bg-purple-500/10 rounded-full text-xs text-purple-300 border border-purple-500/20"
                    >
                      {model.length > 20 ? model.substring(0, 20) + "..." : model}
                    </span>
                  ))}
                  {provider.models.length > 4 && (
                    <span className="px-2 py-0.5 bg-gray-500/10 rounded-full text-xs text-gray-400 border border-gray-500/20">
                      +{provider.models.length - 4} more
                    </span>
                  )}
                </div>

                {/* Embedding Models */}
                {provider.embeddingModels && provider.embeddingModels.length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {provider.embeddingModels.slice(0, 3).map((model) => (
                      <span
                        key={model}
                        className="px-2 py-0.5 bg-green-500/10 rounded-full text-xs text-green-300 border border-green-500/20 flex items-center gap-1"
                      >
                        <Brain className="size-2.5" />
                        {model.length > 15 ? model.substring(0, 15) + "..." : model}
                      </span>
                    ))}
                    {provider.embeddingModels.length > 3 && (
                      <span className="px-2 py-0.5 bg-green-500/10 rounded-full text-xs text-green-400 border border-green-500/20">
                        +{provider.embeddingModels.length - 3} more
                      </span>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {/* Info Box */}
        <div className="mt-6 bg-blue-500/10 border border-blue-500/20 rounded-xl p-4">
          <div className="flex items-start gap-3">
            <Brain className="size-5 text-blue-400 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="text-sm font-medium text-blue-200 mb-2">
                About Providers
              </p>
              <p className="text-xs text-blue-300">
                Configure OpenAI-compatible API providers. These providers are used to power AI agents in your conversations.
              </p>
              <p className="text-xs text-blue-300 mt-2">
                💾 Configuration saved to: <code className="bg-white/10 px-1.5 py-0.5 rounded">~/.config/zeroagent/providers.json</code>
              </p>
            </div>
          </div>
        </div>
      </div>

      <AddProviderDialog
        open={showAddDialog}
        onClose={() => setShowAddDialog(false)}
        onSave={handleSaveProvider}
        editingProvider={editingProvider}
      />
    </>
  );
}
