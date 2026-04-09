import { useState, useMemo } from "react";
import type { ReactNode } from "react";

/** Shape of a single fact from memory.recall JSON */
interface RecallFact {
  key?: string;
  content?: string;
  category?: string;
  confidence?: number;
}

/** Shape of a single episode from memory.recall JSON */
interface RecallEpisode {
  summary?: string;
  outcome?: string;
}

/** Parsed recall data */
interface RecallData {
  facts: RecallFact[];
  episodes: RecallEpisode[];
}

export interface RecallBlockProps {
  /** Raw JSON string from the memory.recall tool result */
  raw: string;
}

/** Collapse threshold — if total items exceed this, show "Show more" toggle */
const COLLAPSE_THRESHOLD = 6;

/**
 * Attempt to parse recall JSON. Returns structured data or null on failure.
 */
function parseRecall(raw: string): RecallData | null {
  try {
    const parsed = JSON.parse(raw);
    const facts: RecallFact[] = Array.isArray(parsed.facts) ? parsed.facts : [];
    const episodes: RecallEpisode[] = Array.isArray(parsed.episodes) ? parsed.episodes : [];
    return { facts, episodes };
  } catch {
    return null;
  }
}

/**
 * RecallBlock — parses JSON from memory.recall tool result.
 * Sections: corrections (red), episodes (outcome badge), domain facts (muted).
 * Collapses with "Show more" toggle when > 6 items total.
 */
export function RecallBlock({ raw }: RecallBlockProps) {
  const [expanded, setExpanded] = useState(false);
  const data = useMemo(() => parseRecall(raw), [raw]);

  // Fallback: show raw text if JSON is malformed
  if (!data) {
    return (
      <div className="recall-block">
        <div className="recall-block__header">Memory Recall</div>
        <div className="recall-block__fact">{raw}</div>
      </div>
    );
  }

  const corrections = data.facts.filter((f) => f.category === "correction");
  const domainFacts = data.facts.filter((f) => f.category !== "correction");
  const { episodes } = data;

  const totalItems = corrections.length + episodes.length + domainFacts.length;
  const needsCollapse = totalItems > COLLAPSE_THRESHOLD;

  // Build ordered list of renderable items
  const allItems: ReactNode[] = [];

  corrections.forEach((c, i) => {
    allItems.push(
      <div key={`corr-${i}`} className="recall-block__correction">
        {c.content || c.key || "correction"}
      </div>,
    );
  });

  episodes.forEach((ep, i) => {
    const badge = ep.outcome === "success" ? "\u2713" : "\u2717";
    allItems.push(
      <div key={`ep-${i}`} className="recall-block__episode">
        <span>{badge}</span> {ep.summary || "episode"}
      </div>,
    );
  });

  domainFacts.forEach((f, i) => {
    allItems.push(
      <div key={`fact-${i}`} className="recall-block__fact">
        {f.content || f.key || "fact"}
      </div>,
    );
  });

  const visibleItems = needsCollapse && !expanded ? allItems.slice(0, COLLAPSE_THRESHOLD) : allItems;

  return (
    <div className="recall-block">
      <div className="recall-block__header">
        <span>Memory Recall</span>
        <span className="recall-block__meta">
          {data.facts.length} facts, {episodes.length} episodes
        </span>
      </div>
      {visibleItems}
      {needsCollapse && (
        <button
          className="btn btn--ghost btn--sm"
          onClick={() => setExpanded((prev) => !prev)}
        >
          {expanded ? "Show less" : `Show ${totalItems - COLLAPSE_THRESHOLD} more`}
        </button>
      )}
    </div>
  );
}
