import { useReducer, useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse, McpServerConfig } from "@/services/transport";
import { NAME_PRESETS } from "./presets";
import { StepIndicator } from "./components/StepIndicator";
import { WizardNav } from "./components/WizardNav";
import { NameStep } from "./steps/NameStep";
import { ProvidersStep } from "./steps/ProvidersStep";
import { SkillsStep } from "./steps/SkillsStep";
import { McpStep } from "./steps/McpStep";
import { AgentsStep } from "./steps/AgentsStep";
import { ReviewStep } from "./steps/ReviewStep";

interface WizardState {
  currentStep: number;
  agentName: string;
  namePreset: string | null;
  providers: ProviderResponse[];
  defaultProviderId: string;
  enabledSkillIds: string[];
  mcpConfigs: McpServerConfig[];
  globalDefault: {
    providerId: string;
    model: string;
    temperature: number;
    maxTokens: number;
  };
  agentOverrides: Record<string, {
    providerId?: string;
    model?: string;
    temperature?: number;
    maxTokens?: number;
  }>;
}

type WizardAction =
  | { type: "SET_STEP"; step: number }
  | { type: "SET_NAME"; name: string; preset: string | null }
  | { type: "SET_PROVIDERS"; providers: ProviderResponse[]; defaultId: string }
  | { type: "SET_SKILLS"; ids: string[] }
  | { type: "SET_MCPS"; configs: McpServerConfig[] }
  | { type: "SET_GLOBAL_DEFAULT"; defaults: WizardState["globalDefault"] }
  | { type: "SET_OVERRIDES"; overrides: WizardState["agentOverrides"] }
  | { type: "HYDRATE"; state: Partial<WizardState> };

function reducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case "SET_STEP": return { ...state, currentStep: action.step };
    case "SET_NAME": return { ...state, agentName: action.name, namePreset: action.preset };
    case "SET_PROVIDERS": return { ...state, providers: action.providers, defaultProviderId: action.defaultId };
    case "SET_SKILLS": return { ...state, enabledSkillIds: action.ids };
    case "SET_MCPS": return { ...state, mcpConfigs: action.configs };
    case "SET_GLOBAL_DEFAULT": return { ...state, globalDefault: action.defaults };
    case "SET_OVERRIDES": return { ...state, agentOverrides: action.overrides };
    case "HYDRATE": return { ...state, ...action.state };
    default: return state;
  }
}

const initialState: WizardState = {
  currentStep: 1,
  agentName: "Brahmi",
  namePreset: "brahmi",
  providers: [],
  defaultProviderId: "",
  enabledSkillIds: [],
  mcpConfigs: [],
  globalDefault: { providerId: "", model: "", temperature: 0.7, maxTokens: 4096 },
  agentOverrides: {},
};

const STEP_TITLES: Record<number, { title: string; subtitle: string }> = {
  1: { title: "What should we call your agent?", subtitle: "Pick a personality or choose your own name." },
  2: { title: "Connect your AI providers", subtitle: "Add at least one provider to power your agents." },
  3: { title: "Enable skills", subtitle: "Choose which skills your agents can use." },
  4: { title: "Configure tool servers", subtitle: "Connect external tools and services via MCP." },
  5: { title: "Configure your agents", subtitle: "Set a default model, then customize individual agents." },
  6: { title: "Review & Launch", subtitle: "Everything looks good? Hit launch to get started." },
};

export function SetupWizard() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [isHydrating, setIsHydrating] = useState(true);
  const navigate = useNavigate();

  // On mount, load existing config for re-run mode
  useEffect(() => {
    const hydrate = async () => {
      try {
        const transport = await getTransport();
        const [providersRes, agentsRes, mcpsRes] = await Promise.all([
          transport.listProviders(),
          transport.listAgents(),
          transport.listMcps(),
        ]);

        const hydrated: Partial<WizardState> = {};

        // Pre-fill providers
        if (providersRes.success && providersRes.data && providersRes.data.length > 0) {
          const providers = providersRes.data;
          hydrated.providers = providers;
          const defaultProvider = providers.find((p) => p.isDefault) || providers[0];
          hydrated.defaultProviderId = defaultProvider?.id || "";

          // Pre-fill global default from the default provider's first model
          if (defaultProvider) {
            hydrated.globalDefault = {
              providerId: defaultProvider.id!,
              model: defaultProvider.defaultModel || defaultProvider.models[0] || "",
              temperature: 0.7,
              maxTokens: 4096,
            };
          }
        }

        // Pre-fill agent name from root agent
        if (agentsRes.success && agentsRes.data) {
          const rootAgent = agentsRes.data.find((a) => a.name === "root");
          if (rootAgent && rootAgent.displayName && rootAgent.displayName !== "root") {
            const name = rootAgent.displayName;
            hydrated.agentName = name;
            const matchingPreset = NAME_PRESETS.find((p) => p.name === name && p.id !== "custom");
            hydrated.namePreset = matchingPreset?.id || "custom";
          }
        }

        // Pre-fill MCP configs from existing servers
        if (mcpsRes.success && mcpsRes.data && mcpsRes.data.servers && mcpsRes.data.servers.length > 0) {
          // Existing MCPs are already configured — map summaries to configs
          // The McpStep will still load defaults for new servers, but we mark existing as done
          hydrated.mcpConfigs = mcpsRes.data.servers.map((s) => ({
            type: s.type as McpServerConfig["type"],
            id: s.id,
            name: s.name,
            description: s.description,
            enabled: s.enabled,
          }));
        }

        if (Object.keys(hydrated).length > 0) {
          dispatch({ type: "HYDRATE", state: hydrated });
        }
      } catch {
        // First run — no existing config, use defaults
      } finally {
        setIsHydrating(false);
      }
    };
    hydrate();
  }, []);

  const canNext = (): boolean => {
    switch (state.currentStep) {
      case 1: return state.agentName.trim().length > 0;
      case 2: return state.providers.some((p) => p.verified);
      case 3: return true;
      case 4: return true;
      case 5: return !!state.globalDefault.providerId && !!state.globalDefault.model;
      case 6: return true;
      default: return false;
    }
  };

  const handleNext = () => {
    if (state.currentStep < 6) {
      dispatch({ type: "SET_STEP", step: state.currentStep + 1 });
    }
  };

  const handleBack = () => {
    if (state.currentStep > 1) {
      dispatch({ type: "SET_STEP", step: state.currentStep - 1 });
    }
  };

  const handleSkip = useCallback(() => {
    navigate("/");
  }, [navigate]);

  const handleLaunchComplete = useCallback(() => {
    sessionStorage.setItem("setupComplete", "true");
    navigate("/");
  }, [navigate]);

  const isSkippable = state.currentStep === 3 || state.currentStep === 4;
  const stepInfo = STEP_TITLES[state.currentStep];

  if (isHydrating) {
    return (
      <div className="setup-wizard">
        <div className="settings-loading">
          <Loader2 className="loading-spinner__icon" />
        </div>
      </div>
    );
  }

  return (
    <div className="setup-wizard">
      <div className="setup-wizard__container">
        <StepIndicator currentStep={state.currentStep} />

        <div className="setup-wizard__header">
          <h2 className="setup-wizard__title">{stepInfo.title}</h2>
          <p className="setup-wizard__subtitle">{stepInfo.subtitle}</p>
        </div>

        <div className="setup-wizard__body">
          {state.currentStep === 1 && (
            <NameStep
              agentName={state.agentName}
              namePreset={state.namePreset}
              onChange={(name, preset) => dispatch({ type: "SET_NAME", name, preset })}
            />
          )}
          {state.currentStep === 2 && (
            <ProvidersStep
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              onProvidersChanged={(providers, defaultId) =>
                dispatch({ type: "SET_PROVIDERS", providers, defaultId })
              }
            />
          )}
          {state.currentStep === 3 && (
            <SkillsStep
              enabledSkillIds={state.enabledSkillIds}
              onChange={(ids) => dispatch({ type: "SET_SKILLS", ids })}
            />
          )}
          {state.currentStep === 4 && (
            <McpStep
              mcpConfigs={state.mcpConfigs}
              onChange={(configs) => dispatch({ type: "SET_MCPS", configs })}
            />
          )}
          {state.currentStep === 5 && (
            <AgentsStep
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              agentName={state.agentName}
              globalDefault={state.globalDefault}
              agentOverrides={state.agentOverrides}
              onGlobalChange={(defaults) => dispatch({ type: "SET_GLOBAL_DEFAULT", defaults })}
              onOverrideChange={(overrides) => dispatch({ type: "SET_OVERRIDES", overrides })}
            />
          )}
          {state.currentStep === 6 && (
            <ReviewStep
              agentName={state.agentName}
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              enabledSkillIds={state.enabledSkillIds}
              mcpConfigs={state.mcpConfigs}
              globalDefault={state.globalDefault}
              agentOverrides={state.agentOverrides}
              onLaunchComplete={handleLaunchComplete}
            />
          )}
        </div>

        {state.currentStep < 6 && (
          <WizardNav
            currentStep={state.currentStep}
            canNext={canNext()}
            onBack={handleBack}
            onNext={handleNext}
            onSkip={isSkippable || state.currentStep === 1 ? handleSkip : undefined}
          />
        )}
      </div>
    </div>
  );
}
