import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ContentList } from "../ContentList";

describe("ContentList", () => {
  it("groups items by age_bucket with headers", () => {
    render(
      <ContentList
        items={[
          {
            id: "a",
            content: "recent",
            category: "instruction",
            confidence: 1,
            created_at: "2026-04-15T12:00:00Z",
            age_bucket: "today",
          },
          {
            id: "b",
            content: "mid",
            category: "pattern",
            confidence: 0.9,
            created_at: "2026-04-13T12:00:00Z",
            age_bucket: "last_7_days",
          },
          {
            id: "c",
            content: "old",
            category: "decision",
            confidence: 0.9,
            created_at: "2026-02-15T12:00:00Z",
            age_bucket: "historical",
          },
        ]}
      />,
    );
    expect(screen.getByText(/^TODAY$/)).toBeInTheDocument();
    expect(screen.getByText(/LAST 7 DAYS/i)).toBeInTheDocument();
    expect(screen.getByText(/^HISTORICAL$/)).toBeInTheDocument();
  });

  it("hides historical items when timewarpDays is 5", () => {
    render(
      <ContentList
        timewarpDays={5}
        items={[
          {
            id: "a",
            content: "recent",
            category: "instruction",
            confidence: 1,
            created_at: "2026-04-15T12:00:00Z",
            age_bucket: "today",
          },
          {
            id: "b",
            content: "mid",
            category: "pattern",
            confidence: 0.9,
            created_at: "2026-04-13T12:00:00Z",
            age_bucket: "last_7_days",
          },
          {
            id: "c",
            content: "old",
            category: "decision",
            confidence: 0.9,
            created_at: "2026-02-15T12:00:00Z",
            age_bucket: "historical",
          },
        ]}
      />,
    );
    expect(screen.getByText(/^TODAY$/)).toBeInTheDocument();
    expect(screen.getByText(/LAST 7 DAYS/i)).toBeInTheDocument();
    expect(screen.queryByText(/^HISTORICAL$/)).not.toBeInTheDocument();
  });

  it("does not render an empty group", () => {
    render(
      <ContentList
        items={[
          {
            id: "a",
            content: "x",
            category: "pattern",
            confidence: 1,
            created_at: "2026-04-15T12:00:00Z",
            age_bucket: "today",
          },
        ]}
      />,
    );
    expect(screen.queryByText(/^HISTORICAL$/)).not.toBeInTheDocument();
  });
});
