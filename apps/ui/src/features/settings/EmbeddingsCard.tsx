// ============================================================================
// EMBEDDINGS CARD
// Settings → Advanced section for choosing the embedding backend.
// Internal BGE (384d) vs Ollama with freeform model typeahead + editable
// dimensions. Mirrors the on-disk shape { internal, ollama: {url, model,
// dimensions} } — see `embedding_service.rs::EmbeddingConfig`.
// ============================================================================

import { useCallback, useEffect, useMemo, useState } from "react";
import { Database, Loader2 } from "lucide-react";
import {
  getTransport,
  type CuratedModel,
  type EmbeddingConfig,
  type EmbeddingsHealth,
  type OllamaModelsResponse,
} from "@/services/transport";
import { EmbeddingProgressModal } from "./EmbeddingProgressModal";

// ---------------------------------------------------------------------------
// Types + constants
// ---------------------------------------------------------------------------

interface FormState {
  useInternal: boolean;
  baseUrl: string;
  modelTag: string;
  dimensions: number;
}

const INTERNAL_DIM = 384;
const DEFAULT_OLLAMA_URL = "http://localhost:11434";
const INTERNAL_ESTIMATE_PER_ITEM = 0.05;
const OLLAMA_ESTIMATE_PER_ITEM = 0.4;
const MODEL_DATALIST_ID = "embedding-models-datalist";

// ---------------------------------------------------------------------------
// Helpers (pure)
// ---------------------------------------------------------------------------

function healthToForm(health: EmbeddingsHealth | null): FormState {
  if (!health || health.backend === "internal") {
    return {
      useInternal: true,
      baseUrl: DEFAULT_OLLAMA_URL,
      modelTag: "",
      dimensions: INTERNAL_DIM,
    };
  }
  return {
    useInternal: false,
    baseUrl: DEFAULT_OLLAMA_URL,
    modelTag: health.model ?? "",
    dimensions: health.dim || 0,
  };
}

function formToConfig(form: FormState): EmbeddingConfig | null {
  if (form.useInternal) {
    return { internal: true };
  }
  if (!form.baseUrl.trim() || !form.modelTag.trim() || form.dimensions <= 0) {
    return null;
  }
  return {
    internal: false,
    ollama: {
      url: form.baseUrl.trim(),
      model: form.modelTag.trim(),
      dimensions: form.dimensions,
    },
  };
}

function formMatchesHealth(form: FormState, health: EmbeddingsHealth | null): boolean {
  if (!health) return false;
  if (form.useInternal) return health.backend === "internal";
  return (
    health.backend === "ollama" &&
    health.model === form.modelTag &&
    health.dim === form.dimensions
  );
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
  const [curated, setCurated] = useState<CuratedModel[]>([]);
  const [liveOllama, setLiveOllama] = useState<OllamaModelsResponse | null>(null);
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

  // Initial load: health + curated suggestions.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      setIsLoading(true);
      const transport = await getTransport();
      const [healthRes, curatedRes] = await Promise.all([
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
      if (curatedRes.success && curatedRes.data) {
        setCurated(curatedRes.data);
      }
      setIsLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Refresh the live Ollama list whenever the URL changes (while in Ollama mode).
  useEffect(() => {
    if (form.useInternal) {
      setLiveOllama(null);
      return;
    }
    const url = form.baseUrl.trim();
    if (!url) {
      setLiveOllama(null);
      return;
    }
    let cancelled = false;
    const handle = globalThis.setTimeout(async () => {
      const transport = await getTransport();
      const res = await transport.getOllamaEmbeddingModels(url);
      if (!cancelled && res.success && res.data) {
        setLiveOllama(res.data);
      }
    }, 300);
    return () => {
      cancelled = true;
      globalThis.clearTimeout(handle);
    };
  }, [form.useInternal, form.baseUrl]);

  const curatedByTag = useMemo(() => {
    const map = new Map<string, CuratedModel>();
    for (const c of curated) map.set(c.tag, c);
    return map;
  }, [curated]);

  const suggestions = useMemo(() => buildSuggestions(curated, liveOllama), [curated, liveOllama]);

  const config = useMemo(() => formToConfig(form), [form]);
  const matches = formMatchesHealth(form, health);
  const targetDim = form.useInternal ? INTERNAL_DIM : form.dimensions;
  const dimChanged = health !== null && targetDim !== 0 && targetDim !== health.dim;
  const currentDim = health?.dim ?? 0;
  const indexed = health?.indexed_count ?? 0;

  const handleToggleInternal = useCallback((useInternal: boolean) => {
    setForm((prev) => ({
      ...prev,
      useInternal,
      dimensions: useInternal ? INTERNAL_DIM : prev.dimensions,
    }));
  }, []);

  const handleModelChange = useCallback(
    (next: string) => {
      setForm((prev) => {
        const curatedDim = curatedByTag.get(next)?.dim;
        return {
          ...prev,
          modelTag: next,
          // Auto-fill dim on a curated match; leave editable otherwise.
          dimensions: curatedDim ?? prev.dimensions,
        };
      });
    },
    [curatedByTag],
  );

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
        <div
          className="settings-alert settings-alert--error"
          style={{ marginBottom: "var(--spacing-3)" }}
        >
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
          onModelChange={handleModelChange}
          onFormChange={setForm}
          suggestions={suggestions}
          liveOllama={liveOllama}
          currentDim={currentDim}
          indexed={indexed}
          dimChanged={dimChanged}
          targetDim={targetDim}
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
        {indexed} indexed. Last switched: {lastSwitched ?? "never"}.
      </div>

      {modalOpen && pendingConfig ? (
        <EmbeddingProgressModal
          config={pendingConfig}
          indexedCount={indexed}
          onClose={handleModalClose}
          onSuccess={handleSuccess}
        />
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Suggestion assembly (curated + live)
// ---------------------------------------------------------------------------

interface ModelSuggestion {
  tag: string;
  label: string;
  installed: boolean;
  dim?: number;
}

function buildSuggestions(
  curated: CuratedModel[],
  live: OllamaModelsResponse | null,
): ModelSuggestion[] {
  const out: ModelSuggestion[] = [];
  const seen = new Set<string>();

  // Live Ollama entries first (marked installed, order preserved).
  if (live?.reachable) {
    for (const tag of live.likely_embedding) {
      if (seen.has(tag)) continue;
      seen.add(tag);
      const dim = curated.find((c) => c.tag === tag || tag.startsWith(`${c.tag}:`))?.dim;
      out.push({ tag, label: `${tag} · installed`, installed: true, dim });
    }
  }

  // Then curated entries that aren't already in the live list.
  for (const c of curated) {
    if (seen.has(c.tag)) continue;
    seen.add(c.tag);
    out.push({
      tag: c.tag,
      label: `${c.tag} (${c.dim}d, ${c.size_mb}MB)`,
      installed: false,
      dim: c.dim,
    });
  }

  return out;
}

// ---------------------------------------------------------------------------
// Ollama subform (extracted for complexity)
// ---------------------------------------------------------------------------

interface OllamaSubformProps {
  form: FormState;
  onModelChange: (next: string) => void;
  onFormChange: React.Dispatch<React.SetStateAction<FormState>>;
  suggestions: ModelSuggestion[];
  liveOllama: OllamaModelsResponse | null;
  currentDim: number;
  indexed: number;
  dimChanged: boolean;
  targetDim: number;
}

function OllamaSubform({
  form,
  onModelChange,
  onFormChange,
  suggestions,
  liveOllama,
  currentDim,
  indexed,
  dimChanged,
  targetDim,
}: OllamaSubformProps) {
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
            onChange={(e) => onFormChange((p) => ({ ...p, baseUrl: e.target.value }))}
            placeholder={DEFAULT_OLLAMA_URL}
          />
          {liveOllama && !liveOllama.reachable ? (
            <div className="page-subtitle" style={{ marginTop: 4, color: "var(--color-warning, #c77)" }}>
              Unreachable — showing curated suggestions only.
            </div>
          ) : null}
        </div>
        <div>
          <label className="settings-field-label" htmlFor="emb-ollama-model">
            Model
          </label>
          <input
            id="emb-ollama-model"
            className="form-input"
            type="text"
            list={MODEL_DATALIST_ID}
            value={form.modelTag}
            onChange={(e) => onModelChange(e.target.value)}
            placeholder="e.g. nomic-embed-text"
            autoComplete="off"
            spellCheck={false}
          />
          <datalist id={MODEL_DATALIST_ID}>
            {suggestions.map((s) => (
              <option key={s.tag} value={s.tag} label={s.label} />
            ))}
          </datalist>
          {form.modelTag && !suggestions.some((s) => s.tag === form.modelTag) ? (
            <div className="page-subtitle" style={{ marginTop: 4 }}>
              Custom model — dimensions not auto-filled. Set it manually below.
            </div>
          ) : null}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3" style={{ marginTop: "var(--spacing-3)" }}>
        <div>
          <label className="settings-field-label" htmlFor="emb-ollama-dim">
            Dimensions
          </label>
          <input
            id="emb-ollama-dim"
            className="form-input"
            type="number"
            min={1}
            value={form.dimensions || ""}
            onChange={(e) => {
              const n = Number.parseInt(e.target.value, 10);
              onFormChange((p) => ({ ...p, dimensions: Number.isFinite(n) ? n : 0 }));
            }}
            placeholder="e.g. 768"
          />
          <div className="page-subtitle" style={{ marginTop: 4 }}>
            Must match the model's output width. Verified at save time.
          </div>
        </div>
      </div>

      {dimChanged && targetDim > 0 ? (
        <div
          className="settings-alert settings-alert--warning"
          style={{ marginTop: "var(--spacing-3)" }}
          data-testid="emb-warning"
        >
          Switching to {form.modelTag || "this model"} will reindex {indexed} embeddings (~
          {estimateSeconds(indexed, targetDim)}s) and briefly pause memory recall. Current {currentDim}d → new {targetDim}d.
        </div>
      ) : null}
    </div>
  );
}
