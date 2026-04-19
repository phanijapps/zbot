// =============================================================================
// ArtifactStrip — R14d tests
//
// Pure presentational component. Covers:
//  - hidden when artifacts[] is empty
//  - renders N chips (0/1/3)
//  - clicking a chip fires onOpen with the matching artifact
//  - each chip is a <button type="button"> with aria-label including filename
// =============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ArtifactStrip } from "./ArtifactStrip";
import type { ResearchArtifactRef } from "./types";

function refs(n: number): ResearchArtifactRef[] {
  return Array.from({ length: n }, (_, i) => ({
    id: `art-${i}`,
    fileName: `file-${i}.md`,
    fileType: "md",
  }));
}

describe("<ArtifactStrip>", () => {
  it("renders nothing when artifacts[] is empty", () => {
    const { container } = render(<ArtifactStrip artifacts={[]} onOpen={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders a single chip when artifacts has one entry", () => {
    render(<ArtifactStrip artifacts={refs(1)} onOpen={vi.fn()} />);
    const chips = screen.getAllByRole("listitem");
    expect(chips).toHaveLength(1);
    expect(chips[0].textContent).toContain("file-0.md");
  });

  it("renders three chips when artifacts has three entries", () => {
    render(<ArtifactStrip artifacts={refs(3)} onOpen={vi.fn()} />);
    const chips = screen.getAllByRole("listitem");
    expect(chips).toHaveLength(3);
  });

  it("clicking a chip calls onOpen with the matching artifact", () => {
    const onOpen = vi.fn();
    const artifacts = refs(3);
    render(<ArtifactStrip artifacts={artifacts} onOpen={onOpen} />);
    const chips = screen.getAllByRole("listitem");
    fireEvent.click(chips[1]);
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith(artifacts[1]);
  });

  it("each chip is a <button type=button> with aria-label including the filename", () => {
    render(<ArtifactStrip artifacts={refs(2)} onOpen={vi.fn()} />);
    const buttons = screen.getAllByRole("listitem") as HTMLButtonElement[];
    for (const btn of buttons) {
      expect(btn.tagName).toBe("BUTTON");
      expect(btn.type).toBe("button");
      expect(btn.getAttribute("aria-label")).toMatch(/Open artifact file-\d\.md/);
    }
  });

  it("chip list has a descriptive aria-label on the container", () => {
    render(<ArtifactStrip artifacts={refs(1)} onOpen={vi.fn()} />);
    const list = screen.getByRole("list");
    expect(list.getAttribute("aria-label")).toBe("Session artifacts");
  });
});
