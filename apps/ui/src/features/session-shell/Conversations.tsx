import { useEffect, useRef } from "react";
import { ChatInput, type UploadedFile } from "../chat/ChatInput";
import { MessageRow } from "../chat-v2/QuickChat";
import { useQuickChat } from "../chat-v2/useQuickChat";
import { MainColumn } from "../research-v2/ResearchPage";
import { useResearchSession } from "../research-v2/useResearchSession";
import { StatusPill } from "../shared/statusPill";
import { A2uiSurfaceRenderer } from "../surfaces/A2uiSurfaceRenderer";

export interface PendingMessage { content: string; attachments: UploadedFile[] }
interface ConversationProps {
  onActive(active: boolean | undefined): void;
}

export function ChatConversation({ sessionId, pending, onActive, onSent }: ConversationProps & {
  sessionId: string; pending: PendingMessage | null; onSent(): void;
}) {
  const chat = useQuickChat({ sessionId });
  const sendMessage = chat.sendMessage;
  const boundSessionId = chat.state.sessionId;
  const sentRef = useRef(false);
  useEffect(() => {
    if (chat.state.sessionId === sessionId) onActive(chat.isActive);
  }, [chat.state.sessionId, chat.isActive, onActive, sessionId]);
  useEffect(() => {
    if (!pending || boundSessionId !== sessionId || sentRef.current) return;
    sentRef.current = true;
    void sendMessage(pending.content, pending.attachments);
    onSent();
  }, [pending, boundSessionId, sendMessage, sessionId, onSent]);
  return <>
    <div className="session-shell__status"><StatusPill state={chat.pillState} />
      {chat.isActive && <button className="btn btn--ghost btn--sm" onClick={() => void chat.stopAgent()}>Stop chat</button>}
      {chat.state.status === "error" && <p role="alert">Chat request unavailable. Your conversation has been kept.</p>}
    </div>
    <div className="session-shell__scroll"><div className="session-shell__tape">
      {chat.state.messages.length === 0 && <div className="session-shell__empty"><h1>How can I help?</h1><p>A conversation with your memory close at hand.</p></div>}
      {chat.state.messages.map(message => <MessageRow key={message.id} message={message} />)}
      {chat.surfaces.map(item => <A2uiSurfaceRenderer key={item.surface.surface_id} surface={item.surface} />)}
    </div></div>
    <div className="session-shell__composer"><ChatInput onSend={chat.sendMessage} disabled={chat.isActive || !chat.state.sessionId} /></div>
  </>;
}

export function ResearchConversation({ sessionId, onActive }: ConversationProps & {sessionId?: string}) {
  const research = useResearchSession({ sessionId, baseRoute: "/session" });
  useEffect(() => {
    if (!sessionId || research.state.sessionId === sessionId) {
      onActive(research.state.status === "error" ? undefined : research.state.status === "running");
    }
  }, [research.state.sessionId, research.state.status, onActive, sessionId]);
  return <>
    <div className="session-shell__status"><StatusPill state={research.pillState} />
      {research.state.status === "running" && <button className="btn btn--ghost btn--sm" disabled={!research.state.sessionId} onClick={() => void research.stopAgent()}>Stop research</button>}
      {research.state.status === "error" && <p role="alert">Research request unavailable. Your conversation has been kept.</p>}
    </div>
    <div className="session-shell__scroll"><div className="session-shell__tape">
      {!research.state.sessionId && research.state.turns.length === 0
        ? <div className="session-shell__empty"><h1>What would you like to research?</h1><p>Plan, investigate, and bring the evidence together.</p></div>
        : <MainColumn state={research.state} surfaces={research.surfaces} onSend={research.sendMessage} showSubagents />}
    </div></div>
    <div className="session-shell__composer"><ChatInput onSend={research.sendMessage} disabled={research.state.status === "running"} /></div>
  </>;
}
