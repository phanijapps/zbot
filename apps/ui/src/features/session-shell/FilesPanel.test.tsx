import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Artifact } from "@/services/transport/types";

const mocks = vi.hoisted(() => ({
  transport: { getSessionFull: vi.fn(), getSessionDetails: vi.fn(), listSessionsFull: vi.fn(), createChatSession: vi.fn(), listSessionArtifacts: vi.fn() },
  chat: vi.fn(), research: vi.fn(), slideOut: vi.fn(),
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
vi.mock("../chat/ArtifactSlideOut", () => ({ ArtifactSlideOut: ({artifact, onClose}: {artifact: Artifact; onClose(): void}) => (
  <div role="dialog" aria-label={`Preview ${artifact.fileName}`}><button type="button" onClick={onClose}>Close preview</button></div>
) }));

import { SessionShell } from "./SessionShell";
import { FilesPanel } from "./FilesPanel";

const artifact = (id: string, sessionId: string, fileName = `${id}.md`): Artifact => ({
  id, sessionId, fileName, fileType: "md", fileSize: 128, createdAt: "2026-10-04T00:00:00Z",
});

beforeEach(() => {
  vi.clearAllMocks();
  Element.prototype.scrollIntoView = vi.fn();
  mocks.transport.listSessionsFull.mockResolvedValue({ success: true, data: [] });
  mocks.transport.getSessionFull.mockResolvedValue({ success: true, data: { id: "sess-1", mode: "fast", title: "Chat", executions: [] } });
  mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => ({success:true,data:{sessionId, mode:"chat", activity:[], activityTruncated:false, sources:[], sourcesTruncated:false}}));
  mocks.transport.listSessionArtifacts.mockImplementation(async (sessionId: string) => ({ success: true, data: [artifact("art-1", sessionId, "summary.md")] }));
  mocks.chat.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", messages:[], artifacts:[]}, isActive:false, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
  mocks.research.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", turns:[], artifacts:[], error:null}, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
});

// Delivered STUB: AC2 — Files tab renders the server artifact manifest for the selected session
describe("FilesPanel (shell wiring)", () => {
  it("lists the session's artifacts under the Files tab", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    const tab = await screen.findByRole("tab", { name: "Files" });
    fireEvent.click(tab);
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("summary.md")).toBeVisible();
    expect(mocks.transport.listSessionArtifacts).toHaveBeenCalledWith("sess-1");
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
  });

  // Delivered STUB: AC2 — opening an artifact resolves content by artifact ID under the selected session
  it("opens artifact content by artifact ID", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    fireEvent.click(await screen.findByRole("tab", { name: "Files" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open artifact summary.md" }));
    expect(await screen.findByRole("dialog", { name: "Preview summary.md" })).toBeVisible();
  });
});

describe("FilesPanel", () => {
  it("waits for a confirmed session identity instead of guessing", () => {
    render(<FilesPanel active={false} />);
    expect(screen.getByText(/Select or start a conversation/)).toBeVisible();
    expect(mocks.transport.listSessionArtifacts).not.toHaveBeenCalled();
  });

  it("shows an honest empty state when the manifest has no artifacts", async () => {
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [] });
    render(<FilesPanel sessionId="sess-one" active={false} />);
    expect(await screen.findByText(/No files recorded/)).toBeVisible();
  });

  it("filters rows from another session out of the manifest", async () => {
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-1", "sess-one"), artifact("art-other", "sess-two", "foreign.md")] });
    render(<FilesPanel sessionId="sess-one" active={false} />);
    expect(await screen.findByText("art-1.md")).toBeVisible();
    expect(screen.queryByText("foreign.md")).not.toBeInTheDocument();
  });

  it("keeps loaded files on failure and retries without duplication", async () => {
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-1", "sess-one")] });
    const view = render(<FilesPanel sessionId="sess-one" active />);
    await screen.findByText("art-1.md");
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: false });
    view.rerender(<FilesPanel sessionId="sess-one" active={false} />);
    await screen.findByText(/Files are unavailable/);
    expect(screen.getByText("art-1.md")).toBeVisible();
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-1", "sess-one")] });
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findAllByText("art-1.md")).toHaveLength(1);
    expect(screen.queryByText(/Files are unavailable/)).not.toBeInTheDocument();
  });

  it("rereads the manifest when an active run settles", async () => {
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [] });
    const view = render(<FilesPanel sessionId="sess-one" active />);
    await screen.findByText(/No files recorded/);
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-final", "sess-one", "final.md")] });
    view.rerender(<FilesPanel sessionId="sess-one" active={false} />);
    expect(await screen.findByText("final.md")).toBeVisible();
  });

  it("discards a late response and stale rows for a previously selected session", async () => {
    let finish!: (value: unknown) => void;
    mocks.transport.listSessionArtifacts.mockImplementationOnce(() => new Promise(resolve => {finish = resolve;}));
    const view = render(<FilesPanel sessionId="sess-old" active={false} />);
    await waitFor(() => expect(mocks.transport.listSessionArtifacts).toHaveBeenCalledWith("sess-old"));
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-new", "sess-one", "current.md")] });
    view.rerender(<FilesPanel sessionId="sess-one" active={false} />);
    expect(await screen.findByText("current.md")).toBeVisible();
    finish({ success: true, data: [artifact("art-old", "sess-old", "stale.md")] });
    await waitFor(() => expect(screen.queryByText("stale.md")).not.toBeInTheDocument());
  });

  it("closes an open preview when the selected conversation changes", async () => {
    const view = render(<FilesPanel sessionId="sess-one" active={false} />);
    fireEvent.click(await screen.findByRole("button", { name: "Open artifact summary.md" }));
    expect(await screen.findByRole("dialog", { name: "Preview summary.md" })).toBeVisible();
    view.rerender(<FilesPanel sessionId="sess-two" active={false} />);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("closes the artifact preview and can open another file", async () => {
    mocks.transport.listSessionArtifacts.mockResolvedValue({ success: true, data: [artifact("art-1", "sess-one", "one.md"), artifact("art-2", "sess-one", "two.md")] });
    render(<FilesPanel sessionId="sess-one" active={false} />);
    fireEvent.click(await screen.findByRole("button", { name: "Open artifact one.md" }));
    expect(await screen.findByRole("dialog", { name: "Preview one.md" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Close preview" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open artifact two.md" }));
    expect(await screen.findByRole("dialog", { name: "Preview two.md" })).toBeVisible();
  });
});
