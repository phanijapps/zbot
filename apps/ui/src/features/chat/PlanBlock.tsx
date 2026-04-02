export type StepStatus = "done" | "active" | "pending";

export interface PlanStep {
  /** Step description */
  text: string;
  /** Step status */
  status: StepStatus;
}

export interface PlanBlockProps {
  /** Ordered list of plan steps */
  steps: PlanStep[];
}

/** Status icon map */
const STATUS_ICON: Record<StepStatus, string> = {
  done: "\u2713",
  active: "\u27F3",
  pending: "\u25CB",
};

/**
 * PlanBlock — checklist-style execution plan.
 * Each step shows a status icon: done (line-through), active (amber), pending (muted).
 */
export function PlanBlock({ steps }: PlanBlockProps) {
  if (steps.length === 0) return null;

  return (
    <div className="plan-block">
      <div className="plan-block__header">Execution Plan</div>
      {steps.map((step, i) => (
        <div key={i} className={`plan-block__step plan-block__step--${step.status}`}>
          <span>{STATUS_ICON[step.status]}</span>
          <span>{step.text}</span>
        </div>
      ))}
    </div>
  );
}
