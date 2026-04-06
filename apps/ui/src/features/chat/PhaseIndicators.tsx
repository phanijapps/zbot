// ============================================================================
// PHASE INDICATORS
// Shows 4-phase execution progress in the center panel.
// ============================================================================

import type { SubagentStateData } from "@/services/transport/types";

export type Phase = "idle" | "intent" | "planning" | "executing" | "responding" | "completed" | "error";

interface PhaseIndicatorsProps {
  phase: Phase;
  subagents?: SubagentStateData[];
}

interface PhaseStep {
  key: string;
  label: string;
  getDetail?: (props: PhaseIndicatorsProps) => string | null;
}

const STEPS: PhaseStep[] = [
  { key: "intent", label: "Analyzing intent" },
  {
    key: "planning",
    label: "Planning execution",
    getDetail: (p) => {
      const count = p.subagents?.length ?? 0;
      return count > 0 ? `${count} agent${count > 1 ? "s" : ""}` : null;
    },
  },
  {
    key: "executing",
    label: "Executing",
    getDetail: (p) => {
      const agents = p.subagents ?? [];
      if (agents.length === 0) return null;
      const done = agents.filter((a) => a.status === "completed").length;
      const active = agents.filter((a) => a.status === "running").map((a) => a.agentId);
      const parts: string[] = [];
      if (active.length > 0) parts.push(active.join(", "));
      parts.push(`(${done}/${agents.length} complete)`);
      return parts.join(" ");
    },
  },
  { key: "responding", label: "Generating response" },
];

const PHASE_ORDER = ["intent", "planning", "executing", "responding", "completed"];

function getStepStatus(stepKey: string, currentPhase: Phase): "done" | "active" | "pending" | "error" {
  if (currentPhase === "error") {
    const stepIdx = PHASE_ORDER.indexOf(stepKey);
    // Find the last phase that was active — mark it as error, earlier as done
    // For simplicity: mark all as done except responding which gets error
    if (stepIdx < PHASE_ORDER.indexOf("responding")) return "done";
    return "error";
  }
  if (currentPhase === "completed") return "done";
  if (currentPhase === "idle") return "pending";

  const currentIdx = PHASE_ORDER.indexOf(currentPhase);
  const stepIdx = PHASE_ORDER.indexOf(stepKey);
  if (stepIdx < currentIdx) return "done";
  if (stepIdx === currentIdx) return "active";
  return "pending";
}

export function PhaseIndicators({ phase, subagents }: PhaseIndicatorsProps) {
  if (phase === "idle") return null;

  return (
    <div className="phase-indicators">
      <div className="phase-indicators__label">Execution Progress</div>
      <div className="phase-indicators__steps">
        {STEPS.map((step) => {
          const status = getStepStatus(step.key, phase);
          const detail = step.getDetail?.({ phase, subagents });
          return (
            <div key={step.key} className={`phase-step phase-step--${status}`}>
              <div className={`phase-step__icon phase-step__icon--${status}`}>
                {status === "done" && <span>&#x2713;</span>}
                {status === "active" && <span className="phase-step__pulse" />}
                {status === "error" && <span>&#x2717;</span>}
              </div>
              <span className="phase-step__label">
                {step.label}
                {detail && <span className="phase-step__detail"> &mdash; {detail}</span>}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
