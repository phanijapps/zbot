// =============================================================================
// ResearchPage — top-level page component for the research-v2 feature.
//
// Three vertical zones:
//   1. Header  — drawer toggle · title · ward chip + new + stop
//   2. Pill strip — StatusPill (centered)
//   3. Body    — scrollable column (max 880 px, centred)
//   4. Composer — ChatInput pinned at the bottom
//
// ArtifactSlideOut state is scaffolded here; artifact CARDS are added in R15.
// =============================================================================

import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";
import { FolderOpen, Menu, Plus, Square } from "lucide-react";
import { toast } from "sonner";
import { ChatInput } from "../chat/ChatInput";
import { ArtifactSlideOut } from "../chat/ArtifactSlideOut";
import { StatusPill } from "../shared/statusPill";
import { AgentTurnBlock } from "./AgentTurnBlock";
import { SessionsDrawer } from "./SessionsDrawer";
import { useResearchSession } from "./useResearchSession";
import { useSessionsList } from "./useSessionsList";
import { rootTurns, childrenOf } from "./turn-tree";
import { getTransport } from "@/services/transport";
import type { ResearchSessionState } from "./types";
import type { Artifact } from "@/services/transport/types";
import "./research.css";

// --- Sub-components ----------------------------------------------------------

interface ResearchHeaderProps {
  state: ResearchSessionState;
  onOpenDrawer(): void;
  onNew(): void;
  onStop(): void;
  onOpenWard(wardId: string): void;
}

function ResearchHeader({ state, onOpenDrawer, onNew, onStop, onOpenWard }: ResearchHeaderProps) {
  return (
    <div className="research-page__header">
      <button
        type="button"
        className="btn btn--ghost btn--sm"
        onClick={onOpenDrawer}
        aria-label="Open sessions"
        title="Open sessions"
      >
        <Menu size={16} />
      </button>

      <div className="research-page__title">zbot</div>

      <div className="research-page__header-actions">
        {state.wardId && state.wardName && (
          <button
            type="button"
            className="research-page__ward-chip research-page__ward-chip--clickable"
            onClick={() => onOpenWard(state.wardId as string)}
            title={`Open ward folder: ${state.wardName}`}
            aria-label={`Open ward folder: ${state.wardName}`}
          >
            <FolderOpen size={12} />
            <span>{state.wardName}</span>
          </button>
        )}
        <button type="button" className="btn btn--ghost btn--sm" onClick={onNew}>
          <Plus size={14} /> New research
        </button>
        {state.status === "running" && (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={onStop}
            title="Stop"
          >
            <Square size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

function IntentLine({ state }: { state: ResearchSessionState }) {
  if (state.intentAnalyzing) {
    return <div className="research-page__intent-muted">analyzing intent…</div>;
  }
  if (state.intentClassification) {
    return (
      <div className="research-page__intent-classification">
        intent: <strong>{state.intentClassification}</strong>
        {state.wardName && (
          <>
            {" · ward: "}
            <strong>{state.wardName}</strong>
          </>
        )}
      </div>
    );
  }
  return null;
}

function EmptyState() {
  return (
    <div className="research-page__empty">
      <h1>Research</h1>
      <p>Ask a research question — the full agent chain kicks in.</p>
    </div>
  );
}

interface MainColumnProps {
  state: ResearchSessionState;
  onToggleThinking(turnId: string): void;
}

function MainColumn({ state, onToggleThinking }: MainColumnProps) {
  const hasContent =
    state.messages.length > 0 || state.turns.length > 0 || state.sessionId !== null;

  if (!hasContent) return <EmptyState />;

  // Render root turns at depth 0; nested children are derived inside
  // AgentTurnBlock via the allTurns prop. See turn-tree.ts and the R14b spec
  // (option A) for why the tree shape is derived at render, not stored.
  const roots = rootTurns(state.turns);

  return (
    <>
      {state.messages.map((m) => (
        <div key={m.id} className="research-page__user-bubble">
          {m.content}
        </div>
      ))}
      <IntentLine state={state} />
      {roots.map((turn) => (
        <AgentTurnBlock
          key={turn.id}
          turn={turn}
          onToggleThinking={onToggleThinking}
          childTurns={childrenOf(turn, state.turns)}
          allTurns={state.turns}
        />
      ))}
    </>
  );
}

// --- Page --------------------------------------------------------------------

export function ResearchPage() {
  const { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking } =
    useResearchSession();
  const { sessions, refresh: refreshSessions } = useSessionsList();
  const navigate = useNavigate();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [viewingArtifact, setViewingArtifact] = useState<Artifact | null>(null);

  const handleSelect = (id: string) => {
    setDrawerOpen(false);
    navigate(`/research-v2/${id}`);
  };

  const handleNew = () => {
    setDrawerOpen(false);
    startNewResearch();
    void refreshSessions();
  };

  // Memoised so the ResearchHeader sub-component doesn't re-render each tick.
  const handleOpenWard = useCallback(async (wardId: string) => {
    const transport = await getTransport();
    const r = await transport.openWard(wardId);
    if (!r.success) {
      toast.error(`Failed to open ward folder: ${r.error ?? "unknown"}`);
    }
  }, []);

  const composerDisabled = state.status === "running";

  return (
    <div className="research-page">
      <ResearchHeader
        state={state}
        onOpenDrawer={() => setDrawerOpen(true)}
        onNew={handleNew}
        onStop={stopAgent}
        onOpenWard={handleOpenWard}
      />

      <div className="research-page__pill-strip">
        <StatusPill state={pillState} />
      </div>

      <SessionsDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        sessions={sessions}
        currentId={state.sessionId}
        onSelect={handleSelect}
        onNew={handleNew}
      />

      <div className="research-page__body">
        <div className="research-page__column">
          <MainColumn state={state} onToggleThinking={toggleThinking} />
        </div>
      </div>

      <div className="research-page__composer">
        <ChatInput onSend={sendMessage} disabled={composerDisabled} />
      </div>

      {viewingArtifact && (
        <ArtifactSlideOut
          artifact={viewingArtifact}
          onClose={() => setViewingArtifact(null)}
        />
      )}
    </div>
  );
}
