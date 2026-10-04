import { Link, useLocation } from "react-router-dom";
import { Brain, Bot, Network, Plus, Settings, Plug } from "lucide-react";
import type { SessionWithExecutions } from "@/services/transport/types";
import { knowledgeDestination } from "./navigation";
import { sessionMode } from "./mode";

interface Props {
  returnTo: string;
  recents: SessionWithExecutions[];
  unavailable: boolean;
  sessionId?: string;
  locked?: boolean;
  onNewChat?(): void;
  onSelect?(id: string): void;
  onClose(): void;
}

export function DesktopRail({returnTo, recents, unavailable, sessionId, locked, onNewChat, onSelect, onClose}: Props) {
  const {pathname} = useLocation();
  return <aside className="session-shell__rail" aria-label="Conversation navigation" onKeyDown={event => { if (event.key === "Escape") onClose(); }}>
    <Link className="session-shell__brand" to={returnTo} onClick={onClose}>zbot</Link>
    {onNewChat
      ? <button type="button" className="session-shell__nav-item" disabled={locked} onClick={onNewChat}><Plus size={18} aria-hidden="true" />New chat</button>
      : <Link className="session-shell__nav-item" to="/session" onClick={onClose}><Plus size={18} aria-hidden="true" />New chat</Link>}
    <h2 className="session-shell__section-title">Recent</h2>
    <nav className="session-shell__recents" aria-label="Recent conversations">
      {unavailable ? <p className="session-shell__hint">Recent conversations unavailable.</p> : recents.length === 0 && <p className="session-shell__hint">No recent conversations.</p>}
      {recents.length === 30 && <p className="session-shell__hint">Latest 30 conversations</p>}
      {recents.map(item => {
        const label = item.title || (sessionMode(item.mode) === "research" ? "Untitled research" : sessionMode(item.mode) === "chat" ? "Untitled chat" : "Unknown mode");
        return onSelect
          ? <button key={item.id} type="button" className="session-shell__nav-item" disabled={locked} aria-current={item.id === sessionId ? "page" : undefined} onClick={() => onSelect(item.id)}><span>{label}</span></button>
          : <Link key={item.id} className="session-shell__nav-item" to={`/session/${encodeURIComponent(item.id)}`} onClick={onClose}><span>{label}</span></Link>;
      })}
    </nav>
    <nav className="session-shell__destinations" aria-label="Knowledge and settings">
      <Link className="session-shell__nav-item" aria-current={pathname === "/memory" ? "page" : undefined} to={knowledgeDestination("/memory", returnTo)} onClick={onClose}><Brain size={18} aria-hidden="true" />Memory</Link>
      <Link className="session-shell__nav-item" aria-current={pathname === "/observatory" ? "page" : undefined} to={knowledgeDestination("/observatory", returnTo)} onClick={onClose}><Network size={18} aria-hidden="true" />Observatory</Link>
      <Link className="session-shell__nav-item" aria-current={pathname === "/agents" ? "page" : undefined} to={knowledgeDestination("/agents", returnTo)} onClick={onClose}><Bot size={18} aria-hidden="true" />Agents</Link>
      <Link className="session-shell__nav-item" aria-current={pathname === "/integrations" ? "page" : undefined} to={knowledgeDestination("/integrations", returnTo)} onClick={onClose}><Plug size={18} aria-hidden="true" />Integrations</Link>
      <Link className="session-shell__nav-item" aria-current={pathname === "/settings" ? "page" : undefined} to={knowledgeDestination("/settings", returnTo)} onClick={onClose}><Settings size={18} aria-hidden="true" />Settings</Link>
    </nav>
    <button type="button" className="session-shell__mobile-close btn btn--ghost" onClick={onClose}>Close navigation</button>
  </aside>;
}
