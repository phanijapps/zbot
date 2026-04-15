import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryItemCard } from "../MemoryItemCard";

describe("MemoryItemCard", () => {
  it("renders content, category badge, age, and match_source when given", () => {
    render(
      <MemoryItemCard
        id="f1"
        content="Hormuz geofence is 24-27N, 54-58E"
        category="instruction"
        confidence={0.9}
        created_at="2026-04-15T10:00:00Z"
        age_bucket="today"
        match_source="hybrid"
      />
    );
    expect(screen.getByText(/Hormuz geofence/)).toBeInTheDocument();
    expect(screen.getByText(/instruction/i)).toBeInTheDocument();
    expect(screen.getByText(/hybrid/i)).toBeInTheDocument();
  });

  it("applies decay class based on age_bucket", () => {
    const { container } = render(
      <MemoryItemCard id="f2" content="x" category="pattern" confidence={1}
        created_at="2026-03-01T00:00:00Z" age_bucket="historical" />
    );
    expect(container.querySelector(".memory-item")).toHaveClass("decay-2");
  });
});
