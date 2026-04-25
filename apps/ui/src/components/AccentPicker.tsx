// ============================================================================
// ACCENT PICKER
// Popover button in the top-bar that lets users pick from 4 accent colors.
// Selection persists to localStorage and live-mutates --fx-accent on <html>.
// ============================================================================

import { useEffect, useRef, useState } from "react";
import { Palette } from "lucide-react";
import { useAccent } from "@/hooks/useAccent";

export function AccentPicker() {
  const { accent, accentId, setAccent, options } = useAccent();
  const [open, setOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Close on outside click / escape
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (!popoverRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onClick);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="accent-picker" ref={popoverRef}>
      <button
        type="button"
        className="accent-picker__trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`Theme accent: ${accent.label}`}
        title={`Theme accent: ${accent.label}`}
        onClick={() => setOpen((v) => !v)}
      >
        <Palette className="accent-picker__icon" />
        <span
          className="accent-picker__swatch"
          style={{ background: accent.hex }}
          aria-hidden="true"
        />
      </button>

      {open && (
        <div className="accent-picker__popover" role="menu">
          <div className="accent-picker__label">Accent</div>
          <div className="accent-picker__grid">
            {options.map((opt) => {
              const selected = opt.id === accentId;
              return (
                <button
                  key={opt.id}
                  type="button"
                  role="menuitemradio"
                  aria-checked={selected}
                  className={`accent-picker__option${selected ? " accent-picker__option--selected" : ""}`}
                  title={opt.label}
                  onClick={() => {
                    setAccent(opt.id);
                    setOpen(false);
                  }}
                >
                  <span
                    className="accent-picker__option-swatch"
                    style={{ background: opt.hex }}
                    aria-hidden="true"
                  />
                  <span className="accent-picker__option-label">{opt.label}</span>
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
