// ============================================================================
// INTELLIGENCE FEED
// Right sidebar: active ward, recalled facts, subagents, plan
// ============================================================================

import type { PlanStep } from "./PlanBlock";
import type { IntentAnalysis } from "./mission-hooks";

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
  intentAnalysis: IntentAnalysis | null;
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
  intentAnalysis,
}: IntelligenceFeedProps) {
  return (
    <div>
      {/* Intent Analysis */}
      {intentAnalysis && (
        <details className="intel-section">
          <summary className="intel-section__header">
            <span className="intel-section__icon">&#x1f9e0;</span>
            Intent Analysis
            <span className="intel-badge">{intentAnalysis.executionStrategy.approach}</span>
          </summary>
          <div className="intel-section__body">
            <div className="intel-item">
              <span className="intel-label">Primary Intent</span>
              <span className="intel-value">{intentAnalysis.primaryIntent}</span>
            </div>

            {intentAnalysis.hiddenIntents.length > 0 && (
              <div className="intel-item">
                <span className="intel-label">Hidden Intents</span>
                <ul className="intel-list">
                  {intentAnalysis.hiddenIntents.map((h, i) => (
                    <li key={i}>{h}</li>
                  ))}
                </ul>
              </div>
            )}

            {intentAnalysis.recommendedSkills.length > 0 && (
              <div className="intel-item">
                <span className="intel-label">Skills</span>
                <div className="intel-tags">
                  {intentAnalysis.recommendedSkills.map((s) => (
                    <span key={s} className="intel-tag">{s}</span>
                  ))}
                </div>
              </div>
            )}

            {intentAnalysis.recommendedAgents.length > 0 && (
              <div className="intel-item">
                <span className="intel-label">Agents</span>
                <div className="intel-tags">
                  {intentAnalysis.recommendedAgents.map((a) => (
                    <span key={a} className="intel-tag">{a}</span>
                  ))}
                </div>
              </div>
            )}

            <div className="intel-item">
              <span className="intel-label">Ward</span>
              <span className="intel-value">
                {intentAnalysis.wardRecommendation.wardName} ({intentAnalysis.wardRecommendation.action})
              </span>
              <span className="intel-detail">{intentAnalysis.wardRecommendation.reason}</span>
            </div>

            {intentAnalysis.executionStrategy.graph && (
              <div className="intel-item">
                <span className="intel-label">Execution Graph</span>
                <ul className="intel-list">
                  {intentAnalysis.executionStrategy.graph.nodes.map((n) => (
                    <li key={n.id}>
                      <strong>{n.id}:</strong> {n.task} <em>({n.agent})</em>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            <div className="intel-item">
              <span className="intel-label">Strategy</span>
              <span className="intel-detail">{intentAnalysis.executionStrategy.explanation}</span>
            </div>
          </div>
        </details>
      )}

      {/* Active Ward */}
      <details className="intel-section" open>
        <summary className="intel-section__header">
          <span className="intel-section__icon">&#x1f3ef;</span>
          Ward
          {ward && <span className="intel-badge">{ward.name}</span>}
        </summary>
        <div className="intel-section__body">
          {ward ? (
            <div className="intel-ward__content">
              {ward.content.length > 200 ? ward.content.slice(0, 200) + "..." : ward.content}
            </div>
          ) : (
            <div className="intel-empty">No active ward</div>
          )}
        </div>
      </details>

      {/* Recalled Facts */}
      <details className="intel-section">
        <summary className="intel-section__header">
          <span className="intel-section__icon">&#x1f4a1;</span>
          Recalled Facts
          {recalledFacts.length > 0 && (
            <span className="intel-badge">{recalledFacts.length}</span>
          )}
        </summary>
        <div className="intel-section__body">
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
      </details>

      {/* Subagents */}
      <details className="intel-section" open={subagents.some((s) => s.status === "active")}>
        <summary className="intel-section__header">
          <span className="intel-section__icon">&#x1f916;</span>
          Subagents
          {subagents.length > 0 && (
            <span className="intel-badge">{subagents.filter((s) => s.status === "active").length} active</span>
          )}
        </summary>
        <div className="intel-section__body">
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
                <span className="intel-subagent__task">{sa.task}</span>
              </div>
            ))
          )}
        </div>
      </details>

      {/* Plan */}
      <details className="intel-section" open>
        <summary className="intel-section__header">
          <span className="intel-section__icon">&#x1f4cb;</span>
          Execution Plan
          {plan.length > 0 && (
            <span className="intel-plan__progress">
              {plan.filter((s) => s.status === "done").length}/{plan.length}
            </span>
          )}
        </summary>
        <div className="intel-section__body">
          {plan.length === 0 ? (
            <div className="intel-empty">{"\u2014"}</div>
          ) : (
            plan.map((step, i) => (
              <div key={i} className={`intel-plan__step intel-plan__step--${step.status}`}>
                <span className="intel-plan__icon">{STEP_ICON[step.status] || "\u25CB"}</span>
                <span className="intel-plan__text">{step.text}</span>
              </div>
            ))
          )}
        </div>
      </details>
    </div>
  );
}
