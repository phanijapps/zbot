export interface NamePreset {
  id: string;
  name: string;
  emoji: string;
  tagline: string;
}

export const NAME_PRESETS: NamePreset[] = [
  { id: "bhrami", name: "Bhrami Software Engineer", emoji: "\uD83C\uDFAD", tagline: "Witty, resourceful, always has a plan" },
  { id: "gajala", name: "Gajala CEO Sonic Solutions", emoji: "\uD83D\uDE02", tagline: "Energetic, creative, makes work fun" },
  { id: "zbot", name: "z-Bot", emoji: "\uD83E\uDD16", tagline: "Professional, focused, gets things done" },
  { id: "custom", name: "Custom...", emoji: "\u2728", tagline: "Choose your own name" },
];
