import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { ArrowLeft, Menu } from "lucide-react";
import { MemoryTab } from "../memory";
import { ObservatoryPage } from "../observatory";
import { DesktopRail } from "./DesktopRail";
import { conversationDestination, knowledgeDestination } from "./navigation";
import { useRecentSessions } from "./useRecentSessions";

export function DesktopKnowledgePage({kind}: {kind: "memory" | "observatory"}) {
  const [search] = useSearchParams();
  const returnTo = conversationDestination(search.get("returnTo"));
  const {recents, unavailable} = useRecentSessions(kind);
  const [navigationOpen, setNavigationOpen] = useState(false);
  const titleRef = useRef<HTMLHeadingElement>(null);
  useEffect(() => { titleRef.current?.focus(); }, [kind]);
  const title = kind === "memory" ? "Memory" : "Observatory";
  return <div className={`session-shell session-shell--knowledge${navigationOpen ? " session-shell--navigation-open" : ""}`}>
    <DesktopRail returnTo={returnTo} recents={recents} unavailable={unavailable} onClose={() => setNavigationOpen(false)} />
    <main className="session-shell__main">
      <header className="session-shell__knowledge-header">
        <button className="session-shell__navigation-toggle btn btn--icon-ghost" aria-label="Open navigation" aria-expanded={navigationOpen} onClick={() => setNavigationOpen(!navigationOpen)}><Menu size={18} aria-hidden="true" /></button>
        <div><h1 ref={titleRef} tabIndex={-1}>{title}</h1><p>{kind === "memory" ? "Find, inspect, and curate your durable knowledge." : "Explore how your knowledge connects."}</p></div>
        <Link className="session-shell__return btn btn--ghost btn--sm" to={returnTo}><ArrowLeft size={16} aria-hidden="true" />Back to conversation</Link>
      </header>
      <div className={`session-shell__knowledge-content session-shell__knowledge-content--${kind}`}>
        {kind === "memory" ? <MemoryTab agentId="root" embedded observatoryHref={knowledgeDestination("/observatory", returnTo)} /> : <ObservatoryPage />}
      </div>
    </main>
  </div>;
}
