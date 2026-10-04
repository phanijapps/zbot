import { useState } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { Slideover } from "./Slideover";

function Editor() {
  const [open, setOpen] = useState(false);
  return <><button onClick={() => setOpen(true)}>Edit agent</button><Slideover open={open} onClose={() => setOpen(false)} title="Agent editor"><input aria-label="Name" /><button>Save</button></Slideover></>;
}

// STUB: AC11 — label, containment, Escape and restoration for existing editors.
it("labels the editor, contains keyboard focus and restores its trigger", async () => {
  const user = userEvent.setup();
  render(<Editor />);
  const trigger = screen.getByRole("button", {name:"Edit agent"});
  await user.click(trigger);
  const dialog = screen.getByRole("dialog", {name:"Agent editor"});
  const close = within(dialog).getByRole("button", {name:"Close"});
  expect(close).toHaveFocus();
  const save = within(dialog).getByRole("button", {name:"Save"});
  save.focus();
  await user.tab();
  expect(close).toHaveFocus();
  await user.tab({shift:true});
  expect(save).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});
