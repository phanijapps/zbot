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
      <div className="flex items-center gap-3">
        {currentStep > 1 && (
          <button className="btn btn--ghost btn--sm" onClick={onBack} disabled={isLoading}>
            &larr; Back
          </button>
        )}
        {onSkip && (
          <button className="setup-wizard__skip" onClick={onSkip}>
            {currentStep === 1 ? "Skip setup" : "Skip"}
          </button>
        )}
      </div>
      <button
        className="btn btn--primary btn--sm"
        onClick={onNext}
        disabled={!canNext || isLoading}
      >
        {isLoading ? "..." : nextLabel || "Next \u2192"}
      </button>
    </div>
  );
}
