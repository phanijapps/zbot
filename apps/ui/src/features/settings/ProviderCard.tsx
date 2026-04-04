// ============================================================================
// PROVIDER CARD
// Displays a configured provider in the card grid
// ============================================================================

import type { ProviderResponse, ModelRegistryResponse } from "@/services/transport";
import { ModelChip } from "@/shared/ui/ModelChip";

interface ProviderCardProps {
  provider: ProviderResponse;
  modelRegistry: ModelRegistryResponse;
  isActive: boolean;
  onClick: () => void;
}

/** Aggregate capabilities across all models for a provider */
function getProviderCapabilities(
  provider: ProviderResponse,
  modelRegistry: ModelRegistryResponse,
): { tools: boolean; vision: boolean; thinking: boolean; embeddings: boolean } {
  const caps = { tools: false, vision: false, thinking: false, embeddings: false };

  if (provider.modelConfigs) {
    for (const config of Object.values(provider.modelConfigs)) {
      if (config.capabilities.tools) caps.tools = true;
      if (config.capabilities.vision) caps.vision = true;
      if (config.capabilities.thinking) caps.thinking = true;
      if (config.capabilities.embeddings) caps.embeddings = true;
    }
    return caps;
  }

  for (const modelId of provider.models) {
    const profile = modelRegistry[modelId];
    if (profile?.capabilities) {
      if (profile.capabilities.tools) caps.tools = true;
      if (profile.capabilities.vision) caps.vision = true;
      if (profile.capabilities.thinking) caps.thinking = true;
      if (profile.capabilities.embeddings) caps.embeddings = true;
    }
  }
  return caps;
}

/** Shorten a base URL for display: "https://api.openai.com/v1" → "api.openai.com" */
function shortenUrl(url: string): string {
  try {
    const parsed = new URL(url);
    return parsed.hostname;
  } catch {
    return url;
  }
}

export function ProviderCard({ provider, modelRegistry, isActive, onClick }: ProviderCardProps) {
  const isVerified = provider.verified === true;
  const maxVisible = 3;
  const visibleModels = provider.models.slice(0, maxVisible);
  const hiddenCount = provider.models.length - maxVisible;

  return (
    <div
      className={`provider-card${isActive ? " provider-card--active" : ""}`}
      onClick={onClick}
      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onClick(); } }}
      role="button"
      tabIndex={0}
    >
      <div className="provider-card__status">
        {isVerified ? (
          <span className="badge badge--success">Connected</span>
        ) : (
          <span className="badge badge--warning">Not tested</span>
        )}
        {isActive && <span className="badge badge--primary">Active</span>}
      </div>

      <div className="provider-card__name">{provider.name}</div>
      <div className="provider-card__url">
        {shortenUrl(provider.baseUrl)}
        {provider.models.length > 0 && <span> · {provider.models.length} model{provider.models.length !== 1 ? "s" : ""}</span>}
      </div>

      <div className="provider-card__models">
        {visibleModels.map((model) => (
          <ModelChip key={model} modelId={model} profile={modelRegistry[model]} />
        ))}
        {hiddenCount > 0 && (
          <span className="model-chip">+{hiddenCount} more</span>
        )}
        {provider.models.length === 0 && (
          <span className="model-chip">No models</span>
        )}
      </div>
      {(() => {
        const caps = getProviderCapabilities(provider, modelRegistry);
        const badges: { label: string; cls: string }[] = [];
        if (caps.tools) badges.push({ label: "Tools", cls: "cap-badge--tools" });
        if (caps.vision) badges.push({ label: "Vision", cls: "cap-badge--vision" });
        if (caps.thinking) badges.push({ label: "Thinking", cls: "cap-badge--thinking" });
        if (caps.embeddings) badges.push({ label: "Embeddings", cls: "cap-badge--embed" });
        if (badges.length === 0) return null;
        return (
          <div className="provider-card__capabilities">
            {badges.map((b) => (
              <span key={b.label} className={`cap-badge ${b.cls}`}>{b.label}</span>
            ))}
          </div>
        );
      })()}
    </div>
  );
}
