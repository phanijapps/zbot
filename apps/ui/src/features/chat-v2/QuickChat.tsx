import { useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Square } from "lucide-react";
import { ChatInput } from "../chat/ChatInput";
import { StatusPill } from "../shared/statusPill";
import { InlineActivityChip } from "./InlineActivityChip";
import { useQuickChat } from "./useQuickChat";
import type { QuickChatMessage } from "./types";
import "./quick-chat.css";

function AssistantBubble({ message }: { message: QuickChatMessage }) {
  return (
    <div className="quick-chat__assistant">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.content}</ReactMarkdown>
      {message.chips && message.chips.length > 0 && (
        <div className="quick-chat__chips">
          {message.chips.map((c) => <InlineActivityChip key={c.id} chip={c} />)}
        </div>
      )}
    </div>
  );
}

function MessageRow({ message }: { message: QuickChatMessage }) {
  return (
    <div className={`quick-chat__msg quick-chat__msg--${message.role}`}>
      {message.role === "user"
        ? <div className="quick-chat__user-bubble">{message.content}</div>
        : <AssistantBubble message={message} />}
    </div>
  );
}

function EmptyState({ wardName }: { wardName: string | null }) {
  return (
    <div className="quick-chat__empty">
      <h1>Quick chat</h1>
      <p className="quick-chat__empty-subtext">
        memory-aware · single-step delegation
        {wardName ? ` · bound to ${wardName}` : ""}
      </p>
    </div>
  );
}

export function QuickChat() {
  const { state, pillState, sendMessage, stopAgent } = useQuickChat();
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [state.messages.length]);

  const hasMessages = state.messages.length > 0;

  return (
    <div className="quick-chat">
      <div className="quick-chat__header">
        <div className="quick-chat__ward">
          {state.activeWardName && (
            <span className="quick-chat__ward-chip">{state.activeWardName}</span>
          )}
        </div>
        <div className="quick-chat__actions">
          {state.status === "running" && (
            <button type="button" className="btn btn--ghost btn--sm" onClick={stopAgent} title="Stop">
              <Square size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="quick-chat__pill-strip">
        <StatusPill state={pillState} />
      </div>

      {hasMessages ? (
        <div className="quick-chat__scroll">
          <div className="quick-chat__messages">
            {state.messages.map((m) => <MessageRow key={m.id} message={m} />)}
            <div ref={endRef} />
          </div>
        </div>
      ) : (
        <EmptyState wardName={state.activeWardName} />
      )}

      <div className="quick-chat__composer">
        <ChatInput
          onSend={sendMessage}
          disabled={state.status === "running" || !state.sessionId}
        />
      </div>
    </div>
  );
}
