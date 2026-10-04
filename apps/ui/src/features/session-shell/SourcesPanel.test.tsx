import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const mocks = vi.hoisted(() => ({
  transport: { getSessionFull: vi.fn(), getSessionDetails: vi.fn(), listSessionsFull: vi.fn(), createChatSession: vi.fn() },
  chat: vi.fn(), research: vi.fn(),
}));
vi.mock("@/services/transport", () => ({ getTransport: async () => mocks.transport }));
vi.mock("../chat-v2/useQuickChat", async () => {
  const {useEffect} = await import("react");
  return {useQuickChat: (...args: unknown[]) => {
    useEffect(() => {}, []);
    return mocks.chat(...args);
  }};
});
vi.mock("../research-v2/useResearchSession", () => ({ useResearchSession: mocks.research }));
vi.mock("../shared/statusPill", () => ({ StatusPill: () => null }));
vi.mock("../research-v2/ResearchPage", () => ({ MainColumn: () => null }));
vi.mock("../shared/markdown", () => ({ Markdown: ({children}: {children: string}) => <p>{children}</p> }));
vi.mock("../chat/ChatInput", () => ({ ChatInput: ({disabled}: {disabled: boolean}) => <input aria-label="Message" disabled={disabled} /> }));

import { SessionShell } from "./SessionShell";
import { SourcesPanel } from "./SourcesPanel";

type TestSource = { id: string; title: string; url: string; evidence: "answer_citation" | "structured_source_use" };
const detailsBody = (sessionId: string, sources: TestSource[], sourcesTruncated = false) => ({
  success: true, data: { sessionId, mode: "chat", activity: [], activityTruncated: false, sources, sourcesTruncated },
});

beforeEach(() => {
  vi.clearAllMocks();
  Element.prototype.scrollIntoView = vi.fn();
  mocks.transport.listSessionsFull.mockResolvedValue({ success: true, data: [] });
  mocks.transport.getSessionFull.mockResolvedValue({ success: true, data: { id: "sess-1", mode: "fast", title: "Chat", executions: [] } });
  mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => detailsBody(sessionId, [
    { id: "src-1", title: "Measured contrast study", url: "https://example.com/study", evidence: "answer_citation" },
    { id: "src-2", title: "Internal report", url: "file:///etc/passwd", evidence: "structured_source_use" },
  ]));
  mocks.chat.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", messages:[], artifacts:[]}, isActive:false, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
  mocks.research.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", turns:[], artifacts:[], error:null}, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
});

// Delivered STUB: AC2 — Sources tab renders server-provided sources from the details contract
describe("SourcesPanel (shell wiring)", () => {
  it("shows a server-provided source under the Sources tab", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    fireEvent.click(await screen.findByRole("tab", { name: "Sources" }));
    expect(await screen.findByText("Measured contrast study")).toBeVisible();
    expect(mocks.transport.getSessionDetails).toHaveBeenCalledWith("sess-1");
  });
});

describe("SourcesPanel", () => {
  it("renders only safe destinations as links and labels evidence", async () => {
    render(<SourcesPanel sessionId="sess-one" active={false} />);
    const link = await screen.findByRole("link", { name: "Measured contrast study" });
    expect(link).toHaveAttribute("href", "https://example.com/study");
    expect(link).toHaveAttribute("rel", "noreferrer noopener");
    expect(screen.getByText("Cited in answer")).toBeVisible();
    const unsafe = screen.getByText("Internal report");
    expect(unsafe.closest("a")).toBeNull();
    expect(screen.getByText("Reported source")).toBeVisible();
  });

  it("shows an honest empty state and a truncation notice", async () => {
    const view = render(<SourcesPanel sessionId="sess-one" active={false} />);
    mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => detailsBody(sessionId, []));
    expect(await screen.findByText(/No cited or used sources/)).toBeVisible();
    mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => detailsBody(sessionId, [], true));
    view.rerender(<SourcesPanel sessionId="sess-two" active={false} />);
    expect(await screen.findByText(/First 100 sources/)).toBeVisible();
  });

  it("keeps loaded sources on failure and retries without losing rows", async () => {
    const view = render(<SourcesPanel sessionId="sess-one" active />);
    await screen.findByText("Measured contrast study");
    mocks.transport.getSessionDetails.mockResolvedValue({ success: false });
    view.rerender(<SourcesPanel sessionId="sess-one" active={false} />);
    await screen.findByText(/Sources are unavailable/);
    expect(screen.getByText("Measured contrast study")).toBeVisible();
    mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => detailsBody(sessionId, [
      { id: "src-3", title: "After retry", url: "https://example.com/after", evidence: "answer_citation" },
    ]));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("After retry")).toBeVisible();
    expect(screen.queryByText(/Sources are unavailable/)).not.toBeInTheDocument();
  });

  it("discards a late response for a previously selected session", async () => {
    let finish!: (value: unknown) => void;
    mocks.transport.getSessionDetails.mockImplementationOnce(() => new Promise(resolve => {finish = resolve;}));
    const view = render(<SourcesPanel sessionId="sess-old" active={false} />);
    await waitFor(() => expect(mocks.transport.getSessionDetails).toHaveBeenCalledWith("sess-old"));
    mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => detailsBody(sessionId, [
      { id: "src-new", title: "Current session source", url: "https://example.com/now", evidence: "answer_citation" },
    ]));
    view.rerender(<SourcesPanel sessionId="sess-one" active={false} />);
    await screen.findByText("Current session source");
    finish(detailsBody("sess-old", [{ id: "src-old", title: "Stale session source", url: "https://example.com/old", evidence: "answer_citation" }]));
    await waitFor(() => expect(screen.queryByText("Stale session source")).not.toBeInTheDocument());
  });

  it("waits for a confirmed session identity instead of guessing", () => {
    render(<SourcesPanel active={false} />);
    expect(screen.getByText(/Select or start a conversation/)).toBeVisible();
    expect(mocks.transport.getSessionDetails).not.toHaveBeenCalled();
  });
});
