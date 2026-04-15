import { useState } from "react";
import type { MemoryCategory } from "@/services/transport/types";
import { AddDrawer } from "./AddDrawer";

// MemoryCategory doesn't include "policy". The "+ Policy" button maps to
// "instruction" (a directive), with the user-visible label "Policy" for UX.
// See the Memory Tab Command Deck design doc.
type WriteChoice = { label: string; category: MemoryCategory; key: string };
const CHOICES: WriteChoice[] = [
  { label: "+ Fact", category: "pattern", key: "F" },
  { label: "+ Instruction", category: "instruction", key: "I" },
  { label: "+ Policy", category: "instruction", key: "P" },
];

interface Props {
  wardId: string;
  counts: { facts: number; wiki: number; procedures: number; episodes: number };
  onSave: (v: { category: MemoryCategory; content: string; ward_id: string }) => void;
}

export function WriteRail({ wardId, counts, onSave }: Props) {
  const [open, setOpen] = useState<MemoryCategory | null>(null);

  return (
    <aside className="memory-write">
      <div className="memory-write__title">WRITE</div>
      {CHOICES.map((c) => (
        <button
          key={c.label}
          type="button"
          className="memory-write__btn"
          onClick={() => setOpen(c.category)}
        >
          <span>{c.label}</span>
          <kbd>{c.key}</kbd>
        </button>
      ))}

      <div className="memory-write__stats">
        <div className="memory-write__ward">{wardId || "—"}</div>
        <div>facts {counts.facts}</div>
        <div>wiki {counts.wiki}</div>
        <div>procedures {counts.procedures}</div>
        <div>episodes {counts.episodes}</div>
      </div>

      {open && (
        <AddDrawer
          initialCategory={open}
          wardId={wardId}
          onClose={() => setOpen(null)}
          onSave={(v) => {
            onSave(v);
            setOpen(null);
          }}
        />
      )}
    </aside>
  );
}
