import { useEffect } from "react";
import { X } from "lucide-react";
import { SessionsList, type SessionsListProps } from "./SessionsList";

export interface SessionsDrawerProps extends Omit<SessionsListProps, "renderDensity"> {
  open: boolean;
  onClose(): void;
}

export function SessionsDrawer({ open, onClose, ...listProps }: SessionsDrawerProps) {
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <>
      <button
        type="button"
        aria-label="Close sessions drawer"
        className="sessions-drawer__backdrop"
        onClick={onClose}
      />
      <dialog
        open
        className="sessions-drawer"
        aria-label="Research sessions"
      >
        <div className="sessions-drawer__header">
          <span>Sessions</span>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={onClose}
            aria-label="Close"
            title="Close"
          >
            <X size={14} />
          </button>
        </div>
        <SessionsList {...listProps} renderDensity="expanded" />
      </dialog>
    </>
  );
}
