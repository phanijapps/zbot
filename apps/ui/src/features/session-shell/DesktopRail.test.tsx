import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { DesktopRail } from "./DesktopRail";

// STUB: AC11 — shared destination inventory must carry refresh-safe return state.
describe("desktop administration destinations", () => {
  it.each([["Agents", "/agents"], ["Settings", "/settings"], ["Integrations", "/integrations"]])("links to %s while preserving the selected conversation", (label, path) => {
    render(<MemoryRouter><DesktopRail returnTo="/session/sess-kept" recents={[]} unavailable={false} onClose={vi.fn()} /></MemoryRouter>);
    const destination = new URL(screen.getByRole("link", {name:label}).getAttribute("href")!, "http://localhost");
    expect(destination.pathname).toBe(path);
    expect(destination.searchParams.get("returnTo")).toBe("/session/sess-kept");
  });
});
