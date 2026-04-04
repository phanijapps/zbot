interface WizardNavProps {
  currentStep: number;
  canNext: boolean;
  isLoading?: boolean;
  nextLabel?: string;
  onBack: () => void;
  onNext: () => void;
  onSkip?: () => void;
}

export function WizardNav({
  currentStep,
  canNext,
  isLoading,
  nextLabel,
  onBack,
  onNext,
  onSkip,
}: WizardNavProps) {
  return (
    <div className="setup-wizard__footer">
      <div>
        {currentStep > 1 ? (
          <button className="btn btn--ghost btn--sm" onClick={onBack} disabled={isLoading}>
            &larr; Back
          </button>
        ) : onSkip ? (
          <button className="setup-wizard__skip" onClick={onSkip}>
            Skip setup
          </button>
        ) : (
          <div />
        )}
      </div>
      <div className="flex items-center gap-3">
        {onSkip && currentStep > 1 && (
          <button className="setup-wizard__skip" onClick={onSkip}>
            Skip
          </button>
        )}
        <button
          className="btn btn--primary btn--sm"
          onClick={onNext}
          disabled={!canNext || isLoading}
        >
          {isLoading ? "..." : nextLabel || "Next \u2192"}
        </button>
      </div>
    </div>
  );
}
