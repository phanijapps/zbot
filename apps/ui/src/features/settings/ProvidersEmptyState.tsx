// ============================================================================
// PROVIDERS EMPTY STATE
// Onboarding flow for new users — Top-3 presets with inline connect
// ============================================================================

import { useState } from "react";
import { Plug, Loader2, ChevronDown, ChevronUp, AlertCircle } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderPreset } from "./providerPresets";
import { getAvailablePresets, PROVIDER_PRESETS } from "./providerPresets";

interface ProvidersEmptyStateProps {
  existingProviders: { baseUrl: string; name: string }[];
  onProviderCreated: () => void;
  onOpenCustom: () => void;
}

export function ProvidersEmptyState({ existingProviders, onProviderCreated, onOpenCustom }: ProvidersEmptyStateProps) {
  const [expandedPreset, setExpandedPreset] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);

  const available = getAvailablePresets(existingProviders);
  const featured = available.filter((p) => p.featured);
  const rest = available.filter((p) => !p.featured);
  const visiblePresets = showAll ? available : featured;
  const hasMore = rest.length > 0;

  const handleConnect = async (preset: ProviderPreset) => {
    setIsConnecting(true);
    setError(null);

    const models = preset.models.split(",").map((m) => m.trim()).filter(Boolean);
    const key = preset.noApiKey ? "ollama" : apiKey;

    try {
      const transport = await getTransport();

      // Test first
      const testResult = await transport.testProvider({
        name: preset.name,
        description: `${preset.name} API`,
        apiKey: key,
        baseUrl: preset.baseUrl,
        models,
      });

      if (!testResult.success || !testResult.data?.success) {
        setError(testResult.data?.message || testResult.error || "Connection failed. Check your API key and try again.");
        setIsConnecting(false);
        return;
      }

      // Test passed — create the provider
      const createResult = await transport.createProvider({
        name: preset.name,
        description: `${preset.name} API`,
        apiKey: key,
        baseUrl: preset.baseUrl,
        models,
      });

      if (createResult.success && createResult.data) {
        // If this is the first provider, set as default
        if (existingProviders.length === 0 && createResult.data.id) {
          await transport.setDefaultProvider(createResult.data.id);
        }
        setExpandedPreset(null);
        setApiKey("");
        onProviderCreated();
      } else {
        setError(createResult.error || "Failed to save provider.");
      }
    } catch (err) {
      setError("Could not reach provider — check the URL and try again.");
    } finally {
      setIsConnecting(false);
    }
  };

  const handlePresetClick = (preset: ProviderPreset) => {
    if (preset.noApiKey) {
      // Ollama: connect directly, no API key needed
      handleConnect(preset);
      return;
    }
    if (expandedPreset === preset.name) {
      setExpandedPreset(null);
      setError(null);
    } else {
      setExpandedPreset(preset.name);
      setApiKey("");
      setError(null);
    }
  };

  return (
    <div className="empty-state" style={{ paddingTop: "12vh" }}>
      <div className="empty-state__icon">
        <Plug />
      </div>
      <h2 className="empty-state__title">Get Started</h2>
      <p className="empty-state__description">
        Connect an AI provider so your agents can think.
        <br />
        Most users start with one of these:
      </p>

      {/* Preset cards */}
      <div style={{ display: "flex", gap: "var(--spacing-4)", justifyContent: "center", marginTop: "var(--spacing-6)", flexWrap: "wrap" }}>
        {visiblePresets.map((preset) => (
          <div
            key={preset.name}
            className={`preset-card${expandedPreset && expandedPreset !== preset.name ? " preset-card--dimmed" : ""}`}
            style={{ width: 160 }}
            onClick={() => handlePresetClick(preset)}
            onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handlePresetClick(preset); } }}
            role="button"
            tabIndex={0}
          >
            <div className="preset-card__name">{preset.name}</div>
            <div className="preset-card__desc">
              {preset.noApiKey ? "Free, runs locally" : preset.models.split(",").slice(0, 2).join(", ")}
            </div>
            {preset.noApiKey && isConnecting && expandedPreset === null ? (
              <Loader2 className="w-4 h-4 animate-spin mx-auto" style={{ color: "var(--primary)" }} />
            ) : (
              <span className="btn btn--primary btn--sm" style={{ display: "inline-block" }}>Connect</span>
            )}
          </div>
        ))}
      </div>

      {/* Inline connect form */}
      {expandedPreset && (
        <div className="inline-connect" style={{ maxWidth: 400, margin: "0 auto" }}>
          {(() => {
            const preset = PROVIDER_PRESETS.find((p) => p.name === expandedPreset)!;
            return (
              <>
                <div className="form-group">
                  <label className="form-label">API Key</label>
                  {preset.apiKeyHint && (
                    <p style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)", marginBottom: "var(--spacing-2)" }}>
                      Get your key from <span style={{ color: "var(--primary)" }}>{preset.apiKeyHint}</span>
                    </p>
                  )}
                  <input
                    className="form-input"
                    type="password"
                    placeholder={preset.apiKeyPlaceholder}
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    autoFocus
                    onKeyDown={(e) => { if (e.key === "Enter" && apiKey) handleConnect(preset); }}
                  />
                </div>

                {error && (
                  <div className="alert alert--error" style={{ marginTop: "var(--spacing-3)" }}>
                    <AlertCircle size={14} />
                    <span>{error}</span>
                  </div>
                )}

                <div style={{ display: "flex", gap: "var(--spacing-2)", justifyContent: "flex-end", marginTop: "var(--spacing-4)" }}>
                  <button className="btn btn--ghost btn--sm" onClick={() => { setExpandedPreset(null); setError(null); }}>
                    Cancel
                  </button>
                  <button className="btn btn--ghost btn--sm" onClick={onOpenCustom}>
                    Advanced
                  </button>
                  <button
                    className="btn btn--primary btn--sm"
                    disabled={!apiKey || isConnecting}
                    onClick={() => handleConnect(preset)}
                  >
                    {isConnecting ? <Loader2 className="w-4 h-4 animate-spin" /> : "Test & Connect"}
                  </button>
                </div>
              </>
            );
          })()}
        </div>
      )}

      {/* Show more / custom */}
      <div style={{ marginTop: "var(--spacing-5)", fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
        {hasMore && !showAll && (
          <button className="btn btn--ghost btn--sm" onClick={() => setShowAll(true)}>
            <ChevronDown size={14} /> Show {rest.length} more providers
          </button>
        )}
        {showAll && hasMore && (
          <button className="btn btn--ghost btn--sm" onClick={() => setShowAll(false)}>
            <ChevronUp size={14} /> Show less
          </button>
        )}
        {" · "}
        <button className="btn btn--ghost btn--sm" onClick={onOpenCustom}>
          Custom provider
        </button>
      </div>
    </div>
  );
}
