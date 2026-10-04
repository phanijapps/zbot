// ============================================================================
// MODAL OVERLAY
// Consistent full-screen modal with proper animations
// ============================================================================

import { memo, useId, useRef } from "react";
import { cn } from "./utils";
import { useDialogFocus } from "@/hooks/useDialogFocus";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface ModalOverlayProps {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  className?: string;
  showCloseButton?: boolean;
  showHeader?: boolean;
  closeOnEscape?: boolean;
  closeOnBackdropClick?: boolean;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const XIcon = () => (
  <svg className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M18 6 6 18M6 6l12 12" />
  </svg>
);

// -----------------------------------------------------------------------------
// Modal Overlay Component
// -----------------------------------------------------------------------------

export const ModalOverlay = memo(({
  open,
  onClose,
  title,
  subtitle,
  children,
  footer,
  className,
  showCloseButton = true,
  showHeader = true,
  closeOnEscape = true,
  closeOnBackdropClick = false,
}: ModalOverlayProps) => {
  const contentRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useDialogFocus(open, contentRef, closeOnEscape ? onClose : undefined);

  if (!open) return null;

  return (
    <div className="modal-overlay fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className={cn(
          "absolute inset-0 bg-black/70 backdrop-blur-sm",
          "animate-in fade-in-0 duration-200"
        )}
        onClick={closeOnBackdropClick ? onClose : undefined}
        aria-hidden="true"
      />

      {/* Modal Content */}
      <div
        ref={contentRef}
        className={cn(
          "modal-overlay__surface relative w-full h-full max-h-screen bg-[var(--background)] flex flex-col",
          "animate-in fade-in-0 zoom-in-95 duration-200",
          "data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95",
          className
        )}
        role="dialog"
        tabIndex={-1}
        aria-modal="true"
        aria-labelledby={showHeader ? titleId : undefined}
        aria-label={showHeader ? undefined : title}
      >
        {/* Header */}
        {showHeader && (
          <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 shrink-0">
            <div>
              <h2 id={titleId} className="text-lg font-semibold text-[var(--foreground)]">
                {title}
              </h2>
              {subtitle && (
                <p className="text-sm text-[var(--muted-foreground)] mt-0.5">{subtitle}</p>
              )}
            </div>
            {showCloseButton && (
              <button
                onClick={onClose}
                className="p-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors rounded-lg hover:bg-[var(--muted)]"
                aria-label="Close"
              >
                <XIcon />
              </button>
            )}
          </div>
        )}

        {/* Content */}
        <div className="flex-1 overflow-hidden">
          {children}
        </div>

        {/* Footer */}
        {footer && (
          <div className="px-6 py-4 border-t border-white/10 shrink-0">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
});

ModalOverlay.displayName = "ModalOverlay";
