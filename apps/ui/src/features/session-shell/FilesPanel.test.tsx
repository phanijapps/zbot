import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const mocks = vi.hoisted(() => ({
  transport: { getSessionFull: vi.fn(), getSessionDetails: vi.fn(), listSessionsFull: vi.fn(), createChatSession: vi.fn(), listSessionArtifacts: vi.fn(), getArtifactContentUrl: vi.fn() },
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
  mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => ({success:true,data:{sessionId, mode:"chat", activity:[], activityTruncated:false, sources:[], sourcesTruncated:false}}));
  mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [
    { id: "art-1", sessionId: "sess-1", fileName: "summary.md", fileType: "md", fileSize: 128, createdAt: "2026-10-04T00:00:00Z" },
  ] });
  mocks.transport.getArtifactContentUrl.mockReturnValue("/api/artifacts/art-1/content?session=sess-1");
  mocks.chat.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", messages:[], artifacts:[]}, isActive:false, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
  mocks.research.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", turns:[], artifacts:[], error:null}, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
});

// STUB: AC2 — Files tab renders the server artifact manifest for the selected session
describe("FilesPanel (stub)", () => {
  it("lists the session's artifacts under the Files tab", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    fireEvent.click(await screen.findByRole("tab", { name: "Files" }));
    expect(await screen.findByText("summary.md")).toBeVisible();
    expect(mocks.transport.listSessionArtifacts).toHaveBeenCalledWith("sess-1");
  });
});
