# "Warm Sand" UI Overhaul — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Overhaul z-Bot's UI from purple/cool tones to a warm, Notion-inspired "Warm Sand" aesthetic with amber/copper accents, unified sidebar, and dark-mode-first design.

**Architecture:** CSS-first approach — most changes are in `theme.css` (design tokens) and `components.css` (component styles). React files only change where inline styles or hardcoded colors exist. No structural/routing/API changes.

**Tech Stack:** CSS custom properties, Tailwind v4, React 19, Radix UI, Lucide icons

**Design doc:** `docs/plans/2026-03-07-ui-overhaul-design.md`

---

### Task 1: Overhaul theme.css — Light Mode Colors

**Files:**
- Modify: `apps/ui/src/styles/theme.css:10-159` (`:root` block)

**Step 1: Replace the entire `:root` block with new light mode tokens**

Replace lines 10-159 in theme.css (the `:root { ... }` block) with:

```css
:root {
  /* ========================================================================
     COLORS - Warm Sand palette (light mode)
     ======================================================================== */

  /* Background & Foreground */
  --background: #F7F5F2;
  --foreground: #37352F;

  /* Card surfaces */
  --card: #FFFFFF;
  --card-foreground: #37352F;

  /* Popover/Dropdown */
  --popover: #FFFFFF;
  --popover-foreground: #37352F;

  /* Primary brand color (amber/copper) */
  --primary: #C17D3F;
  --primary-hover: #A86A30;
  --primary-foreground: #FFFFFF;
  --primary-muted: rgba(193, 125, 63, 0.08);

  /* Secondary */
  --secondary: #F0EDE8;
  --secondary-hover: #E3DFD8;
  --secondary-foreground: #787570;

  /* Muted elements */
  --muted: #F0EDE8;
  --muted-foreground: #787570;

  /* Accent highlights */
  --accent: rgba(193, 125, 63, 0.06);
  --accent-foreground: #C17D3F;

  /* Selection (list items, sidebar) */
  --selection: rgba(193, 125, 63, 0.12);
  --selection-border: #C17D3F;

  /* Semantic: Destructive/Error */
  --destructive: #CC4040;
  --destructive-hover: #B83636;
  --destructive-foreground: #FFFFFF;
  --destructive-muted: rgba(204, 64, 64, 0.08);

  /* Semantic: Success */
  --success: #4EA04E;
  --success-hover: #429042;
  --success-foreground: #FFFFFF;
  --success-muted: rgba(78, 160, 78, 0.08);

  /* Semantic: Warning */
  --warning: #B8892E;
  --warning-hover: #A07824;
  --warning-foreground: #FFFFFF;
  --warning-muted: rgba(184, 137, 46, 0.08);

  /* Borders & Inputs */
  --border: #E3DFD8;
  --input: #E3DFD8;
  --input-background: #FFFFFF;

  /* Focus ring */
  --ring: #C17D3F;
  --ring-muted: rgba(193, 125, 63, 0.2);

  /* Overlay */
  --overlay: rgba(0, 0, 0, 0.4);

  /* ========================================================================
     SIDEBAR - Unified with page, slightly tinted
     ======================================================================== */
  --sidebar: #EFECE7;
  --sidebar-foreground: #37352F;
  --sidebar-primary: #C17D3F;
  --sidebar-primary-foreground: #FFFFFF;
  --sidebar-accent: #E8E4DC;
  --sidebar-accent-hover: rgba(232, 228, 220, 0.5);
  --sidebar-border: #E3DFD8;
  --sidebar-ring: #C17D3F;
  --sidebar-muted: #9B9689;

  /* ========================================================================
     TYPOGRAPHY
     ======================================================================== */
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Monaco, Consolas, monospace;

  --text-xs: 0.75rem;      /* 12px */
  --text-sm: 0.875rem;     /* 14px */
  --text-base: 1rem;       /* 16px */
  --text-lg: 1.125rem;     /* 18px */
  --text-xl: 1.25rem;      /* 20px */
  --text-2xl: 1.5rem;      /* 24px */
  --text-3xl: 1.875rem;    /* 30px */

  /* ========================================================================
     SPACING - Consistent spacing scale
     ======================================================================== */
  --spacing-0-5: 0.125rem;  /* 2px */
  --spacing-1: 0.25rem;     /* 4px */
  --spacing-1-5: 0.375rem;  /* 6px */
  --spacing-2: 0.5rem;      /* 8px */
  --spacing-2-5: 0.625rem;  /* 10px */
  --spacing-3: 0.75rem;     /* 12px */
  --spacing-4: 1rem;        /* 16px */
  --spacing-5: 1.25rem;     /* 20px */
  --spacing-6: 1.5rem;      /* 24px */
  --spacing-8: 2rem;        /* 32px */
  --spacing-10: 2.5rem;     /* 40px */
  --spacing-12: 3rem;       /* 48px */

  /* ========================================================================
     BORDER RADIUS
     ======================================================================== */
  --radius-sm: 0.375rem;    /* 6px */
  --radius-md: 0.5rem;      /* 8px */
  --radius-lg: 0.75rem;     /* 12px */
  --radius-xl: 1rem;        /* 16px */
  --radius-full: 9999px;

  /* ========================================================================
     SHADOWS (warm-tinted, Notion-style)
     ======================================================================== */
  --shadow-card: 0 1px 2px rgba(55, 53, 47, 0.04), 0 3px 8px rgba(55, 53, 47, 0.04);
  --shadow-card-hover: 0 2px 6px rgba(55, 53, 47, 0.06), 0 6px 16px rgba(55, 53, 47, 0.06);
  --shadow-modal: 0 4px 16px rgba(55, 53, 47, 0.1), 0 12px 32px rgba(55, 53, 47, 0.12);
  --shadow-dropdown: 0 4px 16px rgba(55, 53, 47, 0.08);

  /* ========================================================================
     LAYOUT DIMENSIONS
     ======================================================================== */
  --sidebar-width: 14rem;           /* 224px */
  --panel-sidebar-width: 18rem;     /* 288px */
  --content-max-width: 64rem;       /* 1024px */
  --content-narrow-width: 32rem;    /* 512px */
  --modal-width: 28rem;             /* 448px */
  --modal-sm-width: 24rem;          /* 384px */
  --modal-lg-width: 36rem;          /* 576px */

  /* ========================================================================
     CHARTS (warm harmonized palette)
     ======================================================================== */
  --chart-1: #C17D3F;
  --chart-2: #4EA04E;
  --chart-3: #B8892E;
  --chart-4: #B85C5C;
  --chart-5: #5C8EB8;
}
```

**Step 2: Run build to verify no syntax errors**

Run: `cd apps/ui && npx vite build 2>&1 | head -20`
Expected: Build starts without CSS parse errors

**Step 3: Commit**

```bash
git add apps/ui/src/styles/theme.css
git commit -m "style: overhaul light mode to Warm Sand palette"
```

---

### Task 2: Overhaul theme.css — Dark Mode Colors

**Files:**
- Modify: `apps/ui/src/styles/theme.css:164-229` (`.dark` block)

**Step 1: Replace the `.dark { ... }` block with new dark mode tokens**

Replace lines 164-229 with:

```css
.dark {
  --background: #191919;
  --foreground: #E8E4DF;

  --card: #201F1D;
  --card-foreground: #E8E4DF;

  --popover: #252422;
  --popover-foreground: #E8E4DF;

  --primary: #D4945A;
  --primary-hover: #E0A46C;
  --primary-foreground: #1A1714;
  --primary-muted: rgba(212, 148, 90, 0.12);

  --secondary: #252422;
  --secondary-hover: #2E2D2A;
  --secondary-foreground: #E8E4DF;

  --muted: #252422;
  --muted-foreground: #9B9689;

  --accent: rgba(212, 148, 90, 0.08);
  --accent-foreground: #D4945A;

  --selection: rgba(212, 148, 90, 0.15);
  --selection-border: #D4945A;

  --destructive: #E5534B;
  --destructive-hover: #F06560;
  --destructive-foreground: #FFFFFF;
  --destructive-muted: rgba(229, 83, 75, 0.12);

  --success: #6BC46D;
  --success-hover: #7DD47F;
  --success-foreground: #FFFFFF;
  --success-muted: rgba(107, 196, 109, 0.12);

  --warning: #D4A04A;
  --warning-hover: #E0B05C;
  --warning-foreground: #FFFFFF;
  --warning-muted: rgba(212, 160, 74, 0.12);

  --border: #2E2E2B;
  --input: #2E2E2B;
  --input-background: #1E1D1B;

  --ring: #D4945A;
  --ring-muted: rgba(212, 148, 90, 0.25);

  --overlay: rgba(0, 0, 0, 0.6);

  --sidebar: #161614;
  --sidebar-foreground: #E8E4DF;
  --sidebar-primary: #D4945A;
  --sidebar-primary-foreground: #1A1714;
  --sidebar-accent: #1E1D1B;
  --sidebar-accent-hover: rgba(30, 29, 27, 0.5);
  --sidebar-border: #2E2E2B;
  --sidebar-ring: #D4945A;
  --sidebar-muted: #807A70;

  /* Shadows: none for cards in dark mode, borders instead */
  --shadow-card: none;
  --shadow-card-hover: none;
  --shadow-modal: 0 4px 24px rgba(0, 0, 0, 0.4);
  --shadow-dropdown: 0 4px 16px rgba(0, 0, 0, 0.3);

  --chart-1: #D4945A;
  --chart-2: #6BC46D;
  --chart-3: #D4A04A;
  --chart-4: #C47070;
  --chart-5: #6BA3C4;
}
```

**Step 2: Update selection color and base styles**

In the `@layer base` section (~line 283-323), replace the `::selection` color:

```css
::selection {
  background-color: var(--primary);
  color: var(--primary-foreground);
}

::-moz-selection {
  background-color: var(--primary);
  color: var(--primary-foreground);
}
```

**Step 3: Run build**

Run: `cd apps/ui && npx vite build 2>&1 | head -20`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/ui/src/styles/theme.css
git commit -m "style: overhaul dark mode to Warm Sand palette with border-based cards"
```

---

### Task 3: Update components.css — Sidebar & Navigation

**Files:**
- Modify: `apps/ui/src/styles/components.css:62-223` (Sidebar section)

**Step 1: Update sidebar styles for unified look**

Add right border to `.sidebar`:

```css
.sidebar {
  width: var(--sidebar-width);
  background-color: var(--sidebar);
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--sidebar-border);
}
```

**Step 2: Update `.nav-link--active` to use subtle highlight instead of solid fill**

Replace:
```css
.nav-link--active {
  color: white;
  background-color: var(--sidebar-primary);
}
```

With:
```css
.nav-link--active {
  color: var(--sidebar-primary);
  background-color: var(--primary-muted);
  border-left: 2px solid var(--sidebar-primary);
}
```

**Step 3: Remove opacity tricks from nav icons — use color instead**

Replace:
```css
.nav-link__icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  opacity: 0.7;
}

.nav-link:hover .nav-link__icon {
  opacity: 1;
}

.nav-link--active .nav-link__icon {
  opacity: 1;
}
```

With:
```css
.nav-link__icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  color: inherit;
}
```

**Step 4: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "style: unified sidebar with amber active state and left accent bar"
```

---

### Task 4: Update components.css — Cards & Interactive Hover

**Files:**
- Modify: `apps/ui/src/styles/components.css:287-425` (Cards section)

**Step 1: Add dark mode border to base card**

After the `.card` rule, add:

```css
:is(.dark) .card {
  box-shadow: none;
  border: 1px solid var(--border);
}
```

**Step 2: Replace bouncy `translateY` hover with border color change**

Replace:
```css
.card--interactive {
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.card--interactive:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-card-hover);
}

.card--interactive:active {
  transform: translateY(0);
}
```

With:
```css
.card--interactive {
  cursor: pointer;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.card--interactive:hover {
  border-color: var(--primary);
  box-shadow: var(--shadow-card-hover);
}
```

**Step 3: Fix `.list-item--active` hardcoded colors**

Replace:
```css
.list-item--active {
  background-color: #f3e8ff;
  border-left: 2px solid #9333ea;
}
```

With:
```css
.list-item--active {
  background-color: var(--primary-muted);
  border-left: 2px solid var(--primary);
}
```

**Step 4: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "style: border-based dark cards, subtle hover, fix hardcoded list colors"
```

---

### Task 5: Fix ConnectionStatus.tsx — Replace Hardcoded Tailwind

**Files:**
- Modify: `apps/ui/src/components/ConnectionStatus.tsx`
- Modify: `apps/ui/src/styles/components.css` (add new classes)

**Step 1: Add connection-status variant classes to components.css**

Add after the existing `.connection-status__text` rule (~line 191):

```css
.connection-status--connecting {
  background-color: var(--warning-muted);
  color: var(--warning);
}

.connection-status--disconnected {
  background-color: var(--muted);
  color: var(--muted-foreground);
}

.connection-status--failed {
  background-color: var(--destructive-muted);
  color: var(--destructive);
}

.connection-status__action {
  text-decoration: underline;
  margin-left: var(--spacing-1);
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  font-size: inherit;
}

.connection-status__action:hover {
  opacity: 0.8;
}

.connection-status__spinner {
  width: 16px;
  height: 16px;
  animation: spin 1s linear infinite;
}
```

**Step 2: Replace ConnectionStatus.tsx with CSS-class-based version**

Replace the entire file content:

```tsx
/**
 * Connection status indicator component.
 *
 * Shows the current WebSocket connection state and provides
 * a reconnect button when disconnected.
 */

import { WifiOff, Loader2, AlertCircle } from "lucide-react";
import { useConnectionState } from "@/hooks/useConnectionState";
import { getTransport } from "@/services/transport";

export function ConnectionStatus() {
  const state = useConnectionState();

  const handleReconnect = async () => {
    const transport = await getTransport();
    transport.reconnect();
  };

  switch (state.status) {
    case "connected":
      return null;

    case "connecting":
      return (
        <div className="connection-status connection-status--connecting">
          <Loader2 className="connection-status__spinner" />
          <span className="connection-status__text">Connecting...</span>
        </div>
      );

    case "reconnecting":
      return (
        <div className="connection-status connection-status--connecting">
          <Loader2 className="connection-status__spinner" />
          <span className="connection-status__text">
            Reconnecting ({state.attempt}/{state.maxAttempts})...
          </span>
        </div>
      );

    case "disconnected":
      return (
        <div className="connection-status connection-status--disconnected">
          <WifiOff className="connection-status__spinner" style={{ animation: 'none' }} />
          <span className="connection-status__text">Disconnected</span>
          <button onClick={handleReconnect} className="connection-status__action">
            Reconnect
          </button>
        </div>
      );

    case "failed":
      return (
        <div className="connection-status connection-status--failed">
          <AlertCircle className="connection-status__spinner" style={{ animation: 'none' }} />
          <span className="connection-status__text">Connection failed</span>
          <button onClick={handleReconnect} className="connection-status__action">
            Retry
          </button>
        </div>
      );
  }
}
```

**Step 3: Commit**

```bash
git add apps/ui/src/components/ConnectionStatus.tsx apps/ui/src/styles/components.css
git commit -m "style: replace hardcoded Tailwind colors in ConnectionStatus with CSS classes"
```

---

### Task 6: Fix GenerativeCanvas.tsx — Replace Hardcoded Dark Theme

**Files:**
- Modify: `apps/ui/src/features/agent/GenerativeCanvas.tsx`
- Modify: `apps/ui/src/styles/components.css` (add canvas classes)

**Step 1: Add generative canvas classes to components.css**

Add at the end of components.css:

```css
/* ============================================================================
   GENERATIVE CANVAS
   ============================================================================ */

.canvas-backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}

.canvas-backdrop__overlay {
  position: absolute;
  inset: 0;
  background-color: var(--overlay);
  backdrop-filter: blur(4px);
}

.canvas-panel {
  position: relative;
  width: 100%;
  max-width: 56rem;
  max-height: 85vh;
  background-color: var(--card);
  border-top: 1px solid var(--border);
  border-radius: var(--radius-xl) var(--radius-xl) 0 0;
  box-shadow: var(--shadow-modal);
  display: flex;
  flex-direction: column;
}

.canvas-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.canvas-panel__header-info {
  display: flex;
  align-items: center;
  gap: var(--spacing-2-5);
}

.canvas-panel__icon {
  padding: var(--spacing-1-5);
  border-radius: var(--radius-md);
  background-color: var(--primary-muted);
  color: var(--primary);
}

.canvas-panel__icon--input {
  background-color: var(--success-muted);
  color: var(--success);
}

.canvas-panel__title {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--foreground);
}

.canvas-panel__subtitle {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  text-transform: capitalize;
}

.canvas-panel__body {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-4);
}

.canvas-panel__close {
  color: var(--muted-foreground);
  height: 32px;
  width: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.canvas-panel__close:hover {
  color: var(--foreground);
  background-color: var(--muted);
}

.canvas-content-viewer {
  background-color: var(--muted);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  min-height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.canvas-content-viewer--html {
  background-color: #FFFFFF;
  overflow: hidden;
  padding: 0;
}

.canvas-content-viewer--image {
  background-color: var(--muted);
}

.canvas-content-viewer__text {
  white-space: pre-wrap;
  color: var(--foreground);
  font-size: var(--text-sm);
  font-family: var(--font-mono);
}

.canvas-form__description {
  font-size: var(--text-sm);
  color: var(--muted-foreground);
}

.canvas-form__field {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1-5);
}

.canvas-form__label {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--foreground);
}

.canvas-form__required {
  color: var(--destructive);
}

.canvas-form__hint {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.canvas-form__error {
  font-size: var(--text-xs);
  color: var(--destructive);
}

.canvas-form__actions {
  display: flex;
  gap: var(--spacing-2);
  padding-top: var(--spacing-2);
}
```

**Step 2: Rewrite GenerativeCanvas.tsx using CSS classes**

Replace the entire file with the version using CSS classes instead of hardcoded Tailwind. All `bg-gray-800`, `border-gray-700`, `text-white`, `from-violet-500`, etc. become the new CSS classes.

The key replacements in the JSX:
- `bg-gradient-to-br from-gray-900 to-gray-950 border-t border-gray-700` → `canvas-panel`
- `bg-gradient-to-br from-violet-500 to-pink-600` → `canvas-panel__icon`
- `text-sm font-medium text-white` → `canvas-panel__title`
- `text-xs text-gray-500` → `canvas-panel__subtitle`
- `bg-gray-800 border border-gray-700 ... text-white` → `form-input`
- `bg-gradient-to-r from-violet-600 to-blue-600` → `btn btn--primary`
- `border border-gray-600 text-white` → `btn btn--secondary`

**Step 3: Commit**

```bash
git add apps/ui/src/features/agent/GenerativeCanvas.tsx apps/ui/src/styles/components.css
git commit -m "style: replace hardcoded colors in GenerativeCanvas with CSS classes"
```

---

### Task 7: Fix App.tsx — Extract Inline Styles & Fix Toaster

**Files:**
- Modify: `apps/ui/src/App.tsx`
- Modify: `apps/ui/src/styles/components.css` (add settings classes)

**Step 1: Add settings-specific classes to components.css**

Add at end of components.css:

```css
/* ============================================================================
   SETTINGS PAGE
   ============================================================================ */

.settings-info-card {
  padding: var(--spacing-3);
  flex-direction: column;
  align-items: flex-start;
  background-color: var(--muted);
  border-radius: var(--radius-md);
}

.settings-info-card__label {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--muted-foreground);
}

.settings-info-card__value {
  font-family: var(--font-mono);
  margin-top: var(--spacing-1);
  color: var(--foreground);
}

.settings-section-header {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--foreground);
}

.settings-toggle-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
}

.settings-expandable {
  margin-top: var(--spacing-4);
  padding-top: var(--spacing-4);
  border-top: 1px solid var(--border);
}

.settings-field-label {
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--muted-foreground);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  display: block;
  margin-bottom: var(--spacing-2);
}

.settings-toggle-option {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: var(--muted);
  border-radius: var(--radius-md);
  cursor: pointer;
}

.settings-toggle-option--active {
  background-color: var(--primary-muted);
  border: 1px solid var(--primary);
}

.settings-toggle-option__title {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--foreground);
}

.settings-toggle-option__description {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.settings-hint {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: var(--spacing-1);
}

.settings-chevron {
  width: 20px;
  height: 20px;
  color: var(--muted-foreground);
}
```

**Step 2: Replace inline styles in App.tsx Settings panel**

Replace all `style={{ ... }}` occurrences in the `WebSettingsPanel` component with the new CSS classes. Key replacements:

- `style={{ maxWidth: '28rem', textAlign: 'center' }}` → use `page-container--narrow text-center`
- `style={{ backgroundColor: 'var(--destructive-muted)' }}` → use `card__icon--destructive`
- `style={{ fontSize: 'var(--text-base)', fontWeight: 600 }}` → use `settings-section-header`
- `style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}` → use `settings-toggle-btn`
- Badge with inline padding → use `settings-info-card`
- Expandable section dividers → use `settings-expandable`
- Tool group labels → use `settings-field-label`
- Checkbox labels → use `settings-toggle-option`

**Step 3: Fix Toaster to respect dark mode**

In the `<Toaster>` component, change `theme="light"` to `theme="system"`:

```tsx
<Toaster
  position="bottom-right"
  theme="system"
  toastOptions={{
    style: {
      fontWeight: 500,
      fontSize: '14px',
      borderRadius: 'var(--radius-lg)',
      boxShadow: 'var(--shadow-dropdown)',
    },
  }}
/>
```

**Step 4: Run build**

Run: `cd apps/ui && npx vite build 2>&1 | head -20`
Expected: Successful build

**Step 5: Commit**

```bash
git add apps/ui/src/App.tsx apps/ui/src/styles/components.css
git commit -m "style: extract inline styles from Settings panel, fix Toaster theme"
```

---

### Task 8: Visual Verification & Polish

**Files:**
- Possibly: `apps/ui/src/styles/theme.css`, `apps/ui/src/styles/components.css`

**Step 1: Start the dev server**

Run: `cd apps/ui && npm run dev`

**Step 2: Visual check dark mode**

Open http://localhost:3000 in browser. Verify:
- [ ] Background is warm charcoal (#191919), not pure black
- [ ] Cards have subtle borders, no shadows
- [ ] Sidebar is slightly darker, with right border
- [ ] Active nav link has amber text + left accent bar
- [ ] Primary buttons are amber/copper
- [ ] Text selection is amber, not purple
- [ ] Chat slider backdrop is correct overlay color

**Step 3: Visual check light mode**

Toggle to light mode. Verify:
- [ ] Background is warm off-white (#F7F5F2)
- [ ] Cards have warm shadows
- [ ] Sidebar is warm cream (#EFECE7)
- [ ] All semantic colors (success/warning/destructive) work

**Step 4: Fix any issues found**

Adjust colors, spacing, or borders as needed.

**Step 5: Final build check**

Run: `cd apps/ui && npm run build`
Expected: Clean build, no errors

**Step 6: Commit**

```bash
git add -A apps/ui/src/styles/
git commit -m "style: polish and visual adjustments for Warm Sand theme"
```

---

## Task Summary

| Task | What | Files | Estimated Effort |
|------|------|-------|-----------------|
| 1 | Light mode color tokens | theme.css | Small |
| 2 | Dark mode color tokens + selection | theme.css | Small |
| 3 | Sidebar & navigation CSS | components.css | Small |
| 4 | Cards & interactive hover CSS | components.css | Small |
| 5 | ConnectionStatus hardcoded colors | ConnectionStatus.tsx, components.css | Small |
| 6 | GenerativeCanvas hardcoded colors | GenerativeCanvas.tsx, components.css | Medium |
| 7 | App.tsx inline styles + Toaster | App.tsx, components.css | Medium |
| 8 | Visual verification & polish | theme.css, components.css | Small |

**Total: 8 tasks, all CSS/style focused. No structural code changes.**
