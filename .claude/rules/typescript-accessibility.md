# TypeScript/React Accessibility Rules

## Interactive Elements
- Every `onClick` on a non-button/non-anchor element MUST have:
  - `role="button"`
  - `tabIndex={0}`
  - `onKeyDown` handler that triggers on Enter and Space:
    ```tsx
    onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") handler(); }}
    ```
- Prefer `<button>` over `<div onClick>` when possible

## Form Labels
- Every `<label>` MUST have `htmlFor` pointing to the control's `id`
- Every `<select>`, `<input>`, `<textarea>` MUST have a matching `id`
- If a label wraps its control, use `aria-labelledby` instead
- Toggle labels (checkbox wrapping) should use `<label>` element directly

## Media Elements
- `<iframe>` MUST have a `title` attribute describing its content
- `<video>` and `<audio>` MUST contain `<track kind="captions" />`
- Convert self-closing media tags to open/close to include `<track>`

## Number Parsing
- Use `Number.parseInt()` instead of `parseInt()`
- Use `Number.parseFloat()` instead of `parseFloat()`

## String Methods
- Use `String.prototype.codePointAt()` over `charCodeAt()` unless the hash function specifically requires charCode behavior (document with a comment)

## Conditional Rendering
- Use `{condition ? <Component /> : null}` or `{condition && <Component />}`
- When the condition is a number, use `{count > 0 && <Component />}` to avoid rendering `0`
