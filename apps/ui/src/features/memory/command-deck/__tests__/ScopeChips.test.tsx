import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ScopeChips } from "../ScopeChips";

describe("ScopeChips", () => {
  it("toggles a type chip off when clicked", () => {
    const onChange = vi.fn();
    render(<ScopeChips types={["facts", "wiki"]} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: /facts/i }));
    expect(onChange).toHaveBeenCalledWith({ types: ["wiki"] });
  });

  it("toggles a type chip on when clicked", () => {
    const onChange = vi.fn();
    render(<ScopeChips types={["facts"]} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: /procedures/i }));
    expect(onChange).toHaveBeenCalledWith({ types: ["facts", "procedures"] });
  });
});
