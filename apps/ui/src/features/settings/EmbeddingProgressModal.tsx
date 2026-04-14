// ============================================================================
// EMBEDDING PROGRESS MODAL
// Subscribes to the /api/embeddings/configure SSE stream and renders
// pulling / reindexing / ready / error phases.
// ============================================================================

import { useCallback, useEffect, useRef, useState } from "react";
import {
  getTransport,
  type ConfigureProgressEvent,
  type EmbeddingConfig,
  type EmbeddingsHealth,
} from "@/services/transport";
import { ModalOverlay } from "@/shared/ui/modal-overlay";

// ---------------------------------------------------------------------------
// Phase state
// ---------------------------------------------------------------------------

type Phase =
  | { kind: "pulling"; mb_done: number; mb_total: number }
  | { kind: "reindexing"; table: string; current: number; total: number }
  | { kind: "ready" }
  | { kind: "error"; reason: string };

interface EmbeddingProgressModalProps {
  config: EmbeddingConfig;
  indexedCount: number;
  onClose: () => void;
  onSuccess: (health: EmbeddingsHealth) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function EmbeddingProgressModal({
  config,
  indexedCount,
  onClose,
  onSuccess,
}: EmbeddingProgressModalProps) {
  const [phase, setPhase] = useState<Phase>({ kind: "pulling", mb_done: 0, mb_total: 0 });
  const abortRef = useRef<AbortController | null>(null);

  const handleProgress = useCallback((event: ConfigureProgressEvent) => {
    if (event.kind === "pulling") {
      setPhase({ kind: "pulling", mb_done: event.mb_done, mb_total: event.mb_total });
    } else if (event.kind === "reindexing") {
      setPhase({
        kind: "reindexing",
        table: event.table,
        current: event.current,
        total: event.total,
      });
    } else if (event.kind === "ready") {
      setPhase({ kind: "ready" });
    } else if (event.kind === "error") {
      setPhase({ kind: "error", reason: event.reason });
    }
  }, []);

  const start = useCallback(async () => {
    const controller = new AbortController();
    abortRef.current = controller;
    setPhase({ kind: "pulling", mb_done: 0, mb_total: 0 });
    const transport = await getTransport();
    const result = await transport.configureEmbeddings(config, handleProgress, controller.signal);
    if (result.success && result.data) {
      onSuccess(result.data);
      setPhase({ kind: "ready" });
    } else if (controller.signal.aborted) {
      // Modal already closed.
    } else {
      setPhase((prev) =>
        prev.kind === "error" ? prev : { kind: "error", reason: result.error ?? "Unknown error" },
      );
    }
  }, [config, handleProgress, onSuccess]);

  useEffect(() => {
    start();
    return () => {
      abortRef.current?.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const isWorking = phase.kind === "pulling" || phase.kind === "reindexing";

  const handleRetry = useCallback(() => {
    start();
  }, [start]);

  const handleCloseClick = useCallback(() => {
    abortRef.current?.abort();
    onClose();
  }, [onClose]);

  return (
    <ModalOverlay
      open
      onClose={handleCloseClick}
      title="Switching embedding backend"
      subtitle={`Target: ${config.backend}${config.ollama ? ` · ${config.ollama.model}` : ""}`}
      showCloseButton={!isWorking}
      closeOnEscape={!isWorking}
      closeOnBackdropClick={false}
    >
      <div data-testid="emb-progress-modal" style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-3)" }}>
        <div
          className="settings-alert settings-alert--warning"
          role="status"
          aria-live="polite"
        >
          Memory recall is paused; sessions can continue with reduced context until indices are
          rebuilt.
        </div>

        <PhaseBody phase={phase} indexedCount={indexedCount} />

        <div className="flex items-center gap-3" style={{ justifyContent: "flex-end" }}>
          {phase.kind === "error" ? (
            <button type="button" className="btn btn--primary btn--sm" onClick={handleRetry}>
              Retry
            </button>
          ) : null}
          {!isWorking ? (
            <button type="button" className="btn btn--outline btn--sm" onClick={handleCloseClick}>
              Close
            </button>
          ) : null}
        </div>
      </div>
    </ModalOverlay>
  );
}

// ---------------------------------------------------------------------------
// Phase body (extracted to keep component complexity low)
// ---------------------------------------------------------------------------

interface PhaseBodyProps {
  phase: Phase;
  indexedCount: number;
}

function PhaseBody({ phase, indexedCount }: PhaseBodyProps) {
  if (phase.kind === "pulling") return <PullingBody mb_done={phase.mb_done} mb_total={phase.mb_total} />;
  if (phase.kind === "reindexing") {
    return (
      <ReindexingBody
        table={phase.table}
        current={phase.current}
        total={phase.total || indexedCount}
      />
    );
  }
  if (phase.kind === "ready") return <ReadyBody />;
  return <ErrorBody reason={phase.reason} />;
}

function percent(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.round((done / total) * 100));
}

function ProgressBar({ value, label }: { value: number; label: string }) {
  return (
    <div>
      <div className="page-subtitle" style={{ marginBottom: 4 }}>
        {label}
      </div>
      <div
        role="progressbar"
        aria-valuenow={value}
        aria-valuemin={0}
        aria-valuemax={100}
        style={{
          width: "100%",
          height: 8,
          background: "var(--color-surface-2, #eee)",
          borderRadius: 4,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: `${value}%`,
            height: "100%",
            background: "var(--color-primary, #4f46e5)",
            transition: "width 150ms ease",
          }}
        />
      </div>
    </div>
  );
}

function PullingBody({ mb_done, mb_total }: { mb_done: number; mb_total: number }) {
  const pct = percent(mb_done, mb_total);
  return (
    <div data-testid="phase-pulling">
      <h3 className="settings-section-header">Pulling model</h3>
      <ProgressBar value={pct} label={`${mb_done} MB / ${mb_total} MB (${pct}%)`} />
    </div>
  );
}

function ReindexingBody({ table, current, total }: { table: string; current: number; total: number }) {
  const pct = percent(current, total);
  return (
    <div data-testid="phase-reindexing">
      <h3 className="settings-section-header">Reindexing</h3>
      <ProgressBar value={pct} label={`${table}: ${current} / ${total} (${pct}%)`} />
    </div>
  );
}

function ReadyBody() {
  return (
    <div data-testid="phase-ready">
      <h3 className="settings-section-header">Done</h3>
      <p className="page-subtitle">Embedding backend switched successfully. Memory recall resumed.</p>
    </div>
  );
}

function ErrorBody({ reason }: { reason: string }) {
  return (
    <div data-testid="phase-error">
      <h3 className="settings-section-header">Switch failed</h3>
      <p className="page-subtitle">{reason}</p>
    </div>
  );
}
