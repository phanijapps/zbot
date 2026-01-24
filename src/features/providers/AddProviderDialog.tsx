// ============================================================================
// ADD PROVIDER DIALOG
// Dialog for adding/editing OpenAI-compatible providers
// ============================================================================

import { useState, useEffect } from "react";
import { Brain, Key, Globe, Server, Save, Loader2, AlertCircle } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import type { Provider, ProviderTestResult } from "@/shared/types";
import * as providerService from "@/services/provider";
import { useVaults } from "@/features/vaults/useVaults";

interface AddProviderDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (provider: Omit<Provider, "id" | "createdAt">) => void;
  editingProvider?: Provider | null;
}

interface ProviderPreset {
  name: string;
  baseUrl: string;
  description: string;
  models: string;
  embeddingModels?: string;
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    description: "Official OpenAI API",
    models: "gpt-4o, gpt-4-turbo, gpt-4, gpt-3.5-turbo",
    embeddingModels: "text-embedding-3-small, text-embedding-3-large",
  },
  {
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    description: "DeepSeek AI API",
    models: "deepseek-chat, deepseek-coder",
  },
  {
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    description: "OpenRouter - Access to multiple LLMs",
    models: "anthropic/claude-opus, openai/gpt-4-turbo, google/gemini-pro",
  },
  {
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    description: "Groq Lightning-Fast LLM API",
    models: "mixtral-8x7b-32768, llama2-70b-4096, gemma-7b-it",
  },
  {
    name: "Together AI",
    baseUrl: "https://api.together.xyz/v1",
    description: "Together AI Platform",
    models: "mixtral-8x7b, llama-2-70b",
  },
  {
    name: "Azure OpenAI",
    baseUrl: "https://YOUR-RESOURCE.openai.azure.com/openai/deployments/YOUR-DEPLOYMENT",
    description: "Microsoft Azure OpenAI Service",
    models: "gpt-4, gpt-35-turbo",
  },
];

export function AddProviderDialog({ open, onClose, onSave, editingProvider }: AddProviderDialogProps) {
  const { currentVault } = useVaults();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("https://api.openai.com/v1");
  const [models, setModels] = useState("");
  const [embeddingModels, setEmbeddingModels] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  // Populate form when editing an existing provider
  useEffect(() => {
    if (editingProvider) {
      setName(editingProvider.name);
      setDescription(editingProvider.description);
      setApiKey(editingProvider.apiKey);
      setBaseUrl(editingProvider.baseUrl);
      setModels(editingProvider.models.join(", "));
      setEmbeddingModels(editingProvider.embeddingModels?.join(", ") || "");
      setTestResult(null);
    } else {
      setName("");
      setDescription("");
      setApiKey("");
      setBaseUrl("https://api.openai.com/v1");
      setModels("");
      setEmbeddingModels("");
      setTestResult(null);
    }
  }, [editingProvider, open]);

  const handlePresetSelect = (preset: ProviderPreset) => {
    setName(preset.name);
    setBaseUrl(preset.baseUrl);
    setDescription(preset.description);
    setModels(preset.models);
    setEmbeddingModels(preset.embeddingModels || "");
    setTestResult(null);
  };

  const handleTestConnection = async () => {
    if (!apiKey || !baseUrl) return;

    setTesting(true);
    setTestResult(null);
    try {
      const provider: Omit<Provider, "id" | "createdAt"> = {
        name,
        description,
        apiKey,
        baseUrl,
        models: models.split(",").map((m) => m.trim()),
        embeddingModels: embeddingModels ? embeddingModels.split(",").map((m) => m.trim()) : undefined,
      };
      const result = await providerService.testProvider(provider);
      setTestResult(result);
    } catch (error) {
      setTestResult({
        success: false,
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const provider: Omit<Provider, "id" | "createdAt"> = {
        name,
        description,
        apiKey,
        baseUrl,
        models: models.split(",").map((m) => m.trim()),
        embeddingModels: embeddingModels ? embeddingModels.split(",").map((m) => m.trim()) : undefined,
        verified: testResult?.success ?? false,
      };
      await onSave(provider);

      // Reset form
      setName("");
      setDescription("");
      setApiKey("");
      setBaseUrl("https://api.openai.com/v1");
      setModels("");
      setEmbeddingModels("");
      setTestResult(null);
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  const isValid = name && description && apiKey && baseUrl && models;

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-2xl font-bold flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
              <Brain className="size-6 text-white" />
            </div>
            {editingProvider ? "Edit Provider" : "Add OpenAI Compatible Provider"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-6 mt-4">
          {/* Presets - only show when adding new provider */}
          {!editingProvider && (
            <div>
              <Label className="text-white mb-3 block">Quick Presets</Label>
              <div className="grid grid-cols-2 gap-2">
                {PROVIDER_PRESETS.map((preset) => (
                  <button
                    key={preset.name}
                    onClick={() => handlePresetSelect(preset)}
                    className="p-3 bg-white/5 hover:bg-white/10 border border-white/10 hover:border-white/20 rounded-lg text-left transition-all group"
                  >
                    <div className="flex items-center gap-2 mb-1">
                      <Server className="size-4 text-blue-400" />
                      <p className="text-sm font-medium text-white">{preset.name}</p>
                    </div>
                    <p className="text-xs text-gray-400">{preset.description}</p>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Provider Details */}
          <div className="space-y-4">
            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Brain className="size-4 text-purple-400" />
                Provider Name
              </Label>
              <Input
                placeholder="e.g., OpenAI, DeepSeek"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
            </div>

            <div>
              <Label className="text-white mb-2 block">Description</Label>
              <Input
                placeholder="Brief description of this provider"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
            </div>

            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Globe className="size-4 text-blue-400" />
                Base URL
              </Label>
              <Input
                placeholder="https://api.openai.com/v1"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500 font-mono text-sm"
              />
              <p className="text-xs text-gray-500 mt-1">OpenAI compatible API endpoint</p>
            </div>

            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Key className="size-4 text-yellow-400" />
                API Key
              </Label>
              <Input
                type="password"
                placeholder="sk-..."
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500 font-mono text-sm"
              />
              <div className="flex items-start gap-2 mt-2 p-2 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
                <AlertCircle className="size-4 text-yellow-400 shrink-0 mt-0.5" />
                <p className="text-xs text-yellow-200">
                  💾 API keys are stored locally in: <code className="text-yellow-300">{currentVault?.path || "~/.config/zeroagent"}/providers.json</code>
                </p>
              </div>
            </div>

            <div>
              <Label className="text-white mb-2 block">Available Models</Label>
              <Input
                placeholder="gpt-4o, gpt-4-turbo, gpt-3.5-turbo"
                value={models}
                onChange={(e) => setModels(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
              <p className="text-xs text-gray-500 mt-1">Comma-separated list of model names</p>
            </div>

            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Key className="size-4 text-green-400" />
                Embedding Models
                <span className="text-[10px] px-1.5 py-0.5 bg-white/10 rounded text-gray-400">Optional</span>
              </Label>
              <Input
                placeholder="text-embedding-3-small, text-embedding-3-large"
                value={embeddingModels}
                onChange={(e) => setEmbeddingModels(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
              <p className="text-xs text-gray-500 mt-1">For vector search and memory management</p>
            </div>
          </div>

          {/* Test Connection */}
          <div className="bg-white/5 border border-white/10 rounded-xl p-4">
            <p className="text-xs text-gray-400 mb-3">Connection Test</p>
            <Button
              variant="outline"
              className="w-full border-white/20 text-white hover:bg-white/5"
              disabled={!apiKey || !baseUrl || testing}
              onClick={handleTestConnection}
            >
              {testing ? (
                <>
                  <Loader2 className="size-4 mr-2 animate-spin" />
                  Testing...
                </>
              ) : (
                "Test Connection"
              )}
            </Button>
            {testResult && (
              <div className={`mt-3 p-3 rounded-lg ${testResult.success ? "bg-green-500/10 border border-green-500/20" : "bg-red-500/10 border border-red-500/20"}`}>
                <p className={`text-sm ${testResult.success ? "text-green-200" : "text-red-200"}`}>
                  {testResult.success ? "✓" : "✗"} {testResult.message}
                </p>
                {testResult.models && (
                  <p className="text-xs text-gray-400 mt-2">
                    Models found: {testResult.models.length}
                  </p>
                )}
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="flex gap-3 pt-4">
            <Button
              onClick={onClose}
              variant="outline"
              className="flex-1 border-white/20 text-white hover:bg-white/5"
              disabled={isSaving}
            >
              Cancel
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isValid || isSaving}
              className="flex-1 bg-gradient-to-br from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
            >
              {isSaving ? (
                <>
                  <Loader2 className="size-4 mr-2 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Save className="size-4 mr-2" />
                  {editingProvider ? "Update Provider" : "Save Provider"}
                </>
              )}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
