# UI Revamp Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate 7 scattered UI pages into 3 unified pages (Settings, Agents, Integrations) with a warm editorial + command center design system.

**Architecture:** Phase-based implementation starting with design tokens and shared components, then building each page independently. Pages are self-contained — Settings, Agents, and Integrations can be built in parallel after the foundation is in place. Final phase wires up navigation and route redirects.

**Tech Stack:** React 19, TypeScript, CSS custom properties (BEM), Vite, Radix UI primitives, Lucide icons, React Router

**Spec:** `docs/superpowers/specs/2026-03-28-ui-revamp-settings-agents-integrations-design.md`

**Mockups:** `.superpowers/brainstorm/` — `hybrid-ac.html`, `agents-page-mockup.html`, `integrations-page-mockup.html`

---

## Chunk 1: Foundation — Design Tokens & Shared Components

This chunk establishes the visual foundation. Everything else depends on it.

### Task 1: Update Font Loading

**Files:**
- Modify: `apps/ui/src/styles/fonts.css`

- [ ] **Step 1: Add Google Fonts import**

```css
/* apps/ui/src/styles/fonts.css */
@import url('https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,300;9..144,400;9..144,500;9..144,700&family=IBM+Plex+Sans:wght@300;400;500;600&family=JetBrains+Mono:wght@400;500&display=swap');
```

- [ ] **Step 2: Add fonts.css import to index.css entry point**

Check `apps/ui/src/styles/index.css`. If `fonts.css` is not already imported, add it before the theme import:

```css
@import './fonts.css';
@import 'tailwindcss';
@import './theme.css';
@import './components.css';
```

- [ ] **Step 3: Verify fonts load**

Open the app in browser, inspect an element, verify Fraunces/IBM Plex Sans/JetBrains Mono appear in computed font-family.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/styles/fonts.css apps/ui/src/styles/index.css
git commit -m "feat(ui): add Fraunces, IBM Plex Sans, JetBrains Mono fonts"
```

---

### Task 2: Replace Design Tokens in theme.css

**Files:**
- Modify: `apps/ui/src/styles/theme.css`

Read the current file first. The `:root` block (light theme) and `.dark` block need token values replaced per the spec's Token Migration Strategy.

- [ ] **Step 1: Update `:root` (light theme) color tokens**

Replace the color custom properties in `:root` with the new warm editorial palette. Key changes:

```css
:root {
  /* Backgrounds */
  --background: #f6f3ee;
  --background-surface: #ffffff;  /* was --card */
  --background-elevated: #f0ece6; /* was --popover */
  --sidebar: #1a1714;             /* DARK in both themes */

  /* Foreground hierarchy */
  --foreground: #1a1714;
  --muted-foreground: #8a8278;
  --subtle-foreground: #aaa39a;   /* NEW */
  --dim-foreground: #b5afa5;      /* NEW */

  /* Borders */
  --border: #e8e2d9;
  --border-hover: #c8956c;        /* NEW */

  /* Primary accent (copper) */
  --primary: #a07d52;
  --primary-foreground: #ffffff;
  --primary-hover: #8a6b45;
  --primary-muted: rgba(160,125,82,0.12);
  --primary-subtle: rgba(160,125,82,0.06); /* NEW */

  /* Semantic colors */
  --success: #3a8a3a;
  --success-muted: rgba(58,138,58,0.1);
  --warning: #9a7520;
  --warning-muted: rgba(154,117,32,0.1);
  --destructive: #cc4040;
  --destructive-muted: rgba(204,64,64,0.1);

  /* Additional palette */
  --blue: #4a8ac4;
  --blue-muted: rgba(74,138,196,0.1);
  --purple: #7a60b0;
  --purple-muted: rgba(122,96,176,0.1);
  --teal: #3a9a90;
  --teal-muted: rgba(58,154,144,0.1);

  /* Typography */
  --font-display: 'Fraunces', serif;
  --font-body: 'IBM Plex Sans', sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
  --font-sans: var(--font-body); /* backwards compat alias */
}
```

Keep existing spacing, radius, shadow, and layout tokens — those don't change.

Also keep `--card` as an alias: `--card: var(--background-surface);` for backwards compat with existing components not yet migrated.

- [ ] **Step 2: Update `.dark` block**

```css
.dark {
  --background: #141210;
  --background-surface: #1a1816;
  --background-elevated: #201e1b;
  --sidebar: #0d0c0a;

  --foreground: #f0ebe4;
  --muted-foreground: #777777;
  --subtle-foreground: #555555;
  --dim-foreground: #444444;

  --border: rgba(255,255,255,0.06);
  --border-hover: rgba(200,149,108,0.25);

  --primary: #c8956c;
  --primary-foreground: #0d0c0a;
  --primary-hover: #d4a57c;
  --primary-muted: rgba(200,149,108,0.12);
  --primary-subtle: rgba(200,149,108,0.06);

  --success: #4ea04e;
  --success-muted: rgba(78,160,78,0.1);
  --warning: #b8892e;
  --warning-muted: rgba(184,137,46,0.1);
  --destructive: #cc4040;
  --destructive-muted: rgba(204,64,64,0.1);

  --blue: #63b3ed;
  --blue-muted: rgba(99,179,237,0.1);
  --purple: #a882dc;
  --purple-muted: rgba(168,130,220,0.1);
  --teal: #4fd1c5;
  --teal-muted: rgba(79,209,197,0.1);

  --card: var(--background-surface);
}
```

- [ ] **Step 3: Update the Tailwind `@theme inline` block**

Map the new tokens into Tailwind's theme system. Add entries for the new tokens (`--font-display`, `--border-hover`, `--background-surface`, etc.) and alias `--font-sans` to `--font-body`.

- [ ] **Step 4: Visual check**

Open app in browser. Verify colors have shifted (warmer backgrounds, copper primary). Toggle dark/light mode. Sidebar should now be dark in BOTH themes.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/styles/theme.css
git commit -m "feat(ui): replace design tokens with warm editorial palette"
```

---

### Task 3: Update Sidebar to Stay Dark in Both Themes

**Files:**
- Modify: `apps/ui/src/styles/components.css` (sidebar section, ~lines 56-170)
- Modify: `apps/ui/src/App.tsx` (sidebar JSX, ~lines 250-326)

The sidebar currently uses `var(--sidebar)` for its background. With the token change, it's now dark in both themes. But we need to ensure all sidebar TEXT colors also work against a dark background in both modes.

- [ ] **Step 1: Add sidebar-specific color overrides in components.css**

After the `.sidebar` class definition, add scoped colors that override theme for sidebar children:

```css
/* Sidebar is always dark — override foreground colors for its children */
.sidebar {
  --sidebar-foreground: #f0ebe4;
  --sidebar-muted: #777777;
  --sidebar-dim: #444444;
  --sidebar-border: rgba(255,255,255,0.06);
  color: var(--sidebar-foreground);
}
```

Update `.nav-link` and other sidebar children to use `var(--sidebar-muted)` instead of `var(--muted-foreground)`.

- [ ] **Step 2: Verify sidebar looks correct in both light and dark modes**

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/styles/components.css apps/ui/src/App.tsx
git commit -m "feat(ui): make sidebar always-dark with scoped color tokens"
```

---

### Task 4: Create Shared Slideover Component

**Files:**
- Create: `apps/ui/src/components/Slideover.tsx`

Extract a reusable slide-over shell. The existing `ProviderSlideover` has this pattern inline — we're extracting the shell.

- [ ] **Step 1: Create the component**

```tsx
// apps/ui/src/components/Slideover.tsx
import { useEffect, useCallback, type ReactNode } from "react";
import { X } from "lucide-react";

interface SlideoverProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  subtitle?: ReactNode;
  icon?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: string; // default "540px"
}

export function Slideover({ open, onClose, title, subtitle, icon, children, footer, width = "540px" }: SlideoverProps) {
  const handleEscape = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") onClose();
  }, [onClose]);

  useEffect(() => {
    if (open) {
      document.addEventListener("keydown", handleEscape);
      document.body.style.overflow = "hidden";
    }
    return () => {
      document.removeEventListener("keydown", handleEscape);
      document.body.style.overflow = "";
    };
  }, [open, handleEscape]);

  return (
    <>
      <div
        className={`slideover-backdrop ${open ? "slideover-backdrop--open" : ""}`}
        onClick={onClose}
        aria-hidden="true"
      />
      <div
        className={`slideover ${open ? "slideover--open" : ""}`}
        style={{ width }}
        role="dialog"
        aria-modal="true"
      >
        <div className="slideover__header">
          <div className="slideover__header-left">
            {icon && <div className="slideover__icon">{icon}</div>}
            <div>
              <h2 className="slideover__title">{title}</h2>
              {subtitle && <div className="slideover__subtitle">{subtitle}</div>}
            </div>
          </div>
          <button className="slideover__close" onClick={onClose} aria-label="Close">
            <X style={{ width: 18, height: 18 }} />
          </button>
        </div>
        <div className="slideover__body">{children}</div>
        {footer && <div className="slideover__footer">{footer}</div>}
      </div>
    </>
  );
}
```

- [ ] **Step 2: Add slideover CSS classes to components.css**

Append to `apps/ui/src/styles/components.css`:

```css
/* ═══════════════════════════════════════════════════════════
   Slideover
   ═══════════════════════════════════════════════════════════ */

.slideover-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 50;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.3s;
}

.slideover-backdrop--open {
  opacity: 1;
  pointer-events: auto;
}

.slideover {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  background: var(--background);
  border-left: 1px solid var(--border);
  z-index: 51;
  display: flex;
  flex-direction: column;
  transform: translateX(100%);
  transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: -20px 0 60px rgba(0, 0, 0, 0.3);
}

.slideover--open {
  transform: translateX(0);
}

.slideover__header {
  padding: var(--spacing-6) var(--spacing-7);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.slideover__header-left {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.slideover__icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-base);
}

.slideover__title {
  font-family: var(--font-display);
  font-size: var(--text-xl);
  font-weight: 500;
  color: var(--foreground);
}

.slideover__subtitle {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: var(--spacing-1);
}

.slideover__close {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.slideover__close:hover {
  color: var(--foreground);
  border-color: var(--border-hover);
}

.slideover__body {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-6) var(--spacing-7);
}

.slideover__footer {
  padding: var(--spacing-4) var(--spacing-7);
  border-top: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--spacing-2);
  flex-shrink: 0;
  background: rgba(255, 255, 255, 0.01);
}

.slideover__section {
  margin-bottom: var(--spacing-7);
}

.slideover__section-title {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 1.5px;
  color: var(--dim-foreground);
  font-weight: 600;
  margin-bottom: var(--spacing-3);
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.slideover__section-title .line {
  flex: 1;
  height: 1px;
  background: var(--border);
}
```

- [ ] **Step 3: Verify component renders**

Import and render in a test page with `open={true}` to verify slide-over appears and closes.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/components/Slideover.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add shared Slideover component with CSS classes"
```

---

### Task 5: Create Shared HelpBox Component

**Files:**
- Create: `apps/ui/src/components/HelpBox.tsx`

- [ ] **Step 1: Create the component**

```tsx
// apps/ui/src/components/HelpBox.tsx
import type { ReactNode } from "react";
import { HelpCircle } from "lucide-react";

interface HelpBoxProps {
  children: ReactNode;
  icon?: ReactNode;
}

export function HelpBox({ children, icon }: HelpBoxProps) {
  return (
    <div className="help-box">
      <div className="help-box__icon">
        {icon || <HelpCircle style={{ width: 16, height: 16 }} />}
      </div>
      <div className="help-box__content">{children}</div>
    </div>
  );
}
```

- [ ] **Step 2: Add help-box CSS to components.css**

```css
/* ═══════════════════════════════════════════════════════════
   Help Box
   ═══════════════════════════════════════════════════════════ */

.help-box {
  background: var(--primary-subtle);
  border: 1px solid rgba(200, 149, 108, 0.1);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4) var(--spacing-5);
  display: flex;
  gap: var(--spacing-3);
  align-items: flex-start;
}

.help-box__icon {
  background: var(--primary-muted);
  color: var(--primary);
  width: 30px;
  height: 30px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.help-box__content {
  font-size: var(--text-sm);
  color: var(--muted-foreground);
  line-height: 1.7;
  font-weight: 300;
}

.help-box__content strong {
  color: var(--primary);
  font-weight: 500;
}

.help-box__content a {
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.help-box__content code {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  background: rgba(255, 255, 255, 0.06);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  color: var(--primary);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/components/HelpBox.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add shared HelpBox component"
```

---

### Task 6: Create Shared TabBar Component

**Files:**
- Create: `apps/ui/src/components/TabBar.tsx`

- [ ] **Step 1: Create the component**

```tsx
// apps/ui/src/components/TabBar.tsx
interface Tab {
  id: string;
  label: string;
  count?: number;
}

interface TabBarProps {
  tabs: Tab[];
  activeTab: string;
  onTabChange: (tabId: string) => void;
}

export function TabBar({ tabs, activeTab, onTabChange }: TabBarProps) {
  return (
    <div className="tab-bar" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`tab-bar__tab ${activeTab === tab.id ? "tab-bar__tab--active" : ""}`}
          onClick={() => onTabChange(tab.id)}
          role="tab"
          aria-selected={activeTab === tab.id}
        >
          {tab.label}
          {tab.count !== undefined && (
            <span className="tab-bar__count">{tab.count}</span>
          )}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Add tab-bar CSS to components.css**

```css
/* ═══════════════════════════════════════════════════════════
   Tab Bar
   ═══════════════════════════════════════════════════════════ */

.tab-bar {
  display: flex;
  gap: 2px;
  padding: 0 var(--spacing-9);
  border-bottom: 1px solid var(--border);
}

.tab-bar__tab {
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--text-sm);
  font-family: var(--font-body);
  color: var(--subtle-foreground);
  border: none;
  background: none;
  border-bottom: 2px solid transparent;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.tab-bar__tab:hover {
  color: var(--muted-foreground);
}

.tab-bar__tab--active {
  color: var(--primary);
  border-bottom-color: var(--primary);
}

.tab-bar__count {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.04);
  padding: 1px 6px;
  border-radius: var(--radius-md);
  margin-left: var(--spacing-1);
  color: var(--subtle-foreground);
}

.tab-bar__tab--active .tab-bar__count {
  background: var(--primary-muted);
  color: var(--primary);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/components/TabBar.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add shared TabBar component"
```

---

### Task 7: Create Shared ActionBar Component

**Files:**
- Create: `apps/ui/src/components/ActionBar.tsx`

- [ ] **Step 1: Create the component**

```tsx
// apps/ui/src/components/ActionBar.tsx
import { Search } from "lucide-react";
import type { ReactNode } from "react";

interface ActionBarProps {
  searchPlaceholder?: string;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  filters?: ReactNode;
  actions?: ReactNode;
}

export function ActionBar({ searchPlaceholder, searchValue, onSearchChange, filters, actions }: ActionBarProps) {
  return (
    <div className="action-bar">
      <div className="action-bar__left">
        {onSearchChange && (
          <div className="action-bar__search">
            <Search style={{ width: 14, height: 14 }} className="action-bar__search-icon" />
            <input
              className="action-bar__search-input"
              placeholder={searchPlaceholder || "Search..."}
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
            />
          </div>
        )}
        {filters}
      </div>
      {actions && <div className="action-bar__right">{actions}</div>}
    </div>
  );
}

// Reusable filter chip for action bars
interface FilterChipProps {
  label: string;
  active?: boolean;
  onClick: () => void;
}

export function FilterChip({ label, active, onClick }: FilterChipProps) {
  return (
    <button
      className={`filter-chip ${active ? "filter-chip--active" : ""}`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}
```

- [ ] **Step 2: Add action-bar CSS to components.css**

```css
/* ═══════════════════════════════════════════════════════════
   Action Bar
   ═══════════════════════════════════════════════════════════ */

.action-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-4) var(--spacing-9);
  border-bottom: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.01);
}

.action-bar__left {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.action-bar__right {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.action-bar__search {
  position: relative;
}

.action-bar__search-icon {
  position: absolute;
  left: 10px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--subtle-foreground);
  pointer-events: none;
}

.action-bar__search-input {
  background: var(--background-surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 7px 12px 7px 32px;
  font-size: var(--text-sm);
  color: var(--foreground);
  font-family: var(--font-body);
  width: 220px;
  outline: none;
  transition: border-color 0.15s;
}

.action-bar__search-input::placeholder {
  color: var(--dim-foreground);
}

.action-bar__search-input:focus {
  border-color: var(--primary);
}

.filter-chip {
  font-size: var(--text-xs);
  padding: 5px 12px;
  border-radius: var(--radius-sm);
  background: var(--background-surface);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  transition: all 0.15s;
  font-weight: 500;
  font-family: var(--font-body);
}

.filter-chip:hover {
  border-color: var(--border-hover);
  color: var(--primary);
}

.filter-chip--active {
  background: var(--primary-muted);
  border-color: var(--border-hover);
  color: var(--primary);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/components/ActionBar.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add shared ActionBar and FilterChip components"
```

---

### Task 8: Create Shared MetaChip Component

**Files:**
- Create: `apps/ui/src/components/MetaChip.tsx`

- [ ] **Step 1: Create the component**

```tsx
// apps/ui/src/components/MetaChip.tsx
import type { ReactNode } from "react";

type ChipVariant = "model" | "skills" | "mcps" | "schedule" | "tools" | "stdio" | "http" | "sse" | "plugin" | "worker" | "enabled" | "disabled" | "running" | "error";

interface MetaChipProps {
  variant: ChipVariant;
  icon?: ReactNode;
  children: ReactNode;
}

export function MetaChip({ variant, icon, children }: MetaChipProps) {
  return (
    <span className={`meta-chip meta-chip--${variant}`}>
      {icon && <span className="meta-chip__icon">{icon}</span>}
      {children}
    </span>
  );
}
```

- [ ] **Step 2: Add meta-chip CSS to components.css**

```css
/* ═══════════════════════════════════════════════════════════
   Meta Chips
   ═══════════════════════════════════════════════════════════ */

.meta-chip {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-weight: 500;
}

.meta-chip__icon {
  font-size: 12px;
}

.meta-chip--model {
  background: var(--primary-subtle);
  border: 1px solid rgba(200, 149, 108, 0.1);
  color: var(--primary);
  font-family: var(--font-mono);
  font-weight: 400;
}

.meta-chip--skills {
  background: var(--purple-muted);
  border: 1px solid rgba(168, 130, 220, 0.12);
  color: var(--purple);
}

.meta-chip--mcps {
  background: var(--blue-muted);
  border: 1px solid rgba(99, 179, 237, 0.12);
  color: var(--blue);
}

.meta-chip--schedule {
  background: var(--success-muted);
  border: 1px solid rgba(78, 160, 78, 0.12);
  color: var(--success);
}

.meta-chip--tools {
  background: var(--primary-subtle);
  border: 1px solid rgba(200, 149, 108, 0.1);
  color: var(--primary);
}

.meta-chip--stdio {
  background: var(--primary-muted);
  color: var(--primary);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 10px;
}

.meta-chip--http {
  background: var(--blue-muted);
  color: var(--blue);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 10px;
}

.meta-chip--sse {
  background: var(--teal-muted);
  color: var(--teal);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 10px;
}

.meta-chip--plugin {
  background: var(--success-muted);
  color: var(--success);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 10px;
}

.meta-chip--worker {
  background: var(--blue-muted);
  color: var(--blue);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 10px;
}

.meta-chip--enabled {
  background: var(--success-muted);
  border: 1px solid rgba(78, 160, 78, 0.12);
  color: var(--success);
}

.meta-chip--disabled {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
  color: var(--dim-foreground);
}

.meta-chip--running {
  background: var(--success-muted);
  border: 1px solid rgba(78, 160, 78, 0.12);
  color: var(--success);
}

.meta-chip--error {
  background: var(--destructive-muted);
  border: 1px solid rgba(204, 64, 64, 0.12);
  color: var(--destructive);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/components/MetaChip.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add shared MetaChip component with all variants"
```

---

### Task 9: Enhance EmptyState Component

**Files:**
- Modify: `apps/ui/src/shared/ui/EmptyState.tsx`

The existing component has `icon`, `title`, `description`, `action`, `size` props. We need to add a `hint` prop for install/setup hints with code.

- [ ] **Step 1: Read existing component and add `hint` prop**

Add an optional `hint` prop (ReactNode) that renders below the action button in a code-styled hint box.

- [ ] **Step 2: Add CSS for `.empty-state__hint`**

```css
.empty-state__hint {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-xs);
  color: var(--dim-foreground);
  background: var(--background-surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: var(--spacing-2) var(--spacing-4);
  margin-top: var(--spacing-3);
}

.empty-state__hint code {
  font-family: var(--font-mono);
  color: var(--primary);
  font-size: 11px;
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/shared/ui/EmptyState.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): enhance EmptyState with hint prop"
```

---

### Task 10: Add Page-Level Layout CSS Classes

**Files:**
- Modify: `apps/ui/src/styles/components.css`

Add the shared page layout classes that all three pages use.

- [ ] **Step 1: Add page layout classes**

```css
/* ═══════════════════════════════════════════════════════════
   Page Layout (revamped)
   ═══════════════════════════════════════════════════════════ */

.page-header-v2 {
  padding: var(--spacing-7) var(--spacing-9) 0;
}

.page-title-v2 {
  font-family: var(--font-display);
  font-size: 26px;
  font-weight: 500;
  color: var(--foreground);
  letter-spacing: -0.3px;
  margin-bottom: var(--spacing-1);
}

.page-subtitle-v2 {
  font-size: var(--text-sm);
  color: var(--muted-foreground);
  font-weight: 300;
  margin-bottom: var(--spacing-5);
  line-height: 1.7;
  max-width: 560px;
}

.page-content-v2 {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-6) var(--spacing-9) var(--spacing-10);
}

/* Card grids */
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: var(--spacing-4);
}

.card-grid--2col {
  grid-template-columns: repeat(2, 1fr);
}

/* Card entrance animation */
@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}

.animate-fade-in-up {
  animation: fadeInUp 0.4s cubic-bezier(0.4, 0, 0.2, 1) both;
}

/* Stagger delays (applied via style prop or nth-child) */
.animate-delay-1 { animation-delay: 0.05s; }
.animate-delay-2 { animation-delay: 0.1s; }
.animate-delay-3 { animation-delay: 0.15s; }
.animate-delay-4 { animation-delay: 0.2s; }

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
  .animate-fade-in-up {
    animation: none;
    opacity: 1;
  }
}

/* Status dot pulse */
@keyframes statusPulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--success);
  animation: statusPulse 2s ease-in-out infinite;
}

.status-dot--idle {
  background: var(--dim-foreground);
  animation: none;
}

/* Info tooltip */
.info-tip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--primary-muted);
  color: var(--primary);
  font-size: 9px;
  font-weight: 700;
  cursor: help;
  margin-left: 4px;
  vertical-align: middle;
}

/* Add-link */
.add-link {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-1);
  margin-top: var(--spacing-4);
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--primary);
  cursor: pointer;
}

.add-link:hover {
  text-decoration: underline;
  text-underline-offset: 2px;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "feat(ui): add page layout v2, card-grid, animation classes"
```

---

### Task 11: Move ModelChip to Shared UI

**Files:**
- Move: `apps/ui/src/features/integrations/ModelChip.tsx` → `apps/ui/src/shared/ui/ModelChip.tsx`
- Modify: `apps/ui/src/shared/ui/index.ts`

- [ ] **Step 1: Copy `ModelChip.tsx` to `shared/ui/`**

Read the existing file, copy it to the new location. Update any relative imports if needed.

- [ ] **Step 2: Export from barrel**

Add `export { ModelChip } from "./ModelChip";` to `apps/ui/src/shared/ui/index.ts`.

- [ ] **Step 3: Update import in any file that references the old path**

Search for imports of `ModelChip` from the integrations path and update to `@/shared/ui/ModelChip`.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/shared/ui/ModelChip.tsx apps/ui/src/shared/ui/index.ts
git commit -m "refactor(ui): move ModelChip to shared/ui for reuse across pages"
```

---

## Chunk 2: Settings Page — Providers + General + Logging

### Task 12: Move Provider Sub-Components to Settings Feature

**Files:**
- Move: `apps/ui/src/features/integrations/ProviderCard.tsx` → `apps/ui/src/features/settings/ProviderCard.tsx`
- Move: `apps/ui/src/features/integrations/ProviderSlideover.tsx` → `apps/ui/src/features/settings/ProviderSlideover.tsx`
- Move: `apps/ui/src/features/integrations/ProvidersGrid.tsx` → `apps/ui/src/features/settings/ProvidersGrid.tsx`
- Move: `apps/ui/src/features/integrations/ProvidersEmptyState.tsx` → `apps/ui/src/features/settings/ProvidersEmptyState.tsx`
- Move: `apps/ui/src/features/integrations/providerPresets.ts` → `apps/ui/src/features/settings/providerPresets.ts`

- [ ] **Step 1: Copy all 5 files to `features/settings/`**
- [ ] **Step 2: Update internal imports** (relative paths between these files change since they're in the same directory)
- [ ] **Step 3: Update `ModelChip` imports** to point to `@/shared/ui/ModelChip`
- [ ] **Step 4: Verify `npm run build` passes** (no broken imports)
- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/settings/
git commit -m "refactor(ui): move provider components to settings feature"
```

---

### Task 13: Rewrite WebSettingsPanel with Tabbed Layout

**Files:**
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

This is a complete rewrite. The Settings page becomes the container with 3 tabs: Providers (default), General, Logging.

- [ ] **Step 1: Read the existing `WebSettingsPanel.tsx` fully** to understand current state management

- [ ] **Step 2: Rewrite the component**

Structure:
```tsx
export function WebSettingsPanel() {
  const [activeTab, setActiveTab] = useState("providers");

  return (
    <div className="page">
      <div className="page-header-v2">
        <h1 className="page-title-v2">Settings</h1>
        <p className="page-subtitle-v2">
          Configure your AI providers, system preferences, and logging.
          Start here if you're new — add a provider to get going.
        </p>
      </div>
      <TabBar
        tabs={[
          { id: "providers", label: "Providers", count: providers.length },
          { id: "general", label: "General" },
          { id: "logging", label: "Logging" },
        ]}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />
      <div className="page-content-v2">
        {activeTab === "providers" && <ProvidersTab />}
        {activeTab === "general" && <GeneralTab />}
        {activeTab === "logging" && <LoggingTab />}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Extract ProvidersTab** — wraps the existing `ProvidersGrid` / `ProvidersEmptyState` + `ProviderSlideover` + adds HelpBox at bottom
- [ ] **Step 4: Extract GeneralTab** — system info section + context protection (from existing settings code)
- [ ] **Step 5: Extract LoggingTab** — logging config (from existing settings code) + HelpBox
- [ ] **Step 6: Handle URL tab param** — read `?tab=` from URL, default to "providers"
- [ ] **Step 7: Verify all 3 tabs work** — providers show/create/edit/test, general saves, logging saves
- [ ] **Step 8: Commit**

```bash
git add apps/ui/src/features/settings/
git commit -m "feat(ui): rewrite Settings page with Providers/General/Logging tabs"
```

---

## Chunk 3: Agents Page — My Agents + Skills + Schedules

### Task 14: Rewrite WebAgentsPanel with Tabbed Layout

**Files:**
- Modify: `apps/ui/src/features/agent/WebAgentsPanel.tsx`

Complete rewrite to card grid + 3 tabs.

- [ ] **Step 1: Read existing `WebAgentsPanel.tsx` and `AgentEditPanel.tsx` fully**

- [ ] **Step 2: Create the page shell with tabs**

```tsx
export function WebAgentsPanel() {
  const [activeTab, setActiveTab] = useState("agents");
  // ... data fetching

  return (
    <div className="page">
      <div className="page-header-v2">
        <h1 className="page-title-v2">Agents</h1>
        <p className="page-subtitle-v2">
          Create and manage your AI assistants. Each agent has its own
          personality, model, skills, and tools.
        </p>
      </div>
      <TabBar tabs={[
        { id: "agents", label: "My Agents", count: agents.length },
        { id: "skills", label: "Skills Library", count: skills.length },
        { id: "schedules", label: "Schedules", count: jobs.length },
      ]} activeTab={activeTab} onTabChange={setActiveTab} />
      {/* ActionBar + content per tab */}
    </div>
  );
}
```

- [ ] **Step 3: Build AgentsTab** — card grid with agent cards (avatar, name, ID, description, meta chips, footer with hover actions). Click → opens AgentEditPanel slideover.
- [ ] **Step 4: Build agent card CSS classes** in `components.css`:

```css
.agent-card { /* card with hover lift, staggered animation */ }
.agent-card__top { /* avatar + name/id */ }
.agent-card__avatar { /* gradient icon, 44px */ }
.agent-card__avatar--online .online-dot { /* green dot */ }
.agent-card__desc { /* 2-line clamp */ }
.agent-card__meta { /* chip row */ }
.agent-card__footer { /* status + hover actions */ }
```

- [ ] **Step 5: Build SkillsTab** — card grid with skill cards (name, category badge, description). Click → slideover with full details. Create button.
- [ ] **Step 6: Build SchedulesTab** — list of schedule cards with name, cron description, agent selector, enabled toggle, last/next run. Create modal with agent selector (defaults to root).
- [ ] **Step 7: Handle URL tab param** — read `?tab=` from URL
- [ ] **Step 8: Verify all tabs** — agents CRUD, skills CRUD, schedules CRUD
- [ ] **Step 9: Commit**

```bash
git add apps/ui/src/features/agent/
git commit -m "feat(ui): rewrite Agents page with My Agents/Skills/Schedules tabs"
```

---

### Task 15: Redesign AgentEditPanel as Slideover

**Files:**
- Modify: `apps/ui/src/features/agent/AgentEditPanel.tsx`

Refactor to use the shared `Slideover` component. Add Skills toggle section and Schedules inline section.

- [ ] **Step 1: Read existing `AgentEditPanel.tsx`**
- [ ] **Step 2: Refactor to use `<Slideover>`** wrapper instead of inline slide-over markup
- [ ] **Step 3: Add Skills section** — toggle list with skill name + description
- [ ] **Step 4: Add Schedules section** — inline schedule display + "Add schedule" link
- [ ] **Step 5: Add section title styling** with `slideover__section-title` + `.line` divider
- [ ] **Step 6: Verify edit/create flows** work for agents with skills and schedules
- [ ] **Step 7: Commit**

```bash
git add apps/ui/src/features/agent/AgentEditPanel.tsx
git commit -m "feat(ui): redesign AgentEditPanel with skills/schedules sections"
```

---

## Chunk 4: Integrations Page — Tool Servers + Plugins & Workers

### Task 16: Create New WebIntegrationsPanel

**Files:**
- Create: `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` (replaces old file)
- Create: `apps/ui/src/features/integrations/ToolServerCard.tsx`
- Create: `apps/ui/src/features/integrations/ToolServerSlideover.tsx`
- Create: `apps/ui/src/features/integrations/PluginWorkerCard.tsx`

- [ ] **Step 1: Delete old `WebIntegrationsPanel.tsx`** (was the providers page)

- [ ] **Step 2: Create new `WebIntegrationsPanel.tsx`** — page shell with 2 tabs

```tsx
export function WebIntegrationsPanel() {
  const [activeTab, setActiveTab] = useState("tools");

  return (
    <div className="page">
      <div className="page-header-v2">
        <h1 className="page-title-v2">Integrations</h1>
        <p className="page-subtitle-v2">
          Connect z-Bot to external tools, services, and plugins.
          Give your agents new abilities.
        </p>
      </div>
      <TabBar tabs={[
        { id: "tools", label: "Tool Servers", count: mcps.length },
        { id: "plugins", label: "Plugins & Workers", count: workers.length },
      ]} activeTab={activeTab} onTabChange={setActiveTab} />
      {/* Per-tab ActionBar + content */}
    </div>
  );
}
```

- [ ] **Step 3: Create `ToolServerCard.tsx`** — card with icon, name, type badge (MetaChip), command/URL, description, tool count, enabled status, footer with test/edit
- [ ] **Step 4: Create `ToolServerSlideover.tsx`** — uses `<Slideover>`, shows test result, details, discovered tools, usage hint (HelpBox)
- [ ] **Step 5: Create `PluginWorkerCard.tsx`** — card with icon, name, source badge (plugin/worker), description, capabilities/resources counts, running status
- [ ] **Step 6: Add CSS classes** for tool server and plugin/worker cards in `components.css`
- [ ] **Step 7: Wire up data** — `listMcps()` for Tool Servers tab, `listBridgeWorkers()` with 5s polling for Plugins tab
- [ ] **Step 8: Verify both tabs** — MCP CRUD, worker display
- [ ] **Step 9: Commit**

```bash
git add apps/ui/src/features/integrations/
git commit -m "feat(ui): create unified Integrations page with Tool Servers and Plugins tabs"
```

---

## Chunk 5: Navigation, Routing & Cleanup

### Task 17: Restructure Sidebar Navigation

**Files:**
- Modify: `apps/ui/src/App.tsx`

- [ ] **Step 1: Update `navGroups` array**

```tsx
const navGroups: NavGroup[] = [
  {
    items: [
      { to: "/", label: "Dashboard", icon: LayoutDashboard },
      { to: "/logs", label: "Logs", icon: Eye },
      { to: "/memory", label: "Memory", icon: Brain },
    ],
  },
  {
    label: "Manage",
    items: [
      { to: "/agents", label: "Agents", icon: Bot },
      { to: "/integrations", label: "Integrations", icon: Plug },
    ],
  },
  {
    label: "System",
    items: [
      { to: "/settings", label: "Settings", icon: Settings },
    ],
  },
];
```

- [ ] **Step 2: Update `<Routes>`**

```tsx
<Routes>
  <Route path="/" element={<WebOpsDashboard />} />
  <Route path="/chat" element={<WebOpsDashboard />} />
  <Route path="/logs" element={<WebLogsPanel />} />
  <Route path="/memory" element={<WebMemoryPanel />} />
  <Route path="/agents" element={<WebAgentsPanel />} />
  <Route path="/integrations" element={<WebIntegrationsPanel />} />
  <Route path="/settings" element={<WebSettingsPanel />} />

  {/* Redirects from old routes */}
  <Route path="/providers" element={<Navigate to="/settings" replace />} />
  <Route path="/skills" element={<Navigate to="/agents?tab=skills" replace />} />
  <Route path="/hooks" element={<Navigate to="/agents?tab=schedules" replace />} />
  <Route path="/connectors" element={<Navigate to="/integrations?tab=plugins" replace />} />
  <Route path="/mcps" element={<Navigate to="/integrations" replace />} />
</Routes>
```

- [ ] **Step 3: Remove unused imports** (`WebSkillsPanel`, `WebCronPanel`, `WebConnectorsPanel`, `WebMcpsPanel`, `Cable`, `Server`, `Calendar`, `Zap`)
- [ ] **Step 4: Add `Navigate` import from react-router-dom**
- [ ] **Step 5: Update sidebar styling** — apply `--font-display` to logo, ensure sidebar-scoped colors work

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/App.tsx
git commit -m "feat(ui): restructure sidebar navigation and add route redirects"
```

---

### Task 18: Update Barrel Exports and Clean Up

**Files:**
- Modify: `apps/ui/src/features/index.ts`
- Delete: `apps/ui/src/features/skills/WebSkillsPanel.tsx` (if fully absorbed)
- Delete: `apps/ui/src/features/cron/WebCronPanel.tsx` (if fully absorbed)
- Delete: `apps/ui/src/features/connectors/WebConnectorsPanel.tsx` (if fully absorbed)
- Delete: `apps/ui/src/features/mcps/WebMcpsPanel.tsx` (if fully absorbed)

- [ ] **Step 1: Update `features/index.ts`**

```tsx
export { WebChatPanel } from "./agent/WebChatPanel";
export { WebAgentsPanel } from "./agent/WebAgentsPanel";
export { WebIntegrationsPanel } from "./integrations/WebIntegrationsPanel";
export { WebSettingsPanel } from "./settings/WebSettingsPanel";
```

Remove exports for `WebSkillsPanel`, `WebCronPanel`, `WebConnectorsPanel`, old `WebIntegrationsPanel`.

- [ ] **Step 2: Delete deprecated page files**

Only delete after confirming ALL functionality has been absorbed into the new pages. Check:
- Skills CRUD → Agents page Skills tab
- Cron CRUD → Agents page Schedules tab
- Workers display → Integrations Plugins tab
- MCPs CRUD → Integrations Tool Servers tab

- [ ] **Step 3: Remove old provider components from integrations/**

Delete the files that were moved to `features/settings/`:
- `features/integrations/ProviderCard.tsx`
- `features/integrations/ProviderSlideover.tsx`
- `features/integrations/ProvidersGrid.tsx`
- `features/integrations/ProvidersEmptyState.tsx`
- `features/integrations/providerPresets.ts`

- [ ] **Step 4: Run `npm run build`** — verify zero errors
- [ ] **Step 5: Manual smoke test** — navigate all pages, verify no dead links
- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore(ui): remove deprecated page files, update barrel exports"
```

---

### Task 19: Final Polish and Reduced Motion

**Files:**
- Modify: `apps/ui/src/styles/components.css`

- [ ] **Step 1: Add `prefers-reduced-motion` overrides**

```css
@media (prefers-reduced-motion: reduce) {
  .animate-fade-in-up,
  .slideover,
  .slideover-backdrop {
    animation: none !important;
    transition: none !important;
  }

  .status-dot {
    animation: none;
  }

  .slideover--open {
    transform: translateX(0);
  }
}
```

- [ ] **Step 2: Verify** — toggle reduced motion in OS settings, confirm no animations
- [ ] **Step 3: Final build check**

```bash
cd apps/ui && npm run build
```

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "feat(ui): add reduced motion support for accessibility"
```

---

## Parallelization Guide

```
Chunk 1 (Foundation)  ─── must complete first
    │
    ├── Chunk 2 (Settings)      ─── can run in parallel
    ├── Chunk 3 (Agents)        ─── can run in parallel
    └── Chunk 4 (Integrations)  ─── can run in parallel
                │
                └── Chunk 5 (Nav & Cleanup) ─── depends on all above
```

Tasks 1-11 must complete sequentially (they build on each other's CSS).
Tasks 12-13 (Settings), 14-15 (Agents), 16 (Integrations) can be parallelized.
Tasks 17-19 (Nav/Cleanup) must run last.
