// ============================================================================
// SkillsStep — render and interaction tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import type { Transport } from "@/services/transport";

const listSkills = vi.fn<Transport["listSkills"]>();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ listSkills }),
}));

import { SkillsStep } from "./SkillsStep";

function makeSkill(id: string, name: string, category = "core") {
  return {
    id,
    name,
    displayName: name,
    description: `Description of ${name}`,
    category,
    enabled: true,
    content: "",
    filePath: "",
  };
}

describe("SkillsStep", () => {
  beforeEach(() => {
    listSkills.mockReset();
  });

  it("shows loading spinner initially", () => {
    listSkills.mockReturnValue(new Promise(() => { /* never resolves */ }));
    const { container } = render(
      <SkillsStep enabledSkillIds={[]} onChange={vi.fn()} />
    );
    expect(container.querySelector(".settings-loading")).toBeInTheDocument();
  });

  it("renders skills after loading", async () => {
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("skill-1", "Bash Runner", "tools")],
    });
    render(<SkillsStep enabledSkillIds={[]} onChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Bash Runner")).toBeInTheDocument();
    });
  });

  it("renders empty hint when no skills available", async () => {
    listSkills.mockResolvedValue({ success: true, data: [] });
    render(<SkillsStep enabledSkillIds={[]} onChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/no skills installed/i)).toBeInTheDocument();
    });
  });

  it("selects all skills by default on first load when none enabled", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Skill One"), makeSkill("s2", "Skill Two")],
    });
    render(<SkillsStep enabledSkillIds={[]} onChange={onChange} />);
    await waitFor(() => {
      expect(onChange).toHaveBeenCalledWith(["s1", "s2"]);
    });
  });

  it("does not auto-select when some skills are already enabled", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Skill One"), makeSkill("s2", "Skill Two")],
    });
    render(<SkillsStep enabledSkillIds={["s1"]} onChange={onChange} />);
    await waitFor(() => {
      expect(screen.getByText("Skill One")).toBeInTheDocument();
    });
    // onChange should NOT have been called with all ids since we already have some
    const allIds = onChange.mock.calls.find((call) => call[0].length === 2);
    expect(allIds).toBeUndefined();
  });

  it("toggles individual skill off when clicked (was on)", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Skill One")],
    });
    render(<SkillsStep enabledSkillIds={["s1"]} onChange={onChange} />);
    await waitFor(() => screen.getByText("Skill One"));
    fireEvent.click(screen.getByText("Skill One").closest("[role='button']")!);
    expect(onChange).toHaveBeenCalledWith([]);
  });

  it("toggles individual skill on when clicked (was off)", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Skill One")],
    });
    render(<SkillsStep enabledSkillIds={[]} onChange={onChange} />);
    // Wait for auto-select to fire, then reset
    await waitFor(() => screen.getByText("Skill One"));
    onChange.mockClear();
    fireEvent.click(screen.getByText("Skill One").closest("[role='button']")!);
    expect(onChange).toHaveBeenCalledWith(["s1"]);
  });

  it("groups skills by category", async () => {
    listSkills.mockResolvedValue({
      success: true,
      data: [
        makeSkill("s1", "Alpha", "tools"),
        makeSkill("s2", "Beta", "analysis"),
      ],
    });
    render(<SkillsStep enabledSkillIds={[]} onChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("tools")).toBeInTheDocument();
      expect(screen.getByText("analysis")).toBeInTheDocument();
    });
  });

  it("handles Select all / Deselect all for a category", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Alpha", "tools"), makeSkill("s2", "Beta", "tools")],
    });
    render(<SkillsStep enabledSkillIds={["s1", "s2"]} onChange={onChange} />);
    await waitFor(() => screen.getByText("Deselect all"));
    fireEvent.click(screen.getByText("Deselect all"));
    expect(onChange).toHaveBeenCalledWith([]);
  });

  it("handles keyboard activation on skill rows", async () => {
    const onChange = vi.fn();
    listSkills.mockResolvedValue({
      success: true,
      data: [makeSkill("s1", "Skill One")],
    });
    render(<SkillsStep enabledSkillIds={[]} onChange={onChange} />);
    await waitFor(() => screen.getByText("Skill One"));
    onChange.mockClear();
    const row = screen.getByText("Skill One").closest("[role='button']")!;
    fireEvent.keyDown(row, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(["s1"]);
  });
});
