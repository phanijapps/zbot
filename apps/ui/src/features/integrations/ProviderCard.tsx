// ============================================================================
// PROVIDER CARD
// Displays a configured provider in the card grid
// ============================================================================

import type { ProviderResponse, ModelRegistryResponse } from "@/services/transport";
import { ModelChip } from "./ModelChip";

interface ProviderCardProps {
  provider: ProviderResponse;
  modelRegistry: ModelRegistryResponse;
  isActive: boolean;
  onClick: () => void;
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
      <div className="provider-card__url">{shortenUrl(provider.baseUrl)}</div>

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
    </div>
  );
}
