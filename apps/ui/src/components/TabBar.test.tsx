import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { TabBar, TabPanel } from "./TabBar";

function Tabs() {
  const [active, setActive] = useState("agents");
  return <><TabBar tabs={[{id:"agents",label:"My Agents"},{id:"skills",label:"Skills Library"}]} activeTab={active} onTabChange={setActive} /><TabPanel id="agents" activeTab={active}>Agents content</TabPanel><TabPanel id="skills" activeTab={active}>Skills content</TabPanel></>;
}

// STUB: AC11 — keyboard contract of the reused tab component.
it("uses roving focus and arrows/Home/End to select existing page tabs", async () => {
  const user = userEvent.setup();
  render(<Tabs />);
  const agents = screen.getByRole("tab", {name:"My Agents"});
  const skills = screen.getByRole("tab", {name:"Skills Library"});
  expect(agents).toHaveAttribute("tabindex", "0");
  expect(skills).toHaveAttribute("tabindex", "-1");
  agents.focus();
  await user.keyboard("{ArrowRight}");
  expect(skills).toHaveFocus();
  expect(skills).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tabpanel")).toHaveTextContent("Skills content");
  await user.keyboard("{Home}");
  expect(agents).toHaveFocus();
  await user.keyboard("{End}");
  expect(skills).toHaveFocus();
});
