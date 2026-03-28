// ============================================================================
// WEB INTEGRATIONS PANEL (PROVIDERS PAGE)
// Provider management with card grid, slide-over detail, and guided onboarding
// ============================================================================

import { useState, useEffect } from "react";
import { Plug, Plus, Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse, ModelRegistryResponse } from "@/services/transport";
import { ProvidersEmptyState } from "./ProvidersEmptyState";
import { ProvidersGrid } from "./ProvidersGrid";
import { ProviderSlideover } from "./ProviderSlideover";
import type { ProviderPreset } from "./providerPresets";
import { getAvailablePresets } from "./providerPresets";

// ============================================================================
// Component
// ============================================================================

export function WebIntegrationsPanel() {
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [modelRegistry, setModelRegistry] = useState<ModelRegistryResponse>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Slide-over state
  const [slideoverOpen, setSlideoverOpen] = useState(false);
  const [slideoverMode, setSlideoverMode] = useState<"view" | "create">("view");
  const [selectedProvider, setSelectedProvider] = useState<ProviderResponse | null>(null);
  const [createPreset, setCreatePreset] = useState<ProviderPreset | null>(null);

  // Add-more state (shows preset grid when providers exist)
  const [showAddPresets, setShowAddPresets] = useState(false);

  const defaultProvider = providers.find((p) => p.isDefault);
  const defaultProviderId = defaultProvider?.id;

  // ---- Data Loading ----

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const [providersResult, modelsResult] = await Promise.all([
        transport.listProviders(),
        transport.listModels(),
      ]);
      if (providersResult.success && providersResult.data) {
        setProviders(providersResult.data);
      } else {
        setError(providersResult.error || "Failed to load providers");
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

  // ---- Actions ----

  const handleSelectProvider = (provider: ProviderResponse) => {
    setSelectedProvider(provider);
    setSlideoverMode("view");
    setCreatePreset(null);
    setSlideoverOpen(true);
  };

  const handleOpenCreate = (preset?: ProviderPreset) => {
    setSelectedProvider(null);
    setSlideoverMode("create");
    setCreatePreset(preset || null);
    setSlideoverOpen(true);
    setShowAddPresets(false);
  };

  const handleCloseSlider = () => {
    setSlideoverOpen(false);
    setSelectedProvider(null);
    setCreatePreset(null);
  };

  const handleProviderSaved = () => {
    handleCloseSlider();
    loadData();
  };

  const handleProviderDeleted = () => {
    handleCloseSlider();
    loadData();
  };

  const handleSetActive = async (id: string) => {
    try {
      const transport = await getTransport();
      const result = await transport.setDefaultProvider(id);
      if (result.success) {
        await loadData();
      } else {
        setError(result.error || "Failed to set active provider");
      }
    } catch {
      setError("Failed to set active provider");
    }
  };

  // ---- Render ----

  if (isLoading) {
    return (
      <div className="page" style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
        <Loader2 className="w-6 h-6 animate-spin" style={{ color: "var(--primary)" }} />
      </div>
    );
  }

  const hasProviders = providers.length > 0;
  const availablePresets = getAvailablePresets(providers);

  return (
    <div className="page">
      <div className="page-container">
        {hasProviders ? (
          <>
            {/* Page header */}
            <div className="page-header" style={{ marginBottom: "var(--spacing-5)" }}>
              <div>
                <h1 style={{ fontSize: "var(--text-lg)", fontWeight: 600, color: "var(--foreground)", display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
                  <Plug size={20} style={{ color: "var(--primary)" }} />
                  Providers
                </h1>
                <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", marginTop: 2 }}>
                  {providers.filter((p) => p.verified).length} connected · {defaultProvider ? "1 active" : "none active"}
                </p>
              </div>
              <button className="btn btn--primary btn--sm" onClick={() => availablePresets.length > 0 ? setShowAddPresets(!showAddPresets) : handleOpenCreate()}>
                <Plus size={14} /> Add Provider
              </button>
            </div>

            {/* Error */}
            {error && (
              <div className="alert alert--error" style={{ marginBottom: "var(--spacing-4)" }}>
                <span>{error}</span>
                <button className="btn btn--ghost btn--sm" onClick={() => setError(null)} style={{ marginLeft: "auto" }}>Dismiss</button>
              </div>
            )}

            {/* Add-more preset section */}
            {showAddPresets && (
              <div style={{ marginBottom: "var(--spacing-5)" }}>
                <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--spacing-2)", marginBottom: "var(--spacing-3)" }}>
                  {availablePresets.map((preset) => (
                    <button
                      key={preset.name}
                      className="btn btn--outline btn--sm"
                      onClick={() => handleOpenCreate(preset)}
                    >
                      {preset.name}
                    </button>
                  ))}
                  <button className="btn btn--ghost btn--sm" onClick={() => handleOpenCreate()}>
                    Custom...
                  </button>
                </div>
              </div>
            )}

            {/* Provider card grid */}
            <ProvidersGrid
              providers={providers}
              modelRegistry={modelRegistry}
              defaultProviderId={defaultProviderId}
              onSelect={handleSelectProvider}
            />

            {/* Add another link */}
            {availablePresets.length > 0 && !showAddPresets && (
              <div style={{ textAlign: "center", marginTop: "var(--spacing-4)" }}>
                <button className="btn btn--ghost btn--sm" onClick={() => setShowAddPresets(true)}>
                  <Plus size={14} /> Add another provider
                </button>
              </div>
            )}
          </>
        ) : (
          /* Empty state for new users */
          <ProvidersEmptyState
            existingProviders={providers}
            onProviderCreated={loadData}
            onOpenCustom={() => handleOpenCreate()}
          />
        )}
      </div>

      {/* Slide-over detail/edit/create panel */}
      <ProviderSlideover
        provider={selectedProvider}
        modelRegistry={modelRegistry}
        isActive={selectedProvider?.id === defaultProviderId}
        isOpen={slideoverOpen}
        mode={slideoverMode}
        preset={createPreset}
        onClose={handleCloseSlider}
        onSaved={handleProviderSaved}
        onDeleted={handleProviderDeleted}
        onSetActive={handleSetActive}
      />
    </div>
  );
}
