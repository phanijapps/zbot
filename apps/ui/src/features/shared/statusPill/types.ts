// Category of the currently-displayed action — drives color.
export type PillCategory = "read" | "write" | "delegate" | "respond" | "neutral" | "error";

// Computed display state for the pill.
export interface PillState {
  visible: boolean;
  // Primary narration text (from the last Thinking delta), truncated to ~80 chars.
  narration: string;
  // Muted suffix derived from the current ToolCall (e.g., "· yf_fundamentals.py").
  suffix: string;
  category: PillCategory;
  // True when a session is running but no events have arrived yet.
  starting: boolean;
  // Monotonic counter used by UI to trigger slide-in/slide-out animations.
  swapCounter: number;
}

export const EMPTY_PILL: PillState = {
  visible: false,
  narration: "",
  suffix: "",
  category: "neutral",
  starting: false,
  swapCounter: 0,
};

export const NARRATION_MAX = 80;
