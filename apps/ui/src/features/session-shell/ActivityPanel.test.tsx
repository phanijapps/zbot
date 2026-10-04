import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ details: vi.fn() }));
vi.mock("@/services/transport", () => ({getTransport: async () => ({getSessionDetails: mocks.details})}));
import { ActivityPanel } from "./ActivityPanel";
const body = (sessionId = "sess-one", status = "completed", truncated = false) => ({
  sessionId, mode: "chat", activityTruncated: truncated, sources: [], sourcesTruncated: false,
  activity: [{id:"activity-one",sequence:0,kind:"hook",label:`Hook ${status}`,occurredAt:"2026-10-04T12:00:00Z",
    hook:{hookId:"observe",event:"run_end",eventId:"event-one",invocationId:"invocation-one",agentId:"child",runId:"run-one",status,durationMs:12,exitCode:0}}],
});
beforeEach(() => {vi.clearAllMocks(); mocks.details.mockResolvedValue({success:true,data:body()});});
afterEach(() => {vi.useRealTimers();});
describe("ActivityPanel", () => {
  it("waits for a confirmed ID and shows honest empty activity", async () => {
    const view = render(<ActivityPanel active={false} />);
    expect(screen.getByText(/Select or start a conversation/)).toBeVisible();
    expect(mocks.details).not.toHaveBeenCalled();
    mocks.details.mockResolvedValue({success:true,data:{...body(),activity:[]}});
    view.rerender(<ActivityPanel sessionId="sess-one" active={false} />);
    const announcements = screen.getByRole("status");
    expect(announcements).toHaveTextContent("Loading activity…");
    expect(announcements.closest('[aria-busy="true"]')).toBeNull();
    expect(screen.getByRole("list")).toHaveAttribute("aria-busy", "true");
    expect(await screen.findByText(/No recorded activity/)).toBeVisible();
    expect(screen.getByRole("status")).toBe(announcements);
    expect(announcements).toHaveTextContent("");
  });
  it("discloses metadata with fixed status and explicit server truncation", async () => {
    mocks.details.mockResolvedValue({success:true,data:body("sess-one","blocked",true)});
    render(<ActivityPanel sessionId="sess-one" active={false} />);
    expect(await screen.findByText("Hook blocked")).toBeVisible();
    expect(screen.getByText(/Most recent 500/)).toBeVisible();
    fireEvent.click(screen.getByText("observe"));
    expect(screen.getByText("child")).toBeVisible();
    expect(screen.getByText("run-one")).toBeVisible();
    expect(screen.getByText("12 ms")).toBeVisible();
  });
  it("keeps same-session rows on error, rejects wrong-session data, and retries", async () => {
    const view = render(<ActivityPanel sessionId="sess-one" active />);
    await screen.findByText("Hook completed");
    const announcements = screen.getByRole("status");
    expect(announcements).toHaveTextContent("");
    mocks.details.mockResolvedValue({success:true,data:body("sess-other","failed")});
    view.rerender(<ActivityPanel sessionId="sess-one" active={false} />);
    await screen.findByText(/Activity is unavailable/);
    expect(screen.getByRole("status")).toBe(announcements);
    expect(announcements).toHaveTextContent(/Activity is unavailable/);
    expect(screen.getByText("Hook completed")).toBeVisible();
    expect(screen.queryByText("Hook failed")).not.toBeInTheDocument();
    mocks.details.mockResolvedValue({success:true,data:body("sess-one","cancelled")});
    fireEvent.click(screen.getByRole("button",{name:"Retry"}));
    expect(await screen.findByText("Hook cancelled")).toBeVisible();
    expect(screen.getByRole("status")).toBe(announcements);
    expect(announcements).toHaveTextContent("");
  });
  it("ignores a late response for a previously selected session", async () => {
    let finish!: (value: unknown) => void;
    mocks.details.mockImplementationOnce(() => new Promise(resolve => {finish=resolve;}));
    const view = render(<ActivityPanel sessionId="sess-old" active={false} />);
    await waitFor(() => expect(mocks.details).toHaveBeenCalledWith("sess-old"));
    view.rerender(<ActivityPanel sessionId="sess-one" active={false} />);
    await screen.findByText("Hook completed");
    await act(async () => {finish({success:true,data:body("sess-old","failed")});});
    expect(screen.queryByText("Hook failed")).not.toBeInTheDocument();
  });
  it("polls after Stop to collect bounded cleanup outcomes", async () => {
    vi.useFakeTimers();
    mocks.details.mockResolvedValue({success:true,data:body("sess-one","running")});
    const view = render(<ActivityPanel sessionId="sess-one" active />);
    await act(async () => { await Promise.resolve(); });
    mocks.details.mockResolvedValue({success:true,data:body("sess-one","cancelled")});
    view.rerender(<ActivityPanel sessionId="sess-one" active={false} />);
    await act(async () => {await vi.advanceTimersByTimeAsync(6000);});
    expect(screen.getByText("Hook cancelled")).toBeVisible();
    const count = mocks.details.mock.calls.length;
    await act(async () => {await vi.advanceTimersByTimeAsync(6000);});
    expect(mocks.details).toHaveBeenCalledTimes(count);
  });
});
