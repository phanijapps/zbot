import type { ReactElement } from "react";
import type { QuickChatInlineChip } from "./types";
import { Brain, BookOpen, ArrowRight } from "lucide-react";

export interface InlineActivityChipProps {
  chip: QuickChatInlineChip;
}

const KIND_META: Record<QuickChatInlineChip["kind"], { icon: ReactElement; color: string }> = {
  recall: { icon: <Brain size={12} />, color: "rgb(100,200,255)" },
  skill: { icon: <BookOpen size={12} />, color: "rgb(200,150,255)" },
  delegate: { icon: <ArrowRight size={12} />, color: "rgb(255,180,100)" },
};

export function InlineActivityChip({ chip }: InlineActivityChipProps) {
  const meta = KIND_META[chip.kind];
  return (
    <span
      className="quick-chat__chip"
      data-kind={chip.kind}
      style={{ color: meta.color, borderColor: `${meta.color}55`, background: `${meta.color}1a` }}
      title={chip.detail}
    >
      {meta.icon}
      <span>{chip.label}</span>
    </span>
  );
}
