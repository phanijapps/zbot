import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse, AgentResponse } from "@/services/transport";

interface GlobalDefault {
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

interface AgentOverride {
  providerId?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
}

interface AgentsStepProps {
  providers: ProviderResponse[];
  defaultProviderId: string;
  agentName: string;
  globalDefault: GlobalDefault;
  agentOverrides: Record<string, AgentOverride>;
  onGlobalChange: (defaults: GlobalDefault) => void;
  onOverrideChange: (overrides: Record<string, AgentOverride>) => void;
}

export function AgentsStep({
  providers,
  defaultProviderId,
  agentName,
  globalDefault,
  agentOverrides,
  onGlobalChange,
  onOverrideChange,
}: AgentsStepProps) {
  const [agents, setAgents] = useState<AgentResponse[]>([]);
  const [freshProviders, setFreshProviders] = useState<ProviderResponse[]>(providers);
  const [isLoading, setIsLoading] = useState(true);
  const [expandedAgent, setExpandedAgent] = useState<string | null>(null);

  // Use fresh providers (with models) for dropdowns
  const activeProviders = freshProviders.length > 0 ? freshProviders : providers;

  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const [agentsRes, providersRes] = await Promise.all([
          transport.listAgents(),
          transport.listProviders(), // Refresh to get enriched models
        ]);

        if (agentsRes.success && agentsRes.data) {
          setAgents(agentsRes.data);
        }

        // Use refreshed providers (enriched with model lists)
        let resolvedProviders = providers;
        if (providersRes.success && providersRes.data && providersRes.data.length > 0) {
          resolvedProviders = providersRes.data;
          setFreshProviders(resolvedProviders);
        }

        // Ensure global default has a valid provider and model
        if (resolvedProviders.length > 0) {
          const pid = globalDefault.providerId || defaultProviderId;
          const matchedProvider = resolvedProviders.find((p) => p.id === pid);
          const provider = matchedProvider || resolvedProviders[0];
          const providerModels = provider.models || [];
          const currentModelValid = globalDefault.model && providerModels.includes(globalDefault.model);

          if (!matchedProvider || !globalDefault.providerId || !currentModelValid) {
            onGlobalChange({
              providerId: provider.id!,
              model: currentModelValid ? globalDefault.model : (provider.defaultModel || providerModels[0] || ""),
              temperature: globalDefault.temperature || 0.7,
              maxTokens: globalDefault.maxTokens || 4096,
            });
          }
        }
      } finally {
        setIsLoading(false);
      }
    };
    load();
  }, []);

  const selectedProvider = activeProviders.find((p) => p.id === globalDefault.providerId);
  const globalModels = selectedProvider?.models || [];

  // Sync: if current model isn't in the provider's model list, select first available
  useEffect(() => {
    if (!isLoading && globalModels.length > 0 && !globalModels.includes(globalDefault.model)) {
      onGlobalChange({ ...globalDefault, model: globalModels[0] });
    }
  }, [isLoading, globalDefault.providerId, globalModels.length]);

  if (isLoading) {
    return <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>;
  }

  const getEffectiveConfig = (agentId: string) => {
    const override = agentOverrides[agentId];
    if (!override) return globalDefault;
    return {
      providerId: override.providerId || globalDefault.providerId,
      model: override.model || globalDefault.model,
      temperature: override.temperature ?? globalDefault.temperature,
      maxTokens: override.maxTokens ?? globalDefault.maxTokens,
    };
  };

  const getProviderModels = (providerId: string) => {
    return activeProviders.find((p) => p.id === providerId)?.models || [];
  };

  const getProviderName = (providerId: string) => {
    return activeProviders.find((p) => p.id === providerId)?.name || providerId;
  };

  const handleOverride = (agentId: string, field: string, value: string | number) => {
    const current = agentOverrides[agentId] || {};
    onOverrideChange({
      ...agentOverrides,
      [agentId]: { ...current, [field]: value },
    });
  };

  const handleResetOverride = (agentId: string) => {
    const { [agentId]: _, ...rest } = agentOverrides;
    onOverrideChange(rest);
    setExpandedAgent(null);
  };

  const rootAgent = agents.find((a) => a.name === "root");
  const specialists = agents.filter((a) => a !== rootAgent).sort((a, b) => a.name.localeCompare(b.name));
  const sortedAgents = rootAgent ? [rootAgent, ...specialists] : specialists;

  return (
    <div>
      {/* Global default card */}
      <div className="agent-global-card">
        <div className="agent-global-card__label">Default for all</div>
        <div className="agent-global-card__fields">
          <div className="form-group">
            <label className="form-label">Provider</label>
            <select
              className="form-input form-select"
              value={globalDefault.providerId}
              onChange={(e) => {
                const pid = e.target.value;
                const models = getProviderModels(pid);
                onGlobalChange({ ...globalDefault, providerId: pid, model: models[0] || "" });
              }}
            >
              {activeProviders.length === 0 && (
                <option value="">No providers configured</option>
              )}
              {activeProviders.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Model</label>
            <select
              className="form-input form-select"
              value={globalDefault.model}
              onChange={(e) => onGlobalChange({ ...globalDefault, model: e.target.value })}
            >
              {globalModels.length === 0 && (
                <option value="">No models available</option>
              )}
              {globalModels.map((m) => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Temperature</label>
            <input
              className="form-input"
              type="number"
              value={globalDefault.temperature}
              onChange={(e) => onGlobalChange({ ...globalDefault, temperature: parseFloat(e.target.value) || 0 })}
              min={0} max={2} step={0.1}
            />
          </div>
          <div className="form-group">
            <label className="form-label">Max Output Tokens</label>
            <input
              className="form-input"
              type="number"
              value={globalDefault.maxTokens}
              onChange={(e) => onGlobalChange({ ...globalDefault, maxTokens: parseInt(e.target.value) || 4096 })}
              min={256} step={1024}
            />
          </div>
        </div>
      </div>

      {/* Agent list */}
      <div className="settings-field-label">Agents</div>
      <div className="agent-list">
        {sortedAgents.map((agent) => {
          const isRoot = agent === rootAgent;
          const isCustomized = !!agentOverrides[agent.id];
          const isExpanded = expandedAgent === agent.id;
          const effective = getEffectiveConfig(agent.id);
          const overrideModels = isCustomized && agentOverrides[agent.id]?.providerId
            ? getProviderModels(agentOverrides[agent.id].providerId!)
            : globalModels;

          return (
            <div key={agent.id} className={`agent-row ${isCustomized ? "agent-row--customized" : ""}`}>
              <div className="agent-row__summary">
                <div className="agent-row__info">
                  <span className="agent-row__name">{isRoot ? agentName : agent.displayName || agent.name}</span>
                  {isRoot && <span className="badge badge--primary badge--xs">root</span>}
                  {isCustomized && <span className="badge badge--warning badge--xs">customized</span>}
                </div>
                {!isExpanded ? (
                  <div className="flex items-center gap-2">
                    <span className="agent-row__config">
                      {getProviderName(effective.providerId)} &middot; {effective.model} &middot; {effective.temperature} &middot; {effective.maxTokens}
                    </span>
                    <button className="btn btn--outline btn--sm" onClick={() => setExpandedAgent(agent.id)}>
                      Customize
                    </button>
                  </div>
                ) : (
                  <button className="btn btn--ghost btn--sm" onClick={() => handleResetOverride(agent.id)}>
                    Reset to default
                  </button>
                )}
              </div>
              {isExpanded && (
                <div className="agent-row__fields">
                  <div>
                    <div className="agent-row__field-label">Provider</div>
                    <select
                      className="form-input form-select"
                      value={agentOverrides[agent.id]?.providerId || globalDefault.providerId}
                      onChange={(e) => handleOverride(agent.id, "providerId", e.target.value)}
                    >
                      {providers.map((p) => (
                        <option key={p.id} value={p.id}>{p.name}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <div className="agent-row__field-label">Model</div>
                    <select
                      className="form-input form-select"
                      value={agentOverrides[agent.id]?.model || globalDefault.model}
                      onChange={(e) => handleOverride(agent.id, "model", e.target.value)}
                    >
                      {overrideModels.map((m) => (
                        <option key={m} value={m}>{m}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <div className="agent-row__field-label">Temp</div>
                    <input
                      className="form-input"
                      type="number"
                      value={agentOverrides[agent.id]?.temperature ?? globalDefault.temperature}
                      onChange={(e) => handleOverride(agent.id, "temperature", parseFloat(e.target.value) || 0)}
                      min={0} max={2} step={0.1}
                    />
                  </div>
                  <div>
                    <div className="agent-row__field-label">Tokens</div>
                    <input
                      className="form-input"
                      type="number"
                      value={agentOverrides[agent.id]?.maxTokens ?? globalDefault.maxTokens}
                      onChange={(e) => handleOverride(agent.id, "maxTokens", parseInt(e.target.value) || 4096)}
                      min={256} step={1024}
                    />
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
