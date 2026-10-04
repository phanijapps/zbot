import { useState } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { ModalOverlay } from "./modal-overlay";

function EmbeddingDialog() {
  const [open, setOpen] = useState(false);
  return <><button onClick={() => setOpen(true)}>Configure embeddings</button><ModalOverlay open={open} onClose={() => setOpen(false)} title="Embedding configuration"><button>Done</button></ModalOverlay></>;
}

// STUB: AC11 — existing embedding modal must not let focus escape behind it.
it("contains focus and returns to the invoking embedding control", async () => {
  const user = userEvent.setup();
  render(<EmbeddingDialog />);
  const trigger = screen.getByRole("button", {name:"Configure embeddings"});
  await user.click(trigger);
  const dialog = screen.getByRole("dialog", {name:"Embedding configuration"});
  const close = within(dialog).getByRole("button", {name:"Close"});
  within(dialog).getByRole("button", {name:"Done"}).focus();
  await user.tab();
  expect(close).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});
