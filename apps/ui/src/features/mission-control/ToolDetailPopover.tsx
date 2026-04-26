// ============================================================================
// MISSION CONTROL — ToolDetailPopover
// Modal popover that shows the full input + output of a tool call. Triggered
// by clicking a row in the Tools pane.
// ============================================================================

import { useEffect, useRef } from "react";
import { X } from "lucide-react";
import type { TraceNode } from "../logs/trace-types";
import { formatDuration } from "../logs/trace-types";

interface ToolDetailPopoverProps {
  tool: TraceNode | null;
  onClose(): void;
}

export function ToolDetailPopover({ tool, onClose }: ToolDetailPopoverProps) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!tool) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [tool, onClose]);

  if (!tool) return null;

  return (
    <>
      <button
        type="button"
        className="tool-popover__backdrop"
        aria-label="Close tool details"
        onClick={onClose}
      />
      <div
        ref={dialogRef}
        className="tool-popover"
        role="dialog"
        aria-modal="true"
        aria-label={`${tool.label} tool details`}
      >
        <header className="tool-popover__head">
          <div className="tool-popover__title">
            <span className="tool-popover__name">{tool.label}</span>
            {tool.summary && <span className="tool-popover__summary">{tool.summary}</span>}
          </div>
          <button type="button" className="tool-popover__close" aria-label="Close" onClick={onClose}>
            <X size={14} />
          </button>
        </header>
        <div className="tool-popover__meta">
          <Stat label="agent" value={tool.agentId} />
          {tool.durationMs !== undefined && <Stat label="duration" value={formatDuration(tool.durationMs)} />}
          {tool.status && <Stat label="status" value={tool.status} />}
          {tool.tokenCount !== undefined && tool.tokenCount > 0 && (
            <Stat label="tokens" value={tool.tokenCount.toLocaleString()} />
          )}
        </div>
        <div className="tool-popover__body">
          <Section label="Input" content={tool.args} />
          <Section label="Output" content={tool.result} />
          {tool.error && <Section label="Error" content={tool.error} variant="error" />}
        </div>
      </div>
    </>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="tool-popover__stat">
      <span className="tool-popover__stat-label">{label}</span>
      <span className="tool-popover__stat-value">{value}</span>
    </div>
  );
}

interface SectionProps {
  label: string;
  content?: string | null;
  variant?: "default" | "error";
}

function Section({ label, content, variant = "default" }: SectionProps) {
  const formatted = formatJson(content);
  return (
    <section className="tool-popover__section">
      <h3 className="tool-popover__section-label">{label}</h3>
      {!content && <div className="tool-popover__empty">empty</div>}
      {content && (
        <pre className={`tool-popover__pre tool-popover__pre--${variant}`}>{formatted}</pre>
      )}
    </section>
  );
}

/** If the string is JSON, pretty-print it; otherwise return as-is. */
export function formatJson(s: string | null | undefined): string {
  if (s === null || s === undefined) return "";
  const trimmed = s.trim();
  if (trimmed.length === 0) return "";
  if (trimmed[0] !== "{" && trimmed[0] !== "[") return s;
  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2);
  } catch {
    return s;
  }
}
