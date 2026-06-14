export interface NamePreset {
  id: string;
  name: string;
  emoji: string;
  tagline: string;
}

export const NAME_PRESETS: NamePreset[] = [
  { id: "mindful-companion", name: "Mindful Companion", emoji: "\uD83C\uDF3F", tagline: "Calm support with a light touch" },
  { id: "jarvis", name: "Jarvis", emoji: "\u26A1", tagline: "Polished, quick, and a little theatrical" },
  { id: "judy", name: "Judy", emoji: "\uD83C\uDFAE", tagline: "Streetwise coding partner with sharp humor" },
  { id: "camina", name: "CAMINA", emoji: "\uD83D\uDE80", tagline: "No-nonsense captain energy for hard tasks" },
  { id: "ghost-protocol", name: "Ghost Protocol", emoji: "\uD83D\uDD76\uFE0F", tagline: "Stealthy, curious, dramatic problem solving" },
  { id: "database-whisperer", name: "Database Whisperer", emoji: "\uD83D\uDCCA", tagline: "Turns slow queries into good stories" },
  { id: "kuma", name: "Kuma", emoji: "\uD83C\uDF8C", tagline: "Playful language coach for Japanese practice" },
  { id: "custom", name: "Custom...", emoji: "\u2728", tagline: "Choose your own name" },
];

export const DEFAULT_NAME_PRESET = NAME_PRESETS[0];
