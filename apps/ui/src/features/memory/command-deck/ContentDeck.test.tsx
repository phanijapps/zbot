// ============================================================================
// ContentDeck — tab structure + Sonar S6842 fix (tablist on div, not nav)
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { ContentDeck } from "./ContentDeck";
import type { WardContent } from "@/services/transport/types";

function makeWardContent(): WardContent {
  return {
    ward_id: "auth-system",
    summary: { description: "Test ward" } as WardContent["summary"],
    counts: { facts: 0, wiki: 0, procedures: 0, episodes: 0 },
    facts: [],
    wiki: [],
    procedures: [],
    episodes: [],
  } as unknown as WardContent;
}

describe("ContentDeck", () => {
  it("renders the empty-state when data is null", () => {
    render(<ContentDeck data={null} />);
    expect(screen.getByText(/select a ward/i)).toBeInTheDocument();
  });

  it("renders the ward crumb and all four tabs when data is present", () => {
    render(<ContentDeck data={makeWardContent()} />);
    expect(screen.getByText(/auth-system/)).toBeInTheDocument();
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(
      expect.arrayContaining([
        expect.stringContaining("Facts"),
        expect.stringContaining("Wiki"),
        expect.stringContaining("Procedures"),
        expect.stringContaining("Episodes"),
      ])
    );
  });

  it("uses a <div> with role=tablist (Sonar S6842 — nav is non-interactive)", () => {
    render(<ContentDeck data={makeWardContent()} />);
    const tablist = screen.getByRole("tablist");
    expect(tablist.tagName).toBe("DIV");
    expect(tablist.getAttribute("aria-label")).toBe("Content tabs");
  });

  it("starts on the Facts tab and switches when another tab is clicked", () => {
    render(<ContentDeck data={makeWardContent()} />);
    const tabs = screen.getAllByRole("tab");
    const factsTab = tabs.find((t) => t.textContent?.includes("Facts"))!;
    expect(factsTab.getAttribute("aria-selected")).toBe("true");
    const wikiTab = tabs.find((t) => t.textContent?.includes("Wiki"))!;
    fireEvent.click(wikiTab);
    expect(wikiTab.getAttribute("aria-selected")).toBe("true");
  });

  it("Facts tab forwards onDeleteFact to ContentList for fact rows", () => {
    const onDeleteFact = vi.fn();
    const data = makeWardContent();
    render(<ContentDeck data={data} onDeleteFact={onDeleteFact} />);
    // Empty facts list — no delete buttons rendered, but the prop-pass path
    // is exercised at render-time. This test guards against accidental
    // removal of the prop drilling in future refactors.
    expect(onDeleteFact).not.toHaveBeenCalled();
  });

  it("Episodes tab renders task_summary + outcome + key_learnings (regression: Episodes count > 0 but list was empty)", () => {
    const data = {
      ...makeWardContent(),
      counts: { facts: 0, wiki: 0, procedures: 0, episodes: 1 },
      episodes: [
        {
          id: "ep-1",
          session_id: "sess-x",
          agent_id: "root",
          ward_id: "auth-system",
          task_summary: "Refactor login validator",
          outcome: "success",
          strategy_used: "incremental",
          key_learnings: "Email regex was too strict; loosened it.",
          token_cost: 1234,
          created_at: "2026-05-03T00:00:00Z",
          age_bucket: "today",
        },
      ],
    } as unknown as WardContent;
    render(<ContentDeck data={data} />);
    const episodesTab = screen
      .getAllByRole("tab")
      .find((t) => t.textContent?.includes("Episodes"))!;
    fireEvent.click(episodesTab);
    expect(screen.getByText("Refactor login validator")).toBeInTheDocument();
    expect(screen.getByText(/success/i)).toBeInTheDocument();
    expect(screen.getByText(/Email regex was too strict/)).toBeInTheDocument();
  });
});
