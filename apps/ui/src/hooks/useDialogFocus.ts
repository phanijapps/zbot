import { useEffect, useRef, type RefObject } from "react";

/** Shared keyboard lifecycle for the existing non-portal dialogs. */
export function useDialogFocus(open: boolean, ref: RefObject<HTMLElement | null>, onEscape?: () => void) {
  const escapeRef = useRef(onEscape);
  useEffect(() => { escapeRef.current = onEscape; }, [onEscape]);
  useEffect(() => {
    const panel = ref.current;
    if (!open || !panel) return;
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const controls = () => Array.from(panel.querySelectorAll<HTMLElement>(
      'button, a[href], input, select, textarea, [tabindex]'
    )).filter(el => el.tabIndex >= 0 && !el.matches(":disabled") && !el.closest('[hidden], [inert], [aria-hidden="true"]')
      && getComputedStyle(el).display !== "none" && getComputedStyle(el).visibility !== "hidden");
    const focusFirst = () => (controls()[0] ?? panel).focus();
    const isTopDialog = () => {
      const dialogs = Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"][aria-modal="true"]'));
      return dialogs.at(-1) === panel;
    };
    focusFirst();
    const keydown = (event: KeyboardEvent) => {
      if (!isTopDialog() || event.defaultPrevented) return;
      if (event.key === "Escape" && escapeRef.current) {
        event.preventDefault();
        escapeRef.current();
      } else if (event.key === "Tab") {
        const items = controls(), first = items[0], last = items.at(-1);
        if (!first || !panel.contains(document.activeElement) ||
          (event.shiftKey && document.activeElement === first) ||
          (!event.shiftKey && document.activeElement === last)) {
          event.preventDefault();
          (event.shiftKey ? last ?? panel : first ?? panel).focus();
        }
      }
    };
    const focusin = (event: FocusEvent) => {
      if (isTopDialog() && !panel.contains(event.target as Node)) focusFirst();
    };
    document.addEventListener("keydown", keydown);
    document.addEventListener("focusin", focusin);
    return () => {
      document.removeEventListener("keydown", keydown);
      document.removeEventListener("focusin", focusin);
      document.body.style.overflow = overflow;
      if (trigger?.isConnected) trigger.focus();
    };
  }, [open, ref]);
}
