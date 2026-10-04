import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DesktopAdministrationPage } from "./DesktopAdministrationPage";

vi.mock("./useRecentSessions", () => ({useRecentSessions: () => ({recents:[],unavailable:false})}));
beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({matches:false, addEventListener:vi.fn(), removeEventListener:vi.fn()})));
});
afterEach(() => vi.unstubAllGlobals());

it("focuses the route, contains mobile navigation and restores its trigger", async () => {
  const user = userEvent.setup();
  render(<MemoryRouter initialEntries={["/agents?returnTo=%2Fsession%2Fsess-kept"]}>
    <DesktopAdministrationPage><h1>Agents</h1><button>Existing page action</button></DesktopAdministrationPage>
  </MemoryRouter>);
  const main = screen.getByRole("main");
  expect(main).toHaveFocus();
  expect(screen.getAllByRole("heading", {level:1})).toHaveLength(1);
  const trigger = screen.getByRole("button", {name:"Open navigation"});
  await user.click(trigger);
  const dialog = screen.getByRole("dialog", {name:"Navigation"});
  expect(main).toHaveAttribute("inert");
  expect(within(dialog).getByRole("link", {name:"zbot"})).toHaveFocus();
  within(dialog).getByRole("button", {name:"Close navigation"}).focus();
  await user.tab();
  expect(within(dialog).getByRole("link", {name:"zbot"})).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(main).not.toHaveAttribute("inert");
  expect(trigger).toHaveFocus();
  expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session/sess-kept");
});
