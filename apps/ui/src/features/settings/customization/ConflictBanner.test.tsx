import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ConflictBanner } from "./ConflictBanner";

describe("ConflictBanner", () => {
  it("renders the conflict message", () => {
    render(<ConflictBanner onAcceptDisk={() => {}} onKeepEditing={() => {}} />);
    expect(screen.getByText(/changed on disk while you were editing/i)).toBeInTheDocument();
  });

  it("invokes onAcceptDisk when 'View disk version' clicked", () => {
    const onAcceptDisk = vi.fn();
    render(<ConflictBanner onAcceptDisk={onAcceptDisk} onKeepEditing={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /View disk version/i }));
    expect(onAcceptDisk).toHaveBeenCalled();
  });

  it("invokes onKeepEditing when 'Keep editing' clicked", () => {
    const onKeepEditing = vi.fn();
    render(<ConflictBanner onAcceptDisk={() => {}} onKeepEditing={onKeepEditing} />);
    fireEvent.click(screen.getByRole("button", { name: /Keep editing/i }));
    expect(onKeepEditing).toHaveBeenCalled();
  });
});
