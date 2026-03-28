# UI Architecture Principles

## SOLID Principles for UI

### Single Responsibility
- **theme.css**: Only defines design tokens (colors, spacing, typography, shadows)
- **components.css**: Only defines reusable component classes
- **React components**: Only handle logic and structure, NOT styling decisions

### Open/Closed Principle
- Components are **open for extension** via CSS modifier classes (e.g., `.btn--primary`, `.card--interactive`)
- Components are **closed for modification** - don't change base classes, add modifiers
- To change UI appearance, edit CSS files, NOT component code

### Liskov Substitution
- Any component using `.btn` can use `.btn--primary` or `.btn--secondary` interchangeably
- Variant classes extend base behavior, don't break it

### Interface Segregation
- Small, focused CSS classes that do one thing
- Don't create "god classes" that handle everything
- Compose multiple classes: `class="btn btn--primary btn--sm"`

### Dependency Inversion
- Components depend on **abstract** design tokens (`var(--primary)`), not concrete values
- Never hardcode colors, spacing, or fonts in components

---

## File Structure

```
src/styles/
├── index.css          # Entry point - imports in correct order
├── theme.css          # Design tokens (CSS custom properties)
└── components.css     # Reusable component classes
```

### theme.css - Design Tokens

Contains ALL design decisions:
- Colors (with semantic names: `--primary`, `--destructive`, `--muted-foreground`)
- Typography (`--text-sm`, `--font-mono`)
- Spacing scale (`--spacing-1` through `--spacing-12`)
- Border radius (`--radius-sm`, `--radius-md`, `--radius-lg`)
- Shadows (`--shadow-card`, `--shadow-modal`)
- Layout dimensions (`--sidebar-width`, `--modal-width`)

**Rule**: To change how the app looks, edit ONLY this file.

### components.css - Component Classes

Defines reusable patterns using design tokens:

```css
.card {
  background-color: var(--card);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
}

.btn--primary {
  background-color: var(--primary);
  color: var(--primary-foreground);
}
```

**Rule**: These classes use tokens from theme.css, never hardcoded values.

---

## Component Class Naming

Follow BEM-inspired conventions:

```
.block                    # Component root (e.g., .card, .btn, .modal)
.block--modifier          # Variant (e.g., .card--interactive, .btn--primary)
.block__element           # Child element (e.g., .card__header, .btn__icon)
.block__element--modifier # Child variant (e.g., .modal__header--compact)
```

### Examples:

```html
<!-- Card with interactive hover state -->
<div class="card card--interactive card__padding">
  <div class="card__header">
    <div class="card__icon card__icon--primary">...</div>
  </div>
</div>

<!-- Primary button, medium size -->
<button class="btn btn--primary btn--md">
  <span class="btn__icon">...</span>
  Submit
</button>

<!-- Split panel layout -->
<div class="split-panel">
  <div class="split-panel__sidebar">...</div>
  <div class="split-panel__main">...</div>
</div>
```

---

## What NOT to Do

### Never put styles inline in components

```jsx
// BAD - styles mixed in component
<div className="bg-[var(--card)] rounded-xl p-4 shadow-lg hover:shadow-xl">

// GOOD - semantic class names
<div className="card card--interactive card__padding">
```

### Never hardcode values

```css
/* BAD */
.card { background: #ffffff; border-radius: 12px; }

/* GOOD */
.card { background: var(--card); border-radius: var(--radius-lg); }
```

### Never mix concerns

```jsx
// BAD - component decides styling
const Card = () => (
  <div style={{ padding: isLarge ? '24px' : '16px' }}>

// GOOD - component uses CSS classes
const Card = ({ size }) => (
  <div className={`card card__padding${size === 'lg' ? '--lg' : ''}`}>
```

---

## Available Component Classes

### Layout
- `.app-shell` - Main app container (flex, full height)
- `.page` - Page wrapper with scroll
- `.page-container` - Centered content with max-width
- `.page-header` - Title + actions layout
- `.split-panel` - Sidebar + main content layout

### Sidebar
- `.sidebar` - Dark sidebar container
- `.nav-link`, `.nav-link--active` - Navigation items
- `.connection-status` - Status indicator

### Cards
- `.card` - Base card with shadow
- `.card--interactive` - Hover state
- `.card--bordered` - Border instead of shadow
- `.card__padding`, `.card__padding--lg` - Padding variants
- `.card__icon--primary/warning/success/destructive` - Icon backgrounds

### Buttons
- `.btn` - Base button
- `.btn--sm/md/lg` - Sizes
- `.btn--primary/secondary/ghost/outline/destructive` - Variants
- `.btn--icon`, `.btn--icon-ghost` - Icon-only buttons

### Forms
- `.form-group` - Label + input wrapper
- `.form-label` - Label styling
- `.form-input` - Text input
- `.form-textarea` - Textarea
- `.form-select` - Select dropdown

### Feedback
- `.badge`, `.badge--primary/success/warning/destructive` - Status badges
- `.alert`, `.alert--info/success/warning/error` - Alert messages
- `.empty-state` - Empty content placeholder
- `.loading-spinner` - Loading indicator

### Lists
- `.list-item`, `.list-item--active` - Clickable list items
- `.list-category` - Category header

### Modal
- `.modal-backdrop` - Overlay
- `.modal`, `.modal--sm/lg` - Dialog container
- `.modal__header/body/footer` - Modal sections

### Provider Page
- `.provider-grid` - Responsive 2-column card grid
- `.provider-card`, `.provider-card--active` - Provider card with status border
- `.provider-card__status/name/url/models` - Card child elements
- `.model-chip`, `.model-chip--removable` - Model tag with capability badges
- `.model-chip__name/capabilities/context/remove` - Chip child elements
- `.provider-slideover`, `.provider-slideover--open` - Slide-over detail panel
- `.provider-slideover__backdrop/header/body/footer` - Slide-over sections
- `.preset-card`, `.preset-card--dimmed` - Onboarding preset cards
- `.inline-connect` - Expanded preset connect form
- `.connection-bar`, `.connection-bar--ok/warn/error` - Status indicator
- `.field-label`, `.field-value`, `.field-value--mono` - Read-only field display

### Settings Page
- Uses `.card`, `.settings-toggle-btn`, `.settings-expandable` from base components
- `.settings-info-card`, `.settings-toggle-option`, `.settings-field-label` for form controls

---

## Adding New Components

1. Define CSS class in `components.css`
2. Use ONLY design tokens from `theme.css`
3. Follow BEM naming convention
4. Add modifiers for variants
5. Document in this file

---

## Theming

To create a new theme:
1. Copy the `:root` section from `theme.css`
2. Create a new class (e.g., `.theme-dark`, `.theme-blue`)
3. Override the CSS custom properties
4. Apply the class to `<html>` or `<body>`

```css
.theme-corporate {
  --primary: #0066cc;
  --primary-hover: #0052a3;
  --sidebar: #003366;
  /* ... */
}
```

---

## Migration Guide

When refactoring existing components:

1. Identify inline Tailwind classes
2. Map to semantic component classes
3. If no class exists, create one in `components.css`
4. Replace inline styles with class names
5. Test that appearance is unchanged
