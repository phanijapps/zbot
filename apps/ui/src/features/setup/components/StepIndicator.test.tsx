// ============================================================================
// StepIndicator — render tests
// ============================================================================

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { StepIndicator } from "./StepIndicator";

describe("StepIndicator", () => {
  it("renders 6 step dots", () => {
    const { container } = render(<StepIndicator currentStep={1} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    expect(dots).toHaveLength(6);
  });

  it("marks the current step dot as active", () => {
    const { container } = render(<StepIndicator currentStep={3} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    expect(dots[2].className).toContain("step-indicator__dot--active");
  });

  it("marks prior step dots as done", () => {
    const { container } = render(<StepIndicator currentStep={4} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    expect(dots[0].className).toContain("step-indicator__dot--done");
    expect(dots[1].className).toContain("step-indicator__dot--done");
    expect(dots[2].className).toContain("step-indicator__dot--done");
  });

  it("does not mark future step dots as active or done", () => {
    const { container } = render(<StepIndicator currentStep={2} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    // dots[2] and beyond should be plain
    for (let i = 2; i < 6; i++) {
      expect(dots[i].className).not.toContain("--active");
      expect(dots[i].className).not.toContain("--done");
    }
  });

  it("works at step 1 (first step)", () => {
    const { container } = render(<StepIndicator currentStep={1} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    expect(dots[0].className).toContain("step-indicator__dot--active");
    expect(dots[1].className).not.toContain("--active");
  });

  it("works at step 6 (last step)", () => {
    const { container } = render(<StepIndicator currentStep={6} />);
    const dots = container.querySelectorAll(".step-indicator__dot");
    expect(dots[5].className).toContain("step-indicator__dot--active");
    for (let i = 0; i < 5; i++) {
      expect(dots[i].className).toContain("step-indicator__dot--done");
    }
  });
});
