// ============================================================================
// EMBEDDINGS CARD
// Settings → Advanced section for choosing the embedding backend
// (internal BGE-small vs Ollama curated models).
// ============================================================================

import { useCallback, useEffect, useMemo, useState } from "react";
import { Database, Loader2 } from "lucide-react";
import {
  getTransport,
  type CuratedModel,
  type EmbeddingConfig,
  type EmbeddingsHealth,
} from "@/services/transport";
import { EmbeddingProgressModal } from "./EmbeddingProgressModal";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FormState {
  useInternal: boolean;
  baseUrl: string;
  modelTag: string;
}

const INTERNAL_DIM = 384;
const DEFAULT_OLLAMA_URL = "http://localhost:11434";
const INTERNAL_ESTIMATE_PER_ITEM = 0.05;
const OLLAMA_ESTIMATE_PER_ITEM = 0.4;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function healthToForm(health: EmbeddingsHealth | null): FormState {
  if (!health || health.backend === "internal") {
    return { useInternal: true, baseUrl: DEFAULT_OLLAMA_URL, modelTag: "" };
  }
  return {
    useInternal: false,
    baseUrl: DEFAULT_OLLAMA_URL,
    modelTag: health.model ?? "",
  };
}

function formToConfig(form: FormState, model: CuratedModel | undefined): EmbeddingConfig | null {
  if (form.useInternal) {
    return { backend: "internal", dimensions: INTERNAL_DIM };
  }
  if (!model || !form.baseUrl.trim()) return null;
  return {
    backend: "ollama",
    dimensions: model.dim,
    ollama: { base_url: form.baseUrl.trim(), model: model.tag },
  };
}

function formMatchesHealth(form: FormState, health: EmbeddingsHealth | null): boolean {
  if (!health) return false;
  if (form.useInternal) return health.backend === "internal";
  return health.backend === "ollama" && health.model === form.modelTag;
}

function estimateSeconds(indexed: number, targetDim: number): number {
  const perItem = targetDim === INTERNAL_DIM ? INTERNAL_ESTIMATE_PER_ITEM : OLLAMA_ESTIMATE_PER_ITEM;
  return Math.max(1, Math.round(indexed * perItem));
}

function backendLabel(health: EmbeddingsHealth | null): string {
  if (!health) return "unknown";
  if (health.backend === "internal") return "internal (BGE-small)";
  return `ollama (${health.model ?? "?"})`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function EmbeddingsCard() {
  const [health, setHealth] = useState<EmbeddingsHealth | null>(null);
  const [models, setModels] = useState<CuratedModel[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(healthToForm(null));
  const [lastSwitched, setLastSwitched] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [pendingConfig, setPendingConfig] = useState<EmbeddingConfig | null>(null);

  const refreshHealth = useCallback(async () => {
    const transport = await getTransport();
    const result = await transport.getEmbeddingsHealth();
    if (result.success && result.data) {
      setHealth(result.data);
    } else {
      setLoadError(result.error ?? "Failed to load embeddings health");
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setIsLoading(true);
      const transport = await getTransport();
      const [healthRes, modelsRes] = await Promise.all([
        transport.getEmbeddingsHealth(),
        transport.getEmbeddingsModels(),
      ]);
      if (cancelled) return;
      if (healthRes.success && healthRes.data) {
        setHealth(healthRes.data);
        setForm(healthToForm(healthRes.data));
      } else {
        setLoadError(healthRes.error ?? "Failed to load embeddings health");
      }
      if (modelsRes.success && modelsRes.data) {
        setModels(modelsRes.data);
      }
      setIsLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const selectedModel = useMemo(
    () => models.find((m) => m.tag === form.modelTag),
    [models, form.modelTag],
  );

  const config = useMemo(() => formToConfig(form, selectedModel), [form, selectedModel]);
  const matches = formMatchesHealth(form, health);
  const targetDim = form.useInternal ? INTERNAL_DIM : selectedModel?.dim ?? 0;
  const dimChanged = health !== null && targetDim !== 0 && targetDim !== health.dim;

  const handleToggleInternal = useCallback((useInternal: boolean) => {
    setForm((prev) => ({
      ...prev,
      useInternal,
      modelTag: useInternal ? prev.modelTag : prev.modelTag || "",
    }));
  }, []);

  const handleCancel = useCallback(() => {
    setForm(healthToForm(health));
  }, [health]);

  const handleSave = useCallback(() => {
    if (!config) return;
    setPendingConfig(config);
    setModalOpen(true);
  }, [config]);

  const handleModalClose = useCallback(() => {
    setModalOpen(false);
    setPendingConfig(null);
  }, []);

  const handleSuccess = useCallback(
    (next: EmbeddingsHealth) => {
      setHealth(next);
      setForm(healthToForm(next));
      setLastSwitched(new Date().toLocaleString());
      refreshHealth();
    },
    [refreshHealth],
  );

  if (isLoading) {
    return (
      <div className="card card__padding--lg">
        <div className="settings-loading">
          <Loader2 className="loading-spinner__icon" />
        </div>
      </div>
    );
  }

  return (
    <div className="card card__padding--lg">
      <div className="flex items-center gap-3" style={{ marginBottom: "var(--spacing-3)" }}>
        <div className="card__icon card__icon--primary">
          <Database style={{ width: 18, height: 18 }} />
        </div>
        <div>
          <h2 className="settings-section-header">Embeddings</h2>
          <p className="page-subtitle">Vector embedding backend for memory recall</p>
        </div>
      </div>

      {loadError ? (
        <div className="settings-alert settings-alert--error" style={{ marginBottom: "var(--spacing-3)" }}>
          {loadError}
        </div>
      ) : null}

      <label
        className={`settings-toggle-option ${form.useInternal ? "settings-toggle-option--active" : ""}`}
        aria-label="Use internal embedding"
      >
        <input
          type="checkbox"
          checked={form.useInternal}
          onChange={(e) => handleToggleInternal(e.target.checked)}
          className="settings-toggle-option__checkbox"
        />
        <div className="flex-1">
          <div className="settings-toggle-option__title">
            Use internal embedding (BGE-small, 384d, ~130MB)
          </div>
          <div className="settings-toggle-option__description">
            Default. No Ollama required; runs in-process.
          </div>
        </div>
      </label>

      {!form.useInternal ? (
        <OllamaSubform
          form={form}
          setForm={setForm}
          models={models}
          health={health}
          selectedModel={selectedModel}
          dimChanged={dimChanged}
        />
      ) : null}

      <div
        className="flex items-center gap-3"
        style={{ marginTop: "var(--spacing-3)", justifyContent: "flex-end" }}
      >
        <button
          type="button"
          className="btn btn--outline btn--sm"
          onClick={handleCancel}
          disabled={matches}
        >
          Cancel
        </button>
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={handleSave}
          disabled={matches || !config}
        >
          Save &amp; Switch
        </button>
      </div>

      <div
        className="page-subtitle"
        style={{ marginTop: "var(--spacing-3)" }}
        data-testid="embeddings-status-footer"
      >
        Current state: {backendLabel(health)}, {health?.dim ?? 0}d,{" "}
        {health?.indexed_count ?? 0} indexed. Last switched: {lastSwitched ?? "never"}.
      </div>

      {modalOpen && pendingConfig ? (
        <EmbeddingProgressModal
          config={pendingConfig}
          indexedCount={health?.indexed_count ?? 0}
          onClose={handleModalClose}
          onSuccess={handleSuccess}
        />
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Ollama subform (extracted for complexity)
// ---------------------------------------------------------------------------

interface OllamaSubformProps {
  form: FormState;
  setForm: React.Dispatch<React.SetStateAction<FormState>>;
  models: CuratedModel[];
  health: EmbeddingsHealth | null;
  selectedModel: CuratedModel | undefined;
  dimChanged: boolean;
}

function OllamaSubform({
  form,
  setForm,
  models,
  health,
  selectedModel,
  dimChanged,
}: OllamaSubformProps) {
  const currentDim = health?.dim ?? 0;
  const indexed = health?.indexed_count ?? 0;

  return (
    <div style={{ marginTop: "var(--spacing-3)" }}>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="settings-field-label" htmlFor="emb-ollama-url">
            Ollama base URL
          </label>
          <input
            id="emb-ollama-url"
            className="form-input"
            type="text"
            value={form.baseUrl}
            onChange={(e) => setForm((p) => ({ ...p, baseUrl: e.target.value }))}
            placeholder={DEFAULT_OLLAMA_URL}
          />
        </div>
        <div>
          <label className="settings-field-label" htmlFor="emb-ollama-model">
            Model
          </label>
          <select
            id="emb-ollama-model"
            className="form-input form-select"
            value={form.modelTag}
            onChange={(e) => setForm((p) => ({ ...p, modelTag: e.target.value }))}
          >
            <option value="">Select model</option>
            {models.map((m) => {
              const noReindex = m.dim === currentDim;
              const suffix = noReindex ? " ← no reindex" : "";
              return (
                <option key={m.tag} value={m.tag}>
                  {`${m.tag} (${m.dim}d, ${m.size_mb}MB)${suffix}`}
                </option>
              );
            })}
          </select>
        </div>
      </div>

      {selectedModel && dimChanged ? (
        <div
          className="settings-alert settings-alert--warning"
          style={{ marginTop: "var(--spacing-3)" }}
          data-testid="emb-warning"
        >
          Switching to {selectedModel.tag} will: pull ~{selectedModel.size_mb}MB from Ollama (if
          not present), reindex {indexed} embeddings (~
          {estimateSeconds(indexed, selectedModel.dim)}s), briefly pause memory recall during
          reindex.
        </div>
      ) : null}
    </div>
  );
}
