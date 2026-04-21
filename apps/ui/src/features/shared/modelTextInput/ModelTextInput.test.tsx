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
});
