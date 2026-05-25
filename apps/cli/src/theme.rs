//! Visual theme — Direction B: Cool Terminal.
//!
//! Slate-grey neutrals + soft violet primary + cool sky secondary. Inspired
//! by Linear / Cursor / Zed. All Tailwind-named for traceability.
//!
//! Centralised so future theme swaps are a single-file change.

use iocraft::Color;

// ── Primary accent — soft violet ────────────────────────────────────────────
/// violet-400 — used for primary chrome (input prompt, welcome border).
pub const ACCENT: Color = Color::Rgb {
    r: 167,
    g: 139,
    b: 250,
};
/// violet-600 — dim variant, reserved for future use (e.g. focused footer cap).
#[allow(dead_code)]
pub const ACCENT_DIM: Color = Color::Rgb {
    r: 124,
    g: 58,
    b: 237,
};

// ── Secondary — cool sky for the assistant ─────────────────────────────────
/// sky-400 — assistant border + role label.
pub const SECONDARY: Color = Color::Rgb {
    r: 56,
    g: 189,
    b: 248,
};

// ── Text ────────────────────────────────────────────────────────────────────
pub const TEXT: Color = Color::Reset;
pub const MUTED: Color = Color::DarkGrey;
/// slate-600 — dimmer than MUTED, used for status footer.
pub const MUTED_DIM: Color = Color::Rgb {
    r: 100,
    g: 116,
    b: 139,
};

// ── Borders ─────────────────────────────────────────────────────────────────
pub const BORDER_USER: Color = ACCENT;
pub const BORDER_ASSISTANT: Color = SECONDARY;
/// amber-400 — system / slash output.
pub const BORDER_SYSTEM: Color = Color::Rgb {
    r: 251,
    g: 191,
    b: 36,
};
/// slate-500 — nested tool-call cards.
pub const BORDER_TOOL: Color = Color::Rgb {
    r: 100,
    g: 116,
    b: 139,
};

// ── Status colors ───────────────────────────────────────────────────────────
/// emerald-500.
pub const SUCCESS: Color = Color::Rgb {
    r: 34,
    g: 197,
    b: 94,
};
/// red-400.
pub const ERROR: Color = Color::Rgb {
    r: 248,
    g: 113,
    b: 113,
};

// ── Spinner ─────────────────────────────────────────────────────────────────
/// Braille-pattern spinner frames. Ticks every 80 ms while a turn is in flight.
pub const SPINNER_FRAMES: &[&str] = &[
    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
];

pub const SPINNER_TICK_MS: u64 = 80;
