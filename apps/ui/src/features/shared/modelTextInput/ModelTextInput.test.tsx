import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ModelTextInput } from "./ModelTextInput";

describe("ModelTextInput", () => {
  it("renders as a text input (not a select)", () => {
    render(
      <ModelTextInput value="gpt-4" onChange={() => {}} suggestions={["gpt-4", "gpt-4o"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    expect(input.tagName).toBe("INPUT");
    expect((input as HTMLInputElement).type).toBe("text");
    expect((input as HTMLInputElement).value).toBe("gpt-4");
  });

  it("accepts a value not present in suggestions", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="" onChange={onChange} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.change(input, { target: { value: "nemotron-super:cloud" } });
    expect(onChange).toHaveBeenCalledWith("nemotron-super:cloud");
  });

  it("accepts an empty value (provider default)", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="gpt-4" onChange={onChange} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.change(input, { target: { value: "" } });
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("opens the suggestion list on focus and filters as the user types", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["gpt-4", "gpt-4o-mini", "claude-3-opus"]}
        id="m"
      />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    expect(screen.getByRole("listbox")).toBeTruthy();
    expect(screen.getByText("gpt-4o-mini")).toBeTruthy();
    fireEvent.change(input, { target: { value: "claude" } });
    expect(screen.queryByText("gpt-4o-mini")).toBeNull();
    expect(screen.getByText("claude-3-opus")).toBeTruthy();
  });

  it("commits the highlighted suggestion on Enter", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="" onChange={onChange} suggestions={["gpt-4", "gpt-4o"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("gpt-4");
  });

  it("closes the suggestion list on Escape", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("uses div elements for the listbox + options (Sonar S6842 — non-interactive ul/li shouldn't carry interactive roles)", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["gpt-4", "gpt-4o"]} id="m" />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    const listbox = screen.getByRole("listbox");
    expect(listbox.tagName).toBe("DIV");
    const options = screen.getAllByRole("option");
    expect(options.length).toBeGreaterThan(0);
    for (const opt of options) expect(opt.tagName).toBe("DIV");
  });

  // Regression: every option carries tabIndex=-1 so the role="option"
  // element is in the focus tree without being in the tab order
  // (Sonar S6852 — interactive roles must be focusable).
  it("each option element is focusable via tabIndex=-1", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a", "b", "c"]} id="m" />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(3);
    for (const opt of options) {
      expect(opt.getAttribute("tabindex")).toBe("-1");
    }
  });

  // -----------------------------------------------------------------
  // Filtering — case-insensitive, empty-friendly, substring match.
  // -----------------------------------------------------------------

  it("filter is case-insensitive and matches substrings", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["GPT-4", "Claude-3-Opus", "Llama-3-70B"]}
        id="m"
      />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "OPUS" } });
    expect(screen.getByText("Claude-3-Opus")).toBeTruthy();
    expect(screen.queryByText("GPT-4")).toBeNull();
  });

  it("renders all suggestions when input is empty", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["a", "b", "c"]}
        id="m"
      />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    expect(screen.getAllByRole("option")).toHaveLength(3);
  });

  it("hides the listbox entirely when no suggestion matches", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["gpt-4"]}
        id="m"
      />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "no-such-model" } });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("does not crash when the suggestions array is empty", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={[]} id="m" />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  // -----------------------------------------------------------------
  // Keyboard navigation
  // -----------------------------------------------------------------

  it("ArrowDown opens the list when it was closed", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a", "b"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("listbox")).toBeTruthy();
  });

  it("ArrowDown advances highlight and stops at the last option", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a", "b"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.keyDown(input, { key: "ArrowDown" }); // -> 0
    fireEvent.keyDown(input, { key: "ArrowDown" }); // -> 1
    fireEvent.keyDown(input, { key: "ArrowDown" }); // stays at 1
    const opts = screen.getAllByRole("option");
    expect(opts[0].getAttribute("aria-selected")).toBe("false");
    expect(opts[1].getAttribute("aria-selected")).toBe("true");
  });

  it("ArrowUp decreases highlight and clamps at -1 (no selection)", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a", "b"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.keyDown(input, { key: "ArrowDown" }); // -> 0
    fireEvent.keyDown(input, { key: "ArrowUp" }); // -> -1
    fireEvent.keyDown(input, { key: "ArrowUp" }); // stays at -1
    for (const opt of screen.getAllByRole("option")) {
      expect(opt.getAttribute("aria-selected")).toBe("false");
    }
  });

  it("Enter without highlight does NOT commit (free-text intent)", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput
        value="custom-model"
        onChange={onChange}
        suggestions={["gpt-4"]}
        id="m"
      />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    onChange.mockClear(); // ignore the focus-time effects
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("Escape on a closed list is a no-op (does not throw)", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    expect(() => fireEvent.keyDown(input, { key: "Escape" })).not.toThrow();
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  // -----------------------------------------------------------------
  // Mouse interactions
  // -----------------------------------------------------------------

  it("mouseDown on an option commits its value (and prevents blur)", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput
        value=""
        onChange={onChange}
        suggestions={["gpt-4o", "claude-3"]}
        id="m"
      />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    fireEvent.mouseDown(screen.getByText("claude-3"));
    expect(onChange).toHaveBeenCalledWith("claude-3");
    // Listbox closes after commit.
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("mouseEnter on an option moves the highlight (preview)", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["a", "b", "c"]}
        id="m"
      />,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    fireEvent.mouseEnter(screen.getByText("c"));
    const opts = screen.getAllByRole("option");
    expect(opts[2].getAttribute("aria-selected")).toBe("true");
    expect(opts[0].getAttribute("aria-selected")).toBe("false");
  });

  it("mousedown outside the input + listbox closes the list", () => {
    render(
      <div>
        <button type="button" data-testid="outside">
          outside
        </button>
        <ModelTextInput value="" onChange={() => {}} suggestions={["a"]} id="m" />
      </div>,
    );
    fireEvent.focus(screen.getByRole("combobox"));
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  // -----------------------------------------------------------------
  // ARIA — combobox wiring per W3C pattern.
  // -----------------------------------------------------------------

  it("aria-expanded reflects open state", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    expect(input.getAttribute("aria-expanded")).toBe("false");
    fireEvent.focus(input);
    expect(input.getAttribute("aria-expanded")).toBe("true");
  });

  it("aria-activedescendant points at the highlighted option", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a", "b"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    expect(input.getAttribute("aria-activedescendant")).toBeNull();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    const id = input.getAttribute("aria-activedescendant");
    expect(id).toBeTruthy();
    expect(id).toBe(screen.getAllByRole("option")[0].getAttribute("id"));
  });

  it("aria-controls links the combobox to the listbox by id", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["a"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    const listbox = screen.getByRole("listbox");
    expect(input.getAttribute("aria-controls")).toBe(listbox.getAttribute("id"));
  });

  // -----------------------------------------------------------------
  // External value sync + disabled
  // -----------------------------------------------------------------

  it("syncs the displayed value when the prop changes externally", () => {
    const { rerender } = render(
      <ModelTextInput value="gpt-4" onChange={() => {}} suggestions={[]} id="m" />,
    );
    expect((screen.getByRole("combobox") as HTMLInputElement).value).toBe("gpt-4");
    rerender(
      <ModelTextInput value="claude-3" onChange={() => {}} suggestions={[]} id="m" />,
    );
    expect((screen.getByRole("combobox") as HTMLInputElement).value).toBe("claude-3");
  });

  it("respects the disabled prop", () => {
    render(
      <ModelTextInput
        value="gpt-4"
        onChange={() => {}}
        suggestions={["gpt-4"]}
        id="m"
        disabled
      />,
    );
    expect((screen.getByRole("combobox") as HTMLInputElement).disabled).toBe(true);
  });

  it("uses the placeholder when value is empty", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={[]}
        id="m"
        placeholder="pick one"
      />,
    );
    expect(
      (screen.getByRole("combobox") as HTMLInputElement).placeholder,
    ).toBe("pick one");
  });
});
