import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SearchBar } from "../SearchBar";

describe("SearchBar", () => {
  it("fires onChange with query and current mode", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "hormuz" } });
    expect(onChange).toHaveBeenLastCalledWith({ query: "hormuz", mode: "hybrid" });
  });

  it("switches mode to fts when FTS tab clicked", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: /fts/i }));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "q" } });
    expect(onChange).toHaveBeenLastCalledWith({ query: "q", mode: "fts" });
  });

  it("detects quoted phrase and emits fts mode even when hybrid selected", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: '"exact phrase"' } });
    const calls = onChange.mock.calls;
    const last = calls[calls.length - 1]?.[0];
    expect(last.mode).toBe("fts");
  });
});
