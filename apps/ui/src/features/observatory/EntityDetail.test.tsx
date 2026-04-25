// ============================================================================
// EntityDetail — header, mention copy, close handler, properties section
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@/test/utils";
import type { GraphEntity } from "@/services/transport/types";

vi.mock("./graph-hooks", () => ({
  useEntityConnections: () => ({
    data: { neighbors: [] },
    loading: false,
    error: null,
  }),
}));

import { EntityDetail } from "./EntityDetail";

function makeEntity(overrides: Partial<GraphEntity> = {}): GraphEntity {
  return {
    id: "ent-1",
    agent_id: "agent:root",
    entity_type: "person",
    name: "Phani",
    mention_count: 5,
    properties: { role: "founder" },
    first_seen_at: "2026-01-15T00:00:00Z",
    last_seen_at: "2026-04-20T00:00:00Z",
  } as GraphEntity;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("EntityDetail", () => {
  it("returns null when entity is null", () => {
    const { container } = render(<EntityDetail entity={null} onClose={() => {}} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the entity name and type badge", () => {
    render(<EntityDetail entity={makeEntity()} onClose={() => {}} />);
    expect(screen.getByText("Phani")).toBeInTheDocument();
    expect(screen.getByText("person")).toBeInTheDocument();
  });

  it("pluralizes mention count correctly (5 mentions)", () => {
    render(<EntityDetail entity={makeEntity({ mention_count: 5 })} onClose={() => {}} />);
    expect(screen.getByText(/5 mentions/i)).toBeInTheDocument();
  });

  it("uses singular 'mention' for count of 1", () => {
    cleanup();
    const singleMention: GraphEntity = {
      id: "ent-2",
      agent_id: "agent:root",
      entity_type: "person",
      name: "Solo",
      mention_count: 1,
      properties: {},
      first_seen_at: "2026-01-15T00:00:00Z",
      last_seen_at: "2026-01-15T00:00:00Z",
    } as GraphEntity;
    const { container } = render(
      <EntityDetail entity={singleMention} onClose={() => {}} />
    );
    expect(container.textContent).toMatch(/1 mention(?!s)/);
  });

  it("calls onClose when the close (X) button is clicked", () => {
    const onClose = vi.fn();
    render(<EntityDetail entity={makeEntity()} onClose={onClose} />);
    // The close icon is inside a button with the .slideover__close class.
    const close = document.querySelector(".slideover__close") as HTMLButtonElement;
    expect(close).not.toBeNull();
    fireEvent.click(close);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders the Connections, Timeline, and Properties sections", () => {
    render(<EntityDetail entity={makeEntity()} onClose={() => {}} />);
    expect(screen.getByText("Connections")).toBeInTheDocument();
    expect(screen.getByText("Timeline")).toBeInTheDocument();
    expect(screen.getByText("Properties")).toBeInTheDocument();
  });

  it("renders 'No connections found' copy when neighbors list is empty", () => {
    render(<EntityDetail entity={makeEntity()} onClose={() => {}} />);
    expect(screen.getByText(/no connections found/i)).toBeInTheDocument();
  });

  it("renders property keys + values from entity.properties", () => {
    render(<EntityDetail entity={makeEntity()} onClose={() => {}} />);
    expect(screen.getByText("role")).toBeInTheDocument();
    expect(screen.getByText("founder")).toBeInTheDocument();
  });
});
