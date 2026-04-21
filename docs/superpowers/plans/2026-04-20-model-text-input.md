# Model Text Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every model-selection dropdown in Settings > Advanced and the per-agent edit panel with a themed text input, and drop the backend capability gates so user-typed models are trusted end-to-end.

**Architecture:** One shared React component (`ModelTextInput`) wires every surface to the same themed input + autocomplete list. Backend change is two edits: `ModelRegistry` fallback numbers + removal of the thinking auto-disable in the executor.

**Tech Stack:** React 19, TypeScript, Vitest + React Testing Library, Rust (cargo test). Spec: `docs/superpowers/specs/2026-04-20-model-text-input-design.md`.

---

## File Structure

### New

- `apps/ui/src/features/shared/modelTextInput/ModelTextInput.tsx` — component.
- `apps/ui/src/features/shared/modelTextInput/model-text-input.css` — themed autocomplete styles.
- `apps/ui/src/features/shared/modelTextInput/index.ts` — barrel.
- `apps/ui/src/features/shared/modelTextInput/ModelTextInput.test.tsx` — unit tests.

### Modified

- `apps/ui/src/features/settings/WebSettingsPanel.tsx` — three `<select>` model dropdowns (orchestrator, distillation, multimodal) swap to `ModelTextInput`.
- `apps/ui/src/features/agent/AgentEditPanel.tsx` — one `<select>` model dropdown swaps to `ModelTextInput`.
- `gateway/gateway-services/src/models.rs` — fallback profile context window numbers.
- `gateway/gateway-execution/src/invoke/executor.rs` — drop the thinking auto-disable block.

---

### Task 1: Create `ModelTextInput` shared component

**Files:**
- Create: `apps/ui/src/features/shared/modelTextInput/ModelTextInput.tsx`
- Create: `apps/ui/src/features/shared/modelTextInput/index.ts`
- Create: `apps/ui/src/features/shared/modelTextInput/ModelTextInput.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/ui/src/features/shared/modelTextInput/ModelTextInput.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ModelTextInput } from "./ModelTextInput";

describe("ModelTextInput", () => {
  it("renders as a text input (not a select)", () => {
    render(
      <ModelTextInput value="gpt-4" onChange={() => {}} suggestions={["gpt-4", "gpt-4o"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    expect(input.tagName).toBe("INPUT");
    expect((input as HTMLInputElement).type).toBe("text");
    expect((input as HTMLInputElement).value).toBe("gpt-4");
  });

  it("accepts a value not present in suggestions", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="" onChange={onChange} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.change(input, { target: { value: "nemotron-super:cloud" } });
    expect(onChange).toHaveBeenCalledWith("nemotron-super:cloud");
  });

  it("accepts an empty value (provider default)", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="gpt-4" onChange={onChange} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.change(input, { target: { value: "" } });
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("opens the suggestion list on focus and filters as the user types", () => {
    render(
      <ModelTextInput
        value=""
        onChange={() => {}}
        suggestions={["gpt-4", "gpt-4o-mini", "claude-3-opus"]}
        id="m"
      />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    expect(screen.getByRole("listbox")).toBeTruthy();
    expect(screen.getByText("gpt-4o-mini")).toBeTruthy();
    fireEvent.change(input, { target: { value: "claude" } });
    expect(screen.queryByText("gpt-4o-mini")).toBeNull();
    expect(screen.getByText("claude-3-opus")).toBeTruthy();
  });

  it("commits the highlighted suggestion on Enter", () => {
    const onChange = vi.fn();
    render(
      <ModelTextInput value="" onChange={onChange} suggestions={["gpt-4", "gpt-4o"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("gpt-4");
  });

  it("closes the suggestion list on Escape", () => {
    render(
      <ModelTextInput value="" onChange={() => {}} suggestions={["gpt-4"]} id="m" />,
    );
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```
cd apps/ui && npx vitest run src/features/shared/modelTextInput/ModelTextInput.test.tsx
```
Expected: FAIL with `Cannot find module './ModelTextInput'`.

- [ ] **Step 3: Implement the component**

Create `apps/ui/src/features/shared/modelTextInput/ModelTextInput.tsx`:

```tsx
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
}: ModelTextInputProps) {
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLUListElement | null>(null);
  const listId = useId();

  const filtered = filterSuggestions(suggestions, value);

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
  }, [value]);

  const commit = useCallback(
    (next: string) => {
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
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={activeDescendant}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => setOpen(true)}
        onKeyDown={handleKeyDown}
      />
      {open && filtered.length > 0 && (
        <ul
          id={listId}
          ref={listRef}
          role="listbox"
          className="model-text-input__list"
        >
          {filtered.map((s, i) => (
            <li
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
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function filterSuggestions(suggestions: string[], value: string): string[] {
  if (!value) return suggestions;
  const needle = value.toLowerCase();
  return suggestions.filter((s) => s.toLowerCase().includes(needle));
}
```

Create `apps/ui/src/features/shared/modelTextInput/index.ts`:

```ts
export { ModelTextInput } from "./ModelTextInput";
export type { ModelTextInputProps } from "./ModelTextInput";
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```
cd apps/ui && npx vitest run src/features/shared/modelTextInput/
```
Expected: PASS — `Tests  6 passed (6)`.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/shared/modelTextInput/
git commit -m "feat(ui): shared ModelTextInput with themed autocomplete"
```

---

### Task 2: Theme the autocomplete list

**Files:**
- Create: `apps/ui/src/features/shared/modelTextInput/model-text-input.css`

- [ ] **Step 1: Write the CSS**

Create `apps/ui/src/features/shared/modelTextInput/model-text-input.css`:

```css
/* Shared themed text input with autocomplete. The input itself uses
 * .form-input — this file only styles the floating suggestion list. */

.model-text-input {
  position: relative;
  width: 100%;
}

.model-text-input__list {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  z-index: 20;
  margin: 0;
  padding: 4px;
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.08);
}

.model-text-input__item {
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  color: var(--foreground);
  cursor: pointer;
  font-family: var(--font-mono);
}

.model-text-input__item--active,
.model-text-input__item:hover {
  background: var(--muted);
}
```

- [ ] **Step 2: Run UI tests to verify nothing regressed**

Run:
```
cd apps/ui && npx vitest run src/features/shared/modelTextInput/
```
Expected: PASS — same 6 tests still green.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/shared/modelTextInput/model-text-input.css
git commit -m "style(ui): themed autocomplete list for ModelTextInput"
```

---

### Task 3: Wire `ModelTextInput` into Settings > Advanced

Three model `<select>` elements on the Advanced tab (orchestrator, distillation, multimodal) all get the same treatment. Provider `<select>` stays untouched.

**Files:**
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

- [ ] **Step 1: Import the shared component**

Add to the imports block at the top of `apps/ui/src/features/settings/WebSettingsPanel.tsx`:

```tsx
import { ModelTextInput } from "../shared/modelTextInput";
```

- [ ] **Step 2: Replace the orchestrator model `<select>`**

Find the existing block around line 625–642 (starts with `<label ... htmlFor="orch-model">Model</label>`) and replace the `<select>` element:

```tsx
<div>
  <label className="settings-field-label" htmlFor="orch-model">Model</label>
  <ModelTextInput
    id="orch-model"
    value={execSettings.orchestrator?.model || ""}
    onChange={(next) => handleExecChange({
      orchestrator: {
        ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
        model: next || null,
      },
    })}
    suggestions={providers.find((p) => p.id === (execSettings.orchestrator?.providerId || defaultProviderId))?.models || []}
    placeholder="provider default"
  />
</div>
```

- [ ] **Step 3: Replace the distillation model `<select>`**

Find the existing block around line 730–750 (starts with `<label ... htmlFor="dist-model">Model</label>`) and replace the `<select>` element:

```tsx
<div>
  <label className="settings-field-label" htmlFor="dist-model">Model</label>
  <ModelTextInput
    id="dist-model"
    value={execSettings.distillation?.model || ""}
    onChange={(next) => handleExecChange({
      distillation: {
        ...execSettings.distillation || {},
        model: next || null,
      },
    })}
    suggestions={(() => {
      const distProviderId = execSettings.distillation?.providerId || defaultProviderId;
      return providers.find((p) => p.id === distProviderId)?.models || [];
    })()}
    placeholder="provider default"
  />
</div>
```

- [ ] **Step 4: Replace the multimodal model `<select>`**

Find the existing block around line 770 (the multimodal section, `<label ... htmlFor="mm-model">Model</label>`). Read the current value + onChange wiring for that select and replace with:

```tsx
<div>
  <label className="settings-field-label" htmlFor="mm-model">Model</label>
  <ModelTextInput
    id="mm-model"
    value={execSettings.multimodal?.model || ""}
    onChange={(next) => handleExecChange({
      multimodal: {
        ...execSettings.multimodal || { temperature: 0.3, maxTokens: 4096 },
        model: next || null,
      },
    })}
    suggestions={providers.find((p) => p.id === (execSettings.multimodal?.providerId || defaultProviderId))?.models || []}
    placeholder="provider default"
  />
</div>
```

- [ ] **Step 5: Type-check + manual smoke via vitest**

Run:
```
cd apps/ui && npx tsc --noEmit
```
Expected: no output (clean).

Run the existing settings tests:
```
npx vitest run src/features/settings
```
Expected: all pre-existing tests still pass. (Some tests may have used `getByRole("combobox")` on model selects — `<select>` and the new component both expose `combobox` role, so they should keep passing. If a test targets `<option>` or `getByDisplayValue` on the model specifically, update it to assert the input value instead.)

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/settings/WebSettingsPanel.tsx
git commit -m "feat(settings): advanced model fields use ModelTextInput"
```

---

### Task 4: Wire `ModelTextInput` into `AgentEditPanel`

**Files:**
- Modify: `apps/ui/src/features/agent/AgentEditPanel.tsx`

- [ ] **Step 1: Add the import**

At the top of `apps/ui/src/features/agent/AgentEditPanel.tsx`, add:

```tsx
import { ModelTextInput } from "../shared/modelTextInput";
```

- [ ] **Step 2: Replace the model `<select>` (around line 228–248)**

Find the existing block:

```tsx
<div className="form-group">
  <label className="form-label" htmlFor="edit-agent-model">Model</label>
  <select
    id="edit-agent-model"
    className="form-select"
    value={formData.model || ""}
    onChange={(e) => setFormData({ ...formData, model: e.target.value })}
  >
    {selectedProvider?.models.map((m) => (
      <option key={m} value={m}>{m}</option>
    )) || <option value="">Select a provider first</option>}
  </select>
  {formData.model && modelRegistry[formData.model] && (
    <div style={{ marginTop: "var(--spacing-2)" }}>
      <ModelChip
        modelId={formData.model}
        profile={modelRegistry[formData.model]}
        showContext
      />
    </div>
  )}
</div>
```

Replace with:

```tsx
<div className="form-group">
  <label className="form-label" htmlFor="edit-agent-model">Model</label>
  <ModelTextInput
    id="edit-agent-model"
    value={formData.model || ""}
    onChange={(next) => setFormData({ ...formData, model: next })}
    suggestions={selectedProvider?.models || []}
    placeholder="provider default"
  />
  {formData.model && modelRegistry[formData.model] && (
    <div style={{ marginTop: "var(--spacing-2)" }}>
      <ModelChip
        modelId={formData.model}
        profile={modelRegistry[formData.model]}
        showContext
      />
    </div>
  )}
</div>
```

- [ ] **Step 3: Type-check + run agent tests**

Run:
```
cd apps/ui && npx tsc --noEmit
```
Expected: no output.

Run:
```
npx vitest run src/features/agent
```
Expected: all tests still pass. If any test queried the model dropdown's options (e.g. `getByText("gpt-4o")` to assert it appeared in the list), update to assert `getByRole("combobox", { name: /model/i })` instead and no longer expect the list of options to be in the DOM unless focused.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/agent/AgentEditPanel.tsx
git commit -m "feat(agents): edit panel uses ModelTextInput for model"
```

---

### Task 5: Backend — adjust `ModelRegistry` fallback profile

**Files:**
- Modify: `gateway/gateway-services/src/models.rs` (fallback profile around line 122–139)

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `gateway/gateway-services/src/models.rs`:

```rust
    #[test]
    fn fallback_profile_uses_200k_in_64k_out_with_tools() {
        let registry = ModelRegistry::load(&[], &PathBuf::from("/nonexistent"));
        let profile = registry.get("some-unknown-model-id");
        assert_eq!(profile.context.input, 200_000);
        assert_eq!(profile.context.output, Some(64_000));
        assert!(profile.capabilities.tools);
        assert!(!profile.capabilities.vision);
        assert!(!profile.capabilities.thinking);
        assert!(!profile.capabilities.embeddings);
    }
```

- [ ] **Step 2: Run it to confirm it fails**

Run:
```
cargo test -p gateway-services --lib models::tests::fallback_profile_uses_200k_in_64k_out_with_tools
```
Expected: FAIL — current fallback uses 256_000 / 128_000.

- [ ] **Step 3: Edit the fallback profile**

In `gateway/gateway-services/src/models.rs`, find:

```rust
        let fallback = ModelProfile {
            name: "Unknown Model".to_string(),
            provider: "unknown".to_string(),
            capabilities: ModelCapabilities {
                tools: true,
                vision: false,
                thinking: false,
                embeddings: false,
                voice: false,
                image_generation: false,
                video_generation: false,
            },
            context: ContextWindow {
                input: 256_000,
                output: Some(128_000),
            },
            embedding: None,
        };
```

Replace the `context` field:

```rust
            context: ContextWindow {
                input: 200_000,
                output: Some(64_000),
            },
```

Capabilities stay as-is.

- [ ] **Step 4: Run the test again**

Run:
```
cargo test -p gateway-services --lib models::tests
```
Expected: PASS — all tests in `models::tests`, including the new one.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-services/src/models.rs
git commit -m "fix(models): narrow fallback ctx to 200K/64K for unknown models"
```

---

### Task 6: Backend — remove the thinking auto-disable gate

When `agent.thinking_enabled == true` the executor consults `ModelRegistry.has_capability(&agent.model, Thinking)` and silently flips `thinking_enabled` to `false` if the registry says the model can't think. Spec option A: trust the user — drop the gate.

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs` (lines 311–330)

- [ ] **Step 1: Write the failing test**

Create `gateway/gateway-execution/tests/executor_thinking_tests.rs` (new file):

```rust
//! Executor trusts agent.thinking_enabled regardless of registry capability.

use gateway_execution::invoke::executor;

#[test]
fn thinking_enabled_respected_for_registry_unknown_model() {
    // The behaviour under test is pure: given thinking_enabled=true and a
    // model absent from the registry, the executor should forward the flag
    // as true. We test the helper that the executor uses so the assertion
    // is unit-level rather than a full invoke wiring.
    let out = executor::resolve_thinking_flag(true, "some-unknown-model");
    assert!(out, "thinking_enabled must be respected for unknown models");
}

#[test]
fn thinking_enabled_false_stays_false() {
    let out = executor::resolve_thinking_flag(false, "some-model");
    assert!(!out);
}
```

The test references `executor::resolve_thinking_flag` — that function doesn't exist yet; the next step extracts it.

- [ ] **Step 2: Run the test — confirm it fails**

Run:
```
cargo test -p gateway-execution --test executor_thinking_tests
```
Expected: FAIL — `function or associated item not found: resolve_thinking_flag`.

- [ ] **Step 3: Replace the gate with a pure helper**

In `gateway/gateway-execution/src/invoke/executor.rs`, find:

```rust
        // Validate thinking capability against model registry
        let thinking_enabled = if agent.thinking_enabled {
            if let Some(ref registry) = self.model_registry {
                if !registry
                    .has_capability(&agent.model, gateway_services::models::Capability::Thinking)
                {
                    tracing::warn!(
                        model = %agent.model,
                        "thinking_enabled but model lacks thinking capability — disabling"
                    );
                    false
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            false
        };
```

Replace with:

```rust
        // User-driven: trust agent.thinking_enabled. If the provider
        // rejects the reasoning payload, the LLM client surfaces the error
        // through the normal tool_error path.
        let thinking_enabled = resolve_thinking_flag(agent.thinking_enabled, &agent.model);
```

Then add this public helper near the top of the same file (below the existing imports, above the first `impl` or `fn`):

```rust
/// Public so tests can exercise it without wiring the full invoke path.
/// Takes the user-declared flag verbatim. Keeping this as a separate
/// function makes the "trust the user" decision explicit and makes the
/// unit test independent of the executor's inner state.
pub fn resolve_thinking_flag(user_flag: bool, _model: &str) -> bool {
    user_flag
}
```

The `_model` parameter is kept in the signature for future extensibility (e.g., telemetry that logs which unknown models users turn thinking on for) — not a capability check.

- [ ] **Step 4: Run the test — confirm it passes**

Run:
```
cargo test -p gateway-execution --test executor_thinking_tests
```
Expected: PASS — 2 tests.

Also run the full gateway-execution test suite to confirm nothing else broke:

```
cargo test -p gateway-execution
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs gateway/gateway-execution/tests/executor_thinking_tests.rs
git commit -m "fix(executor): trust agent.thinking_enabled, drop registry gate"
```

---

### Task 7: Backend — audit vision capability gates

Spec option A says we should not gate vision calls against the registry either. An earlier grep showed only ONE site references `Capability::Vision` in production code (`gateway/gateway-services/src/models.rs:90` — the enum match in `has()`, not a gate). This task is a verification pass.

**Files:**
- Read-only scan across `gateway/` and `runtime/`.

- [ ] **Step 1: Grep for vision gates**

Run:
```
grep -rn "Capability::Vision\|has_capability.*Vision" gateway/ runtime/ services/ --include="*.rs"
```

Expected output (current state):
```
gateway/gateway-services/src/models.rs:90:            Capability::Vision => self.vision,
gateway/gateway-services/src/models.rs:303:        assert!(registry.has_capability("gpt-4o", Capability::Vision));
```

The first hit is the enum arm inside `ModelCapabilities::has()`, which is fine — it's how `Vision` maps to the bool field. Not a gate.

The second hit is a unit test inside `mod tests` — fine.

If you see any OTHER hit (for example inside an executor, tool, or a preflight in `gateway/gateway-execution/` or `runtime/agent-tools/`), remove the gate the same way Task 6 removed the thinking gate: replace the `if !has_capability` branch with the trusted path, and add a unit test asserting the trusted path is taken.

- [ ] **Step 2: Grep for multimodal preflights**

Run:
```
grep -rn "multimodal.*capab\|vision_required\|refuse.*vision\|not vision" gateway/ runtime/ services/ --include="*.rs"
```

Expected output: no matches.

- [ ] **Step 3: Verify the multimodal tool path is unconditional**

Inspect `runtime/agent-tools/src/tools/execution/multimodal_analyze.rs` (or equivalent). Read the `execute` function and confirm it does not consult `ModelRegistry` before sending the vision request. If it does, remove that consultation and add a unit test similar to Task 6's `resolve_thinking_flag` pattern.

- [ ] **Step 4: No-op commit if clean (skip if changes were needed)**

If Steps 1–3 found nothing to change, skip this step — there is nothing to commit. If a gate was found and removed, commit:

```bash
git add gateway/ runtime/
git commit -m "fix(vision): trust user model choice, drop registry gate"
```

---

### Task 8: Run the full test suites

**Files:** none (sanity run).

- [ ] **Step 1: UI feature tests**

Run:
```
cd apps/ui && npx vitest run src/features src/components
```
Expected: all tests pass (pre-existing flakes in `tests/integration/dashboard.test.tsx` and `src/services/transport/http.test.ts` for Connector Inbound Log may still fail — those are unrelated to this change).

- [ ] **Step 2: Rust tests touched by this plan**

Run:
```
cargo test -p gateway-services --lib models
cargo test -p gateway-execution
```
Expected: all pass.

- [ ] **Step 3: Commit any incidental fixes (if any)**

If Step 1 or 2 surfaced a test that breaks because of the dropdown → input swap, update the assertion (typically changing `getByRole("combobox")` still works for both, but `getByDisplayValue` or queries that iterate `<option>` children may need rewriting). Commit per incidental fix with a message like:

```bash
git add <files>
git commit -m "test: assert ModelTextInput value instead of dropdown options"
```

---

## Self-review checklist

**1. Spec coverage**

| Spec section | Implemented by |
|---|---|
| UI component `ModelTextInput` (props, behavior, styling) | Tasks 1 + 2 |
| Settings > Advanced — Orchestrator / Distillation / Multimodal model fields become text | Task 3 (three sub-steps, one per panel) |
| Agent edit panel model field becomes text | Task 4 |
| Provider picker stays a dropdown | Intentionally not touched in Tasks 3 + 4 |
| Fallback profile numbers 200K / 64K / tools=true | Task 5 |
| Thinking auto-disable removed | Task 6 |
| Vision gate audit (and removal if found) | Task 7 |
| Stored config shapes unchanged (no migration) | Covered — Tasks 3 + 4 preserve the `{providerId, model}` shape |
| Error surfacing via existing LLM error path | Covered — no new plumbing added |

**2. Placeholder scan**

All code blocks contain full, runnable snippets. No "TBD" / "TODO" / "implement later" / "similar to Task N" references. Every function, class, prop, and CSS variable used appears defined or referenced in this plan or the files it points at.

**3. Type consistency**

- `ModelTextInputProps` shape matches every call site (id, value, onChange, suggestions, placeholder).
- `onChange(next: string)` returns a bare string; every consumer wraps it into its own config mutator.
- `resolve_thinking_flag(user_flag: bool, _model: &str) -> bool` — signature is stable; the executor call site uses `&agent.model` and `agent.thinking_enabled` which are `&str` and `bool` respectively.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-20-model-text-input.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
