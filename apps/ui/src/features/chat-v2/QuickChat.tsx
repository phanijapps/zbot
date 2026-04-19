import { useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Plus, Square } from "lucide-react";
import { ChatInput } from "../chat/ChatInput";
import { StatusPill } from "../shared/statusPill";
import { InlineActivityChip } from "./InlineActivityChip";
import { useQuickChat } from "./useQuickChat";
import "./quick-chat.css";

export function QuickChat() {
  const { state, pillState, sendMessage, startNewChat, stopAgent, loadOlder } = useQuickChat();
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [state.messages.length]);

  const isEmpty = state.messages.length === 0 && !state.sessionId;

  return (
    <div className="quick-chat">
      <div className="quick-chat__header">
        <div className="quick-chat__ward">
          {state.activeWardName
            ? <span className="quick-chat__ward-chip">{state.activeWardName}</span>
            : <span className="quick-chat__ward-chip quick-chat__ward-chip--muted">no ward</span>}
        </div>
        <div className="quick-chat__actions">
          {state.status === "running" && (
            <button type="button" className="btn btn--ghost btn--sm" onClick={stopAgent} title="Stop">
              <Square size={14} />
            </button>
          )}
          <button type="button" className="btn btn--ghost btn--sm" onClick={startNewChat} title="New chat">
            <Plus size={14} /> New chat
          </button>
        </div>
      </div>

      <div className="quick-chat__pill-strip">
        <StatusPill state={pillState} />
      </div>

      {isEmpty ? (
        <div className="quick-chat__empty">
          <h1>Quick chat</h1>
          <p className="quick-chat__empty-subtext">
            memory-aware · single-step delegation
            {state.activeWardName ? ` · bound to ${state.activeWardName}` : ""}
          </p>
        </div>
      ) : (
        <div className="quick-chat__scroll">
          {state.hasMoreOlder && (
            <button type="button" className="quick-chat__load-older" onClick={loadOlder}>
              ↑ Show earlier turns
            </button>
          )}
          <div className="quick-chat__messages">
            {state.messages.map((m) => (
              <div key={m.id} className={`quick-chat__msg quick-chat__msg--${m.role}`}>
                {m.role === "user" ? (
                  <div className="quick-chat__user-bubble">{m.content}</div>
                ) : (
                  <div className="quick-chat__assistant">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>
                      {m.content}
                    </ReactMarkdown>
                    {m.chips && m.chips.length > 0 && (
                      <div className="quick-chat__chips">
                        {m.chips.map((c) => <InlineActivityChip key={c.id} chip={c} />)}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
            <div ref={endRef} />
          </div>
        </div>
      )}

      <div className="quick-chat__composer">
        <ChatInput onSend={sendMessage} disabled={state.status === "running"} />
      </div>
    </div>
  );
}
