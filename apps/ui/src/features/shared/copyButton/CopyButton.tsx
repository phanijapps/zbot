import { useCallback, useState } from "react";
import { Check, Copy } from "lucide-react";
import "./copy-button.css";

const COPY_FEEDBACK_MS = 1500;

/** Copy text to the clipboard and flash a "Copied" state for 1.5s. */
export function useCopyToClipboard(): {
  copied: boolean;
  onCopy: (text: string) => Promise<void>;
} {
  const [copied, setCopied] = useState(false);
  const onCopy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
    } catch {
      // ignore — clipboard unavailable
    }
  }, []);
  return { copied, onCopy };
}

export interface CopyButtonProps {
  /** Raw text to copy. Usually markdown for assistant responses. */
  text: string;
  /** aria-label / title. Defaults to "Copy message". */
  label?: string;
  /** Extra class names. */
  className?: string;
}

/**
 * Small icon-only copy-to-clipboard button. Hidden by default; revealed
 * when the closest ancestor has `data-copy-host="true"` and is hovered
 * or when the button itself is focused.
 */
export function CopyButton({ text, label = "Copy message", className }: CopyButtonProps) {
  const { copied, onCopy } = useCopyToClipboard();
  const classes = ["copy-btn", className].filter(Boolean).join(" ");
  return (
    <button
      type="button"
      className={classes}
      data-testid="copy-btn"
      onClick={() => void onCopy(text)}
      aria-label={copied ? "Copied" : label}
      title={copied ? "Copied" : label}
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
    </button>
  );
}
