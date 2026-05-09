// ============================================================================
// WizardNav — render and interaction tests
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WizardNav } from "./WizardNav";

const defaultProps = {
  currentStep: 2,
  canNext: true,
  onBack: vi.fn(),
  onNext: vi.fn(),
};

describe("WizardNav", () => {
  it("renders Next button", () => {
    render(<WizardNav {...defaultProps} />);
    expect(screen.getByRole("button", { name: /next/i })).toBeInTheDocument();
  });

  it("renders Back button when currentStep > 1", () => {
    render(<WizardNav {...defaultProps} currentStep={2} />);
    expect(screen.getByRole("button", { name: /back/i })).toBeInTheDocument();
  });

  it("does not render Back button at step 1", () => {
    render(<WizardNav {...defaultProps} currentStep={1} />);
    expect(screen.queryByRole("button", { name: /back/i })).toBeNull();
  });

  it("calls onBack when Back is clicked", () => {
    const onBack = vi.fn();
    render(<WizardNav {...defaultProps} onBack={onBack} />);
    fireEvent.click(screen.getByRole("button", { name: /back/i }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("calls onNext when Next is clicked", () => {
    const onNext = vi.fn();
    render(<WizardNav {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole("button", { name: /next/i }));
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("disables Next when canNext is false", () => {
    render(<WizardNav {...defaultProps} canNext={false} />);
    expect(screen.getByRole("button", { name: /next/i })).toBeDisabled();
  });

  it("disables all buttons when isLoading is true", () => {
    render(<WizardNav {...defaultProps} isLoading={true} />);
    expect(screen.getByRole("button", { name: /back/i })).toBeDisabled();
    // The next/submit button shows "..." when loading
    const buttons = screen.getAllByRole("button");
    const disabledButtons = buttons.filter((b) => b.hasAttribute("disabled"));
    expect(disabledButtons.length).toBeGreaterThanOrEqual(2);
  });

  it("shows '...' text when loading", () => {
    render(<WizardNav {...defaultProps} isLoading={true} />);
    expect(screen.getByText("...")).toBeInTheDocument();
  });

  it("renders custom nextLabel", () => {
    render(<WizardNav {...defaultProps} nextLabel="Launch" />);
    expect(screen.getByRole("button", { name: /launch/i })).toBeInTheDocument();
  });

  it("renders Skip button when onSkip is provided", () => {
    const onSkip = vi.fn();
    render(<WizardNav {...defaultProps} onSkip={onSkip} />);
    expect(screen.getByText(/skip/i)).toBeInTheDocument();
  });

  it("shows 'Skip setup' text on step 1", () => {
    const onSkip = vi.fn();
    render(<WizardNav {...defaultProps} currentStep={1} onSkip={onSkip} />);
    expect(screen.getByText("Skip setup")).toBeInTheDocument();
  });

  it("shows 'Skip' text on step > 1", () => {
    const onSkip = vi.fn();
    render(<WizardNav {...defaultProps} currentStep={2} onSkip={onSkip} />);
    expect(screen.getByText("Skip")).toBeInTheDocument();
  });

  it("calls onSkip when Skip is clicked", () => {
    const onSkip = vi.fn();
    render(<WizardNav {...defaultProps} onSkip={onSkip} />);
    fireEvent.click(screen.getByText(/skip/i));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });

  it("does not render Skip button when onSkip is not provided", () => {
    render(<WizardNav {...defaultProps} />);
    expect(screen.queryByText(/skip/i)).toBeNull();
  });
});
