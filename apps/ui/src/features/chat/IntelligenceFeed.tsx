// ============================================================================
// INTELLIGENCE FEED
// Right sidebar: active ward, recalled facts, subagents, plan
// ============================================================================

import type { PlanStep } from "./PlanBlock";

// ============================================================================
// Types
// ============================================================================

export interface RecalledFact {
  key: string;
  content: string;
  category?: string;
  confidence?: number;
}

export interface SubagentInfo {
  agentId: string;
  task: string;
  status: "active" | "completed" | "error";
}

export interface IntelligenceFeedProps {
  ward: { name: string; content: string } | null;
  recalledFacts: RecalledFact[];
  subagents: SubagentInfo[];
  plan: PlanStep[];
}

// ============================================================================
// Status Icon
// ============================================================================

const STEP_ICON: Record<string, string> = {
  done: "\u2713",
  active: "\u27F3",
  pending: "\u25CB",
};

const SUBAGENT_DOT_COLOR: Record<string, string> = {
  active: "var(--success)",
  completed: "var(--success)",
  error: "var(--destructive)",
};

// ============================================================================
// Component
// ============================================================================

/**
 * IntelligenceFeed — right sidebar with 4 always-visible sections:
 * Active Ward, Recalled Facts, Subagents, Plan.
 */
export function IntelligenceFeed({
  ward,
  recalledFacts,
  subagents,
  plan,
}: IntelligenceFeedProps) {
  return (
    <div>
      {/* Active Ward */}
      <div className="intel-section">
        <div className="intel-section__title">Active Ward</div>
        {ward ? (
          <div>
            <div className="intel-ward__name">{ward.name}</div>
            <div className="intel-ward__content">
              {ward.content.length > 200 ? ward.content.slice(0, 200) + "..." : ward.content}
            </div>
          </div>
        ) : (
          <div className="intel-empty">No active ward</div>
        )}
      </div>

      {/* Recalled Facts */}
      <div className="intel-section">
        <div className="intel-section__title">Recalled Facts</div>
        {recalledFacts.length === 0 ? (
          <div className="intel-empty">{"\u2014"}</div>
        ) : (
          recalledFacts.map((fact, i) => (
            <div
              key={`${fact.key}-${i}`}
              className={`intel-fact${fact.category === "correction" ? " intel-fact--correction" : ""}`}
            >
              {fact.content || fact.key}
            </div>
          ))
        )}
      </div>

      {/* Subagents */}
      <div className="intel-section">
        <div className="intel-section__title">Subagents</div>
        {subagents.length === 0 ? (
          <div className="intel-empty">{"\u2014"}</div>
        ) : (
          subagents.map((sa, i) => (
            <div key={`${sa.agentId}-${i}`} className="intel-subagent">
              <span className="intel-subagent__name">
                <span
                  className="intel-subagent__dot"
                  style={{ background: SUBAGENT_DOT_COLOR[sa.status] || "var(--muted-foreground)" }}
                />
                {sa.agentId}
              </span>
              <span className="intel-subagent__task">
                {sa.task}
              </span>
            </div>
          ))
        )}
      </div>

      {/* Plan */}
      <div className="intel-section">
        <div className="intel-section__title">Plan</div>
        {plan.length === 0 ? (
          <div className="intel-empty">{"\u2014"}</div>
        ) : (
          plan.map((step, i) => (
            <div key={i} className={`plan-block__step plan-block__step--${step.status}`}>
              <span>{STEP_ICON[step.status] || "\u25CB"}</span>
              <span>{step.text}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
