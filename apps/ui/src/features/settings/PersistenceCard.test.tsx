// ============================================================================
// PersistenceCard tests
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen } from "@/test/utils";

import { PersistenceCard } from "./PersistenceCard";

describe("PersistenceCard", () => {
  it("renders the Persistence header", () => {
    render(<PersistenceCard />);
    expect(
      screen.getByRole("heading", { name: /persistence/i, level: 2 }),
    ).toBeInTheDocument();
  });

  it("describes the experimental SurrealDB backend in a warning banner", () => {
    render(<PersistenceCard />);
    expect(
      screen.getByText(/SurrealDB backend is experimental/i),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/cargo run -p daemon --features surreal-backend/i),
    ).toBeInTheDocument();
  });

  it("renders a disabled Backend dropdown locked to SQLite", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    expect(select).toBeInTheDocument();
    expect(select.disabled).toBe(true);
    expect(select.value).toBe("sqlite");
  });

  it("offers SurrealDB as a disabled option in the dropdown", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    const surrealOption = Array.from(select.options).find(
      (o) => o.value === "surreal",
    );
    expect(surrealOption).toBeDefined();
    expect(surrealOption?.disabled).toBe(true);
  });

  it("documents the recovery path", () => {
    render(<PersistenceCard />);
    expect(screen.getByText(/Recovery:/i)).toBeInTheDocument();
    expect(
      screen.getByText(/zero-stores-surreal-recovery/i),
    ).toBeInTheDocument();
  });

  it("explains that the SurrealDB option requires a feature flag build", () => {
    render(<PersistenceCard />);
    expect(
      screen.getByText(/persistence_factory::build_surreal_pair/i),
    ).toBeInTheDocument();
  });
});
