// ============================================================================
// INTENT ANALYSIS BLOCK
// Renders intent analysis data within the execution narrative.
// Supports streaming (skeleton) and complete (full data) states.
// ============================================================================

import type { IntentAnalysis } from "./mission-hooks";

// ============================================================================
// Types
// ============================================================================

export interface IntentAnalysisBlockProps {
  /** The intent analysis data — null while streaming */
  analysis: IntentAnalysis | null;
  /** Whether the analysis is still being computed */
  isStreaming?: boolean;
}

// ============================================================================
// Helpers
// ============================================================================

const APPROACH_LABELS: Record<string, string> = {
  simple: "Simple",
  graph: "Graph",
  // Legacy values for session replay of older sessions
  tracked: "Tracked",
  multi_step: "Multi-Step",
  parallel: "Parallel",
  sequential: "Sequential",
  complex: "Complex",
};

function approachLabel(approach: string): string {
  return APPROACH_LABELS[approach] ?? approach;
}

// ============================================================================
// Component
// ============================================================================

/**
 * IntentAnalysisBlock -- shows the agent's understanding of the user's intent.
 *
 * Streaming state: skeleton with pulsing header.
 * Complete state: primary intent, hidden intents, recommended skills/agents,
 *   ward recommendation, and execution strategy summary.
 */
export function IntentAnalysisBlock({ analysis, isStreaming }: IntentAnalysisBlockProps) {
  // -- Streaming skeleton --
  if (isStreaming || !analysis) {
    return (
      <div className="intent-analysis-block intent-analysis-block--streaming">
        <div className="intent-analysis-block__header">
          <span className="intent-analysis-block__icon">&#9889;</span>
          <span>Analyzing intent...</span>
        </div>
        <div className="intent-analysis-block__skeleton">
          <div className="intent-analysis-block__skeleton-line" />
          <div className="intent-analysis-block__skeleton-line intent-analysis-block__skeleton-line--short" />
        </div>
      </div>
    );
  }

  // -- Complete state --
  const hasHiddenIntents = analysis.hiddenIntents.length > 0;
  const hasSkills = analysis.recommendedSkills.length > 0;
  const hasAgents = analysis.recommendedAgents.length > 0;
  const hasStrategy = analysis.executionStrategy.explanation.length > 0;

  return (
    <div className="intent-analysis-block">
      <div className="intent-analysis-block__header">
        <span className="intent-analysis-block__icon">&#9889;</span>
        <span>Intent Analysis</span>
        <span className="intent-analysis-block__approach-badge">
          {approachLabel(analysis.executionStrategy.approach)}
        </span>
      </div>

      {/* Primary intent */}
      <div className="intent-analysis-block__primary">
        {analysis.primaryIntent || "Unknown intent"}
      </div>

      {/* Hidden intents */}
      {hasHiddenIntents && (
        <div className="intent-analysis-block__section">
          <span className="intent-analysis-block__label">Hidden intents:</span>
          <div className="intent-analysis-block__tags">
            {analysis.hiddenIntents.map((intent, i) => (
              <span key={i} className="intent-analysis-block__tag intent-analysis-block__tag--hidden">
                {intent}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Recommended skills */}
      {hasSkills && (
        <div className="intent-analysis-block__section">
          <span className="intent-analysis-block__label">Skills:</span>
          <div className="intent-analysis-block__tags">
            {analysis.recommendedSkills.map((skill, i) => (
              <span key={i} className="intent-analysis-block__tag intent-analysis-block__tag--skill">
                {skill}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Recommended agents */}
      {hasAgents && (
        <div className="intent-analysis-block__section">
          <span className="intent-analysis-block__label">Agents:</span>
          <div className="intent-analysis-block__tags">
            {analysis.recommendedAgents.map((agent, i) => (
              <span key={i} className="intent-analysis-block__tag intent-analysis-block__tag--agent">
                {agent}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Ward recommendation */}
      {analysis.wardRecommendation.wardName && (
        <div className="intent-analysis-block__section">
          <span className="intent-analysis-block__label">Ward:</span>
          <span className="intent-analysis-block__ward">
            {analysis.wardRecommendation.wardName}
          </span>
          {analysis.wardRecommendation.reason && (
            <span className="intent-analysis-block__ward-reason">
              &mdash; {analysis.wardRecommendation.reason}
            </span>
          )}
        </div>
      )}

      {/* Execution strategy explanation */}
      {hasStrategy && (
        <div className="intent-analysis-block__strategy">
          {analysis.executionStrategy.explanation}
        </div>
      )}
    </div>
  );
}
