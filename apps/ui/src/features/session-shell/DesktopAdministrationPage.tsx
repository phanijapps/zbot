import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link, useLocation, useSearchParams } from "react-router-dom";
import { ArrowLeft, Menu } from "lucide-react";
import { DesktopRail } from "./DesktopRail";
import { conversationDestination } from "./navigation";
import { useRecentSessions } from "./useRecentSessions";
import { useDialogFocus } from "@/hooks/useDialogFocus";

export function DesktopAdministrationPage({children}: {children: ReactNode}) {
  const {pathname} = useLocation();
  const [search] = useSearchParams();
  const returnTo = conversationDestination(search.get("returnTo"));
  const {recents, unavailable} = useRecentSessions(pathname);
  const [navigationOpen, setNavigationOpen] = useState(false);
  const mainRef = useRef<HTMLElement>(null);
  const navigationRef = useRef<HTMLDivElement>(null);
  useEffect(() => { mainRef.current?.focus(); }, [pathname]);
  const closeNavigation = () => setNavigationOpen(false);
  useDialogFocus(navigationOpen, navigationRef, closeNavigation);
  useEffect(() => {
    const wide = window.matchMedia("(min-width: 760px)");
    const closeWhenWide = () => { if (wide.matches) setNavigationOpen(false); };
    wide.addEventListener("change", closeWhenWide);
    return () => wide.removeEventListener("change", closeWhenWide);
  }, []);
  return <div className={`session-shell session-shell--administration${navigationOpen ? " session-shell--navigation-open" : ""}`}>
    <div ref={navigationRef} tabIndex={-1} className="session-shell__administration-navigation" role={navigationOpen ? "dialog" : undefined} aria-modal={navigationOpen ? true : undefined} aria-label={navigationOpen ? "Navigation" : undefined}>
      <DesktopRail returnTo={returnTo} recents={recents} unavailable={unavailable} onClose={closeNavigation} />
    </div>
    <main ref={mainRef} tabIndex={-1} className="session-shell__main" inert={navigationOpen}>
      <div className="session-shell__administration-toolbar">
        <button className="session-shell__navigation-toggle btn btn--icon-ghost" aria-label="Open navigation" aria-expanded={navigationOpen} onClick={() => setNavigationOpen(true)}><Menu size={18} aria-hidden="true" /></button>
        <Link className="session-shell__return btn btn--ghost btn--sm" to={returnTo}><ArrowLeft size={16} aria-hidden="true" />Back to conversation</Link>
      </div>
      <div className="session-shell__administration-content">{children}</div>
    </main>
  </div>;
}
