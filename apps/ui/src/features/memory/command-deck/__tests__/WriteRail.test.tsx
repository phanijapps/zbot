import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WriteRail } from "../WriteRail";

describe("WriteRail", () => {
  it("opens AddDrawer with preset category when + Instruction clicked", () => {
    const onSave = vi.fn();
    render(
      <WriteRail
        wardId="wardA"
        onSave={onSave}
        counts={{ facts: 10, wiki: 2, procedures: 1, episodes: 3 }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /\+ instruction/i }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: /memory content/i }), {
      target: { value: "Always verify OPF metadata" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    expect(onSave).toHaveBeenCalledWith({
      category: "instruction",
      content: "Always verify OPF metadata",
      ward_id: "wardA",
    });
  });

  it("closes AddDrawer on cancel", () => {
    const onSave = vi.fn();
    render(
      <WriteRail
        wardId="wardA"
        onSave={onSave}
        counts={{ facts: 0, wiki: 0, procedures: 0, episodes: 0 }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /\+ fact/i }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
  });
});
