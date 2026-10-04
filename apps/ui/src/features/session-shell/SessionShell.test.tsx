import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const mocks = vi.hoisted(() => ({
  transport: { getSessionFull: vi.fn(), getSessionDetails: vi.fn(), listSessionsFull: vi.fn(), createChatSession: vi.fn() },
  chat: vi.fn(), research: vi.fn(),
  liveChats: 0, maxLiveChats: 0,
}));
vi.mock("@/services/transport", () => ({ getTransport: async () => mocks.transport }));
vi.mock("../chat-v2/useQuickChat", async () => {
  const {useEffect} = await import("react");
  return {useQuickChat: (...args: unknown[]) => {
    useEffect(() => {
      mocks.liveChats++;
      mocks.maxLiveChats = Math.max(mocks.maxLiveChats, mocks.liveChats);
      return () => { mocks.liveChats--; };
    }, []);
    return mocks.chat(...args);
  }};
});
vi.mock("../research-v2/useResearchSession", () => ({ useResearchSession: mocks.research }));
vi.mock("../shared/statusPill", () => ({ StatusPill: () => null }));
vi.mock("../research-v2/ResearchPage", () => ({ MainColumn: () => <p>Restored Research answer</p> }));
vi.mock("../shared/markdown", () => ({ Markdown: ({children}: {children: string}) => <p>{children}</p> }));
vi.mock("../chat/ChatInput", () => ({ ChatInput: ({disabled}: {disabled: boolean}) => <input aria-label="Message" disabled={disabled} /> }));

import { SessionShell } from "./SessionShell";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.liveChats = 0; mocks.maxLiveChats = 0;
  Element.prototype.scrollIntoView = vi.fn();
  mocks.transport.listSessionsFull.mockResolvedValue({ success: true, data: [] });
  mocks.transport.getSessionDetails.mockImplementation(async (sessionId: string) => ({success:true,data:{sessionId,mode:"chat",activity:[],activityTruncated:false,sources:[],sourcesTruncated:false}}));
  mocks.transport.getSessionFull.mockResolvedValue({ success: true, data: { id: "sess-1", mode: "fast", title: "Earlier chat", executions: [] } });
  mocks.transport.createChatSession.mockResolvedValue({ success: true, data: { sessionId: "sess-new", conversationId: "sess-new", created: true, isLive: false } });
  mocks.chat.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", messages:[{id:"answer", content:"Restored Chat answer"}], artifacts:[]}, isActive:false, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
  mocks.research.mockReturnValue({ state: {sessionId:"sess-1", status:"idle", turns:[], artifacts:[], error:null}, surfaces:[], pillState:{}, sendMessage:vi.fn(), stopAgent:vi.fn() });
});

describe("SessionShell", () => {
  it("shows the quiet entry and knowledge destinations without a ward explorer", () => {
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    expect(screen.getByRole("button", {name:"New chat"})).toBeVisible();
    expect(screen.getByRole("tab", {name:"Chat"})).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("link", {name:"Memory"})).toHaveAttribute("href", "/memory");
    expect(screen.getByRole("link", {name:"Observatory"})).toHaveAttribute("href", "/observatory");
    expect(screen.queryByRole("link", {name:/ward/i})).not.toBeInTheDocument();
  });

  it("restores the server's Chat mode and selected transcript", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    expect(await screen.findByText("Restored Chat answer")).toBeVisible();
    expect(mocks.chat).toHaveBeenCalledWith({sessionId:"sess-1"});
    expect(screen.getByRole("tab", {name:"Chat"})).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(mocks.liveChats).toBe(1));
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("uses the default QuickChat confirmed ID for Activity without bootstrap or remount", async () => {
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    await waitFor(() => expect(mocks.transport.getSessionDetails).toHaveBeenCalledWith("sess-1"));
    expect(mocks.transport.getSessionFull).not.toHaveBeenCalled();
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("restores Research and disables mode switching while active", async () => {
    mocks.transport.getSessionFull.mockResolvedValue({success:true, data:{id:"sess-1",mode:"deep",executions:[{status:"running"}]}});
    mocks.research.mockReturnValue({state:{sessionId:"sess-1",status:"running",turns:[],artifacts:[],error:null},surfaces:[],pillState:{},sendMessage:vi.fn(),stopAgent:vi.fn()});
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    expect(await screen.findByText("Restored Research answer")).toBeVisible();
    expect(screen.getByRole("tab", {name:"Research"})).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", {name:"Chat"})).toBeDisabled();
    expect(screen.getByRole("button", {name:"New chat"})).toBeDisabled();
  });

  it("resumes reserved Quick Chat when an idle Research switches mode", async () => {
    mocks.transport.getSessionFull.mockImplementation(async (id: string) => ({success:true,data:{id,mode:id === "sess-new" ? "fast" : "deep",executions:[]}}));
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    await screen.findByText("Restored Research answer");
    fireEvent.click(screen.getByRole("tab", {name:"Chat"}));
    await screen.findByText("Restored Chat answer");
    expect(mocks.chat).toHaveBeenCalledWith();
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("defaults to the existing Quick Chat history without creating an independent session", async () => {
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    expect(await screen.findByText("Restored Chat answer")).toBeVisible();
    expect(mocks.chat).toHaveBeenCalledWith();
    expect(screen.getByRole("button", {name:"Clear chat"})).toBeVisible();
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("returns from a selected Chat to reserved Quick Chat through the Chat tab", async () => {
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    await screen.findByText("Restored Chat answer");
    fireEvent.click(screen.getByRole("tab", {name:"Chat"}));
    expect(await screen.findByRole("button", {name:"Clear chat"})).toBeVisible();
    expect(mocks.chat).toHaveBeenLastCalledWith();
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("keeps explicit New chat independent from default Quick Chat", async () => {
    mocks.transport.getSessionFull.mockImplementation(async (id: string) => ({success:true,data:{id,mode:"fast",executions:[]}}));
    mocks.chat.mockImplementation((options?: {sessionId:string}) => ({state:{sessionId:options?.sessionId ?? "sess-reserved",status:"idle",messages:[{id:"answer",content:options ? "Independent answer" : "Reserved history"}],artifacts:[]},isActive:false,surfaces:[],pillState:{},sendMessage:vi.fn(),stopAgent:vi.fn()}));
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    await screen.findByText("Reserved history");
    fireEvent.click(screen.getByRole("button", {name:"New chat"}));
    expect(await screen.findByText("Independent answer")).toBeVisible();
    expect(mocks.chat).toHaveBeenLastCalledWith({sessionId:"sess-new"});
    expect(mocks.transport.createChatSession).toHaveBeenCalledTimes(1);
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
    fireEvent.click(screen.getByRole("tab", {name:"Chat"}));
    expect(await screen.findByText("Reserved history")).toBeVisible();
    expect(mocks.transport.createChatSession).toHaveBeenCalledTimes(1);
    expect(mocks.liveChats).toBe(1);
    expect(mocks.maxLiveChats).toBe(1);
  });

  it("locks modes and New chat while reserved Quick Chat is running", async () => {
    mocks.chat.mockReturnValue({state:{sessionId:"sess-reserved",status:"running",messages:[],artifacts:[]},isActive:true,surfaces:[],pillState:{},sendMessage:vi.fn(),stopAgent:vi.fn()});
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    expect(await screen.findByRole("button", {name:"Stop chat"})).toBeVisible();
    expect(screen.getByRole("tab", {name:"Research"})).toBeDisabled();
    expect(screen.getByRole("button", {name:"New chat"})).toBeDisabled();
  });

  it("keeps default Quick Chat visible when independent creation fails", async () => {
    mocks.transport.createChatSession.mockResolvedValue({success:false});
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    await screen.findByText("Restored Chat answer");
    fireEvent.click(screen.getByRole("button", {name:"New chat"}));
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't start a new chat");
    expect(screen.getByText("Restored Chat answer")).toBeVisible();
    expect(mocks.liveChats).toBe(1);
  });

  it("keeps unknown stored mode explicit and mounts no execution adapter", async () => {
    mocks.transport.getSessionFull.mockResolvedValue({success:true,data:{id:"sess-1",executions:[]}});
    render(<MemoryRouter><SessionShell initialSessionId="sess-1" /></MemoryRouter>);
    expect(await screen.findByText(/mode is unknown/i)).toBeVisible();
    expect(mocks.chat).not.toHaveBeenCalled();
    expect(mocks.research).not.toHaveBeenCalled();
  });

  it("selects a server-backed recent session by its ID, never by guessed mode", async () => {
    mocks.transport.listSessionsFull.mockResolvedValue({success:true,data:[{id:"sess-recent",title:"Earlier investigation",mode:"deep"}]});
    mocks.transport.getSessionFull.mockResolvedValue({success:true,data:{id:"sess-recent",mode:"deep",executions:[]}});
    mocks.research.mockReturnValue({state:{sessionId:"sess-recent",status:"complete",turns:[],artifacts:[]},surfaces:[],pillState:{},sendMessage:vi.fn(),stopAgent:vi.fn()});
    render(<MemoryRouter><SessionShell /></MemoryRouter>);
    fireEvent.click(await screen.findByRole("button", {name:"Earlier investigation"}));
    expect(await screen.findByText("Restored Research answer")).toBeVisible();
    expect(mocks.transport.getSessionFull).toHaveBeenCalledWith("sess-recent");
    expect(mocks.research).toHaveBeenCalledWith({sessionId:"sess-recent",baseRoute:"/session"});
    expect(mocks.transport.createChatSession).not.toHaveBeenCalled();
  });
});
