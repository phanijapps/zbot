// ============================================================================
// CHAT SLIDER
// Slide-in panel for chat that overlays the main content
// ============================================================================

import { useEffect, useCallback } from "react";
import { ChevronRight } from "lucide-react";

interface ChatSliderProps {
  isOpen: boolean;
  onClose: () => void;
  children: React.ReactNode;
}

export function ChatSlider({ isOpen, onClose, children }: ChatSliderProps) {
  // Handle escape key
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen) {
        onClose();
      }
    },
    [isOpen, onClose]
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Prevent body scroll when slider is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [isOpen]);

  return (
    <>
      {/* Backdrop */}
      <div
        className={`chat-slider__backdrop ${isOpen ? "chat-slider__backdrop--visible" : ""}`}
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Slider Panel */}
      <div
        className={`chat-slider ${isOpen ? "chat-slider--open" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label="Chat"
      >
        {/* Close Handle */}
        <button
          className="chat-slider__handle"
          onClick={onClose}
          title="Close chat (Esc)"
        >
          <ChevronRight size={20} />
        </button>

        {/* Chat Content */}
        <div className="chat-slider__content">
          {children}
        </div>
      </div>
    </>
  );
}

export default ChatSlider;
