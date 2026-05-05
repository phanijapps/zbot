// =============================================================================
// ActionBar + FilterChip — small render + interaction tests.
// =============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { ActionBar, FilterChip } from "./ActionBar";

describe("<ActionBar>", () => {
  it("renders neither search nor right slot when no props are passed", () => {
    const { container } = render(<ActionBar />);
    expect(container.querySelector(".action-bar__search")).toBeNull();
    expect(container.querySelector(".action-bar__right")).toBeNull();
  });

  it("renders the search input when onSearchChange is provided", () => {
    render(<ActionBar onSearchChange={() => {}} />);
    expect(screen.getByPlaceholderText("Search...")).toBeInTheDocument();
  });

  it("uses the custom searchPlaceholder when provided", () => {
    render(<ActionBar onSearchChange={() => {}} searchPlaceholder="Find a session" />);
    expect(screen.getByPlaceholderText("Find a session")).toBeInTheDocument();
  });

  it("displays the searchValue and fires onSearchChange on input", () => {
    const onSearchChange = vi.fn();
    render(<ActionBar onSearchChange={onSearchChange} searchValue="initial" />);
    const input = screen.getByDisplayValue("initial") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "next" } });
    expect(onSearchChange).toHaveBeenCalledWith("next");
  });

  it("falls back to an empty string when searchValue is undefined", () => {
    render(<ActionBar onSearchChange={() => {}} />);
    const input = screen.getByPlaceholderText("Search...") as HTMLInputElement;
    expect(input.value).toBe("");
  });

  it("renders filters and actions slots when provided", () => {
    const { container } = render(
      <ActionBar
        filters={<span data-testid="filter-slot">filters here</span>}
        actions={<button data-testid="actions-slot">Go</button>}
      />,
    );
    expect(screen.getByTestId("filter-slot")).toBeInTheDocument();
    expect(screen.getByTestId("actions-slot")).toBeInTheDocument();
    expect(container.querySelector(".action-bar__right")).not.toBeNull();
  });
});

describe("<FilterChip>", () => {
  it("renders the label and fires onClick", () => {
    const onClick = vi.fn();
    render(<FilterChip label="Active" onClick={onClick} />);
    fireEvent.click(screen.getByText("Active"));
    expect(onClick).toHaveBeenCalled();
  });

  it("applies the --active modifier when active is true", () => {
    const { container } = render(<FilterChip label="Pinned" onClick={() => {}} active />);
    const btn = container.querySelector("button.filter-chip");
    expect(btn?.className).toContain("filter-chip--active");
  });

  it("omits the --active modifier when active is false / undefined", () => {
    const { container } = render(<FilterChip label="Closed" onClick={() => {}} />);
    const btn = container.querySelector("button.filter-chip");
    expect(btn?.className).not.toContain("filter-chip--active");
  });
});
