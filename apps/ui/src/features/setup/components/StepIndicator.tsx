const TOTAL_STEPS = 6;

interface StepIndicatorProps {
  currentStep: number;
}

export function StepIndicator({ currentStep }: StepIndicatorProps) {
  return (
    <div className="step-indicator">
      {Array.from({ length: TOTAL_STEPS }, (_, i) => {
        const step = i + 1;
        let className = "step-indicator__dot";
        if (step === currentStep) className += " step-indicator__dot--active";
        else if (step < currentStep) className += " step-indicator__dot--done";
        return <div key={step} className={className} />;
      })}
    </div>
  );
}
