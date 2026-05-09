// ============================================================================
// NameStep — render and interaction tests
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { NameStep } from "./NameStep";
import { NAME_PRESETS } from "../presets";

const defaultProps = {
  agentName: "z-Bot",
  namePreset: "zbot",
  aboutMe: "",
  onChange: vi.fn(),
  onAboutMeChange: vi.fn(),
};

describe("NameStep", () => {
  it("renders all name presets", () => {
    render(<NameStep {...defaultProps} />);
    for (const preset of NAME_PRESETS) {
      expect(screen.getByText(preset.name)).toBeInTheDocument();
    }
  });

  it("shows agent name in the input", () => {
    render(<NameStep {...defaultProps} agentName="My Agent" />);
    expect(screen.getByLabelText("Agent Name")).toHaveValue("My Agent");
  });

  it("shows aboutMe in the textarea", () => {
    render(<NameStep {...defaultProps} aboutMe="I am a developer" />);
    expect(screen.getByLabelText("About You")).toHaveValue("I am a developer");
  });

  it("calls onChange when a preset is clicked", () => {
    const onChange = vi.fn();
    render(<NameStep {...defaultProps} onChange={onChange} />);
    // Click the z-Bot preset (id: "zbot")
    const zBot = NAME_PRESETS.find((p) => p.id === "zbot")!;
    fireEvent.click(screen.getByText(zBot.name));
    expect(onChange).toHaveBeenCalledWith(zBot.name, "zbot");
  });

  it("calls onChange with empty name and 'custom' when Custom preset is clicked", () => {
    const onChange = vi.fn();
    render(<NameStep {...defaultProps} onChange={onChange} />);
    const custom = NAME_PRESETS.find((p) => p.id === "custom")!;
    fireEvent.click(screen.getByText(custom.name));
    expect(onChange).toHaveBeenCalledWith("", "custom");
  });

  it("calls onChange when typing in the name input", () => {
    const onChange = vi.fn();
    render(<NameStep {...defaultProps} onChange={onChange} />);
    const input = screen.getByLabelText("Agent Name");
    fireEvent.change(input, { target: { value: "Nova" } });
    expect(onChange).toHaveBeenCalled();
  });

  it("calls onAboutMeChange when typing in the About You textarea", () => {
    const onAboutMeChange = vi.fn();
    render(<NameStep {...defaultProps} onAboutMeChange={onAboutMeChange} />);
    const textarea = screen.getByLabelText("About You");
    fireEvent.change(textarea, { target: { value: "Hello world" } });
    expect(onAboutMeChange).toHaveBeenCalledWith("Hello world");
  });

  it("handles Enter keydown on a preset", () => {
    const onChange = vi.fn();
    render(<NameStep {...defaultProps} onChange={onChange} />);
    const zBot = NAME_PRESETS.find((p) => p.id === "zbot")!;
    const presetEl = screen.getByText(zBot.name).closest("[role='button']")!;
    fireEvent.keyDown(presetEl, { key: "Enter" });
    expect(onChange).toHaveBeenCalled();
  });

  it("handles Space keydown on a preset", () => {
    const onChange = vi.fn();
    render(<NameStep {...defaultProps} onChange={onChange} />);
    const zBot = NAME_PRESETS.find((p) => p.id === "zbot")!;
    const presetEl = screen.getByText(zBot.name).closest("[role='button']")!;
    fireEvent.keyDown(presetEl, { key: " " });
    expect(onChange).toHaveBeenCalled();
  });

  it("marks selected preset with active class", () => {
    render(<NameStep {...defaultProps} namePreset="zbot" />);
    const zBot = NAME_PRESETS.find((p) => p.id === "zbot")!;
    const presetEl = screen.getByText(zBot.name).closest(".name-preset")!;
    expect(presetEl.className).toContain("name-preset--selected");
  });
});
