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

beforeEach(() => {
  vi.clearAllMocks();
  Element.prototype.scrollIntoView = vi.fn();
  mocks.transport.listSessionsFull.mockResolvedValue({ success: true, data: [] });
  mocks.transport.getSessionFull.mockResolvedValue({ success: true, data: { id: "sess-1", mode: "fast", title: "Chat", executions: [] } });
  mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => ({success:true,data:{
    sessionId, mode: "chat",
    activity: [], activityTruncated: false,
    sources: [{ id: "src-1", title: "Measured contrast study", url: "https://example.com/study", evidence: "answer_citation" }],
    sourcesTruncated: false,
  }}));
  mocks.chat.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", messages:[], artifacts:[]}, isActive:false, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
  mocks.research.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", turns:[], artifacts:[], error:null}, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
});

// STUB: AC2 — Sources tab renders server-provided sources from the details contract
describe("SourcesPanel (stub)", () => {
  it("shows a server-provided source under the Sources tab", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    fireEvent.click(await screen.findByRole("tab", { name: "Sources" }));
    expect(await screen.findByText("Measured contrast study")).toBeVisible();
    expect(mocks.transport.getSessionDetails).toHaveBeenCalledWith("sess-1");
  });
});
