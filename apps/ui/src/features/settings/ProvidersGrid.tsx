// ============================================================================
// PROVIDERS GRID
// Responsive card grid of configured providers
// ============================================================================

import type { ProviderResponse, ModelRegistryResponse } from "@/services/transport";
import { ProviderCard } from "./ProviderCard";

interface ProvidersGridProps {
  providers: ProviderResponse[];
  modelRegistry: ModelRegistryResponse;
  defaultProviderId?: string;
  onSelect: (provider: ProviderResponse) => void;
}

export function ProvidersGrid({ providers, modelRegistry, defaultProviderId, onSelect }: ProvidersGridProps) {
  return (
    <div className="provider-grid">
      {providers.map((provider) => (
        <ProviderCard
          key={provider.id}
          provider={provider}
          modelRegistry={modelRegistry}
          isActive={provider.id === defaultProviderId}
          onClick={() => onSelect(provider)}
        />
      ))}
    </div>
  );
}
