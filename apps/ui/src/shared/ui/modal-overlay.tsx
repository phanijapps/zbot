// ============================================================================
// MODAL OVERLAY
// Consistent full-screen modal with proper animations
// ============================================================================

import { memo, useEffect, useRef } from "react";
import { cn } from "./utils";

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

  // Handle ESC key
  useEffect(() => {
    if (!closeOnEscape || !open) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, onClose, closeOnEscape]);

  // Prevent body scroll when modal is open
  useEffect(() => {
    if (open) {
      document.body.style.overflow = "hidden";
      return () => {
        document.body.style.overflow = "";
      };
    }
  }, [open]);

  // Focus trap
  useEffect(() => {
    if (open && contentRef.current) {
      const focusableElements = contentRef.current.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      const firstElement = focusableElements[0] as HTMLElement;
      firstElement?.focus();
    }
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
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
          "relative w-full h-full max-h-screen bg-[#0a0a0a] flex flex-col",
          "animate-in fade-in-0 zoom-in-95 duration-200",
          "data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95",
          className
        )}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
      >
        {/* Header */}
        {showHeader && (
          <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 shrink-0">
            <div>
              <h2 id="modal-title" className="text-lg font-semibold text-[var(--foreground)]">
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
