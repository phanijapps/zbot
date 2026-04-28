// ============================================================================
// PersistenceCard tests
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";

import { PersistenceCard } from "./PersistenceCard";

describe("PersistenceCard", () => {
  it("renders the Persistence header", () => {
    render(<PersistenceCard />);
    expect(
      screen.getByRole("heading", { name: /persistence/i, level: 2 }),
    ).toBeInTheDocument();
  });

  it("renders the Knowledge Backend dropdown defaulting to SQLite", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    expect(select).toBeInTheDocument();
    expect(select.value).toBe("sqlite");
    expect(select.disabled).toBe(false);
  });

  it("offers both SQLite and SurrealDB as enabled options", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    const options = Array.from(select.options).map((o) => ({
      value: o.value,
      disabled: o.disabled,
    }));
    expect(options).toEqual([
      { value: "sqlite", disabled: false },
      { value: "surreal", disabled: false },
    ]);
  });

  it("shows the SQLite hint by default and no rebuild banner", () => {
    render(<PersistenceCard />);
    expect(
      screen.getByText(/SQLite is the only backend wired/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/SurrealDB requires a daemon rebuild/i),
    ).not.toBeInTheDocument();
  });

  it("reveals the rebuild banner when SurrealDB is selected", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "surreal" } });
    expect(
      screen.getByText(/SurrealDB requires a daemon rebuild/i),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/cargo run -p daemon --features surreal-backend/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/SQLite is the only backend wired/i),
    ).not.toBeInTheDocument();
  });

  it("hides the rebuild banner when switching back to SQLite", () => {
    render(<PersistenceCard />);
    const select = screen.getByLabelText(/knowledge backend/i) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "surreal" } });
    fireEvent.change(select, { target: { value: "sqlite" } });
    expect(
      screen.queryByText(/SurrealDB requires a daemon rebuild/i),
    ).not.toBeInTheDocument();
  });

  it("documents the recovery path", () => {
    render(<PersistenceCard />);
    expect(screen.getByText(/Recovery:/i)).toBeInTheDocument();
    expect(
      screen.getByText(/zero-stores-surreal-recovery/i),
    ).toBeInTheDocument();
  });
});
