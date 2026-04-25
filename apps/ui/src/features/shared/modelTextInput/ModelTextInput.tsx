// =============================================================================
// ModelTextInput — themed free-text input with filtered autocomplete
// suggestions. Used by Settings > Advanced (orchestrator / distillation /
// multimodal) and AgentEditPanel. Provider stays a dropdown next to this;
// this component does not know about providers.
// =============================================================================

import { useCallback, useEffect, useId, useRef, useState } from "react";
import "./model-text-input.css";

export interface ModelTextInputProps {
  /** Current model value. Empty string means "use the provider default". */
  value: string;
  /** Fires on every keystroke and on suggestion-click / Enter commit. */
  onChange(next: string): void;
  /** Values to show in the suggestion list. Typically the selected
   *  provider's `models` array. Filtering against the current value is
   *  done inside this component. */
  suggestions: string[];
  /** Placeholder when the field is empty. */
  placeholder?: string;
  /** id for <label htmlFor>. */
  id?: string;
  disabled?: boolean;
}

export function ModelTextInput({
  value,
  onChange,
  suggestions,
  placeholder = "provider default",
  id,
  disabled = false,
}: Readonly<ModelTextInputProps>) {
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  // inputValue tracks what is displayed in the input and used for filtering.
  // It stays in sync with the `value` prop but also reflects mid-keystroke
  // text so filtering works even when the parent hasn't re-rendered yet.
  const [inputValue, setInputValue] = useState(value);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const listId = useId();

  // Keep inputValue in sync when the prop changes from outside (e.g. commit).
  useEffect(() => {
    setInputValue(value);
  }, [value]);

  const filtered = filterSuggestions(suggestions, inputValue);

  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (inputRef.current?.contains(target)) return;
      if (listRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  useEffect(() => {
    setHighlight(-1);
  }, [inputValue]);

  const commit = useCallback(
    (next: string) => {
      setInputValue(next);
      onChange(next);
      setOpen(false);
    },
    [onChange],
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (!open) setOpen(true);
      setHighlight((h) => Math.min(filtered.length - 1, h + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHighlight((h) => Math.max(-1, h - 1));
    } else if (e.key === "Enter") {
      if (open && highlight >= 0 && filtered[highlight] !== undefined) {
        e.preventDefault();
        commit(filtered[highlight]);
      }
    } else if (e.key === "Escape") {
      if (open) {
        e.preventDefault();
        setOpen(false);
      }
    }
  };

  const activeDescendant =
    open && highlight >= 0 ? `${listId}-opt-${highlight}` : undefined;

  return (
    <div className="model-text-input">
      <input
        ref={inputRef}
        id={id}
        type="text"
        role="combobox"
        className="form-input"
        value={inputValue}
        placeholder={placeholder}
        disabled={disabled}
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={activeDescendant}
        onChange={(e) => {
          setInputValue(e.target.value);
          onChange(e.target.value);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={handleKeyDown}
      />
      {open && filtered.length > 0 && (
        <div
          id={listId}
          ref={listRef}
          role="listbox"
          className="model-text-input__list"
        >
          {filtered.map((s, i) => (
            <div
              key={s}
              id={`${listId}-opt-${i}`}
              role="option"
              aria-selected={i === highlight}
              className={
                "model-text-input__item" +
                (i === highlight ? " model-text-input__item--active" : "")
              }
              onMouseDown={(e) => {
                // mousedown (not click) so input blur fires after we commit
                e.preventDefault();
                commit(s);
              }}
              onMouseEnter={() => setHighlight(i)}
            >
              {s}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function filterSuggestions(suggestions: string[], value: string): string[] {
  if (!value) return suggestions;
  const needle = value.toLowerCase();
  return suggestions.filter((s) => s.toLowerCase().includes(needle));
}
