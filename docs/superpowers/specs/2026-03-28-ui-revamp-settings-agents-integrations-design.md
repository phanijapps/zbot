# UI Revamp: Settings, Agents, Integrations

## Overview

Consolidate 7 scattered pages (Settings, Providers, MCPs, Skills, Agents, Workers, Schedules) into 3 unified pages with consistent design language, contextual help text, and a warm editorial + command center aesthetic.

**Target user**: Someone who has used ChatGPT/Claude but never configured their own AI system. They know chat but "MCP servers", "Skills", and "Bridge Workers" are jargon.

**Design direction**: Hybrid of "Warm Command Center" (dark, warm charcoal, copper accents) + "Warm Editorial" (Fraunces serif headings, IBM Plex Sans body, generous whitespace). Fully themeable via CSS custom properties. Light mode available as alternate theme.

## Information Architecture

### Before (7+ pages)

```
Configure: Agents, Skills
Connect:   Workers, Schedules, Providers, MCPs
System:    Settings
```

### After (3 pages)

```
Manage:  Agents (My Agents | Skills Library | Schedules)
         Integrations (Tool Servers | Plugins & Workers)
System:  Settings (Providers | General | Logging)
```

### Navigation Sidebar

Sidebar stays dark in both light/dark themes (anchor element). Restructured groups:

```
z-Bot v0.1

[Main]
  Dashboard
  Logs
  Memory

[Manage]
  Agents         → badge with agent count
  Integrations   → badge with total count

[System]
  Settings

[Footer]
  Status dot + "Connected"
  Theme toggle
```

### Routing Changes

| Old Route | New Route | Notes |
|-----------|-----------|-------|
| `/agents` | `/agents` | Default tab: My Agents |
| `/skills` | `/agents?tab=skills` | Now a tab within Agents |
| `/hooks` | `/agents?tab=schedules` | Now a tab within Agents |
| `/providers` | `/settings` | Default tab: Providers |
| `/settings` | `/settings?tab=general` | General tab |
| `/mcps` | `/integrations` | Default tab: Tool Servers |
| `/connectors` | `/integrations?tab=plugins` | Merged with plugins |

Old routes should redirect to new locations for bookmarks.

## Design System Updates

### Typography

| Token | Value |
|-------|-------|
| `--font-display` | `'Fraunces', serif` |
| `--font-body` | `'IBM Plex Sans', sans-serif` |
| `--font-mono` | `'JetBrains Mono', monospace` |

Font loading: Google Fonts import in `fonts.css`. Fraunces weights: 300, 400, 500, 700. IBM Plex Sans weights: 300, 400, 500, 600. JetBrains Mono weights: 400, 500.

### Color Tokens (Dark Theme — Default)

| Token | Value | Usage |
|-------|-------|-------|
| `--background` | `#141210` | Main content area |
| `--background-surface` | `#1a1816` | Cards, elevated surfaces |
| `--background-elevated` | `#201e1b` | Hover states, dropdowns |
| `--sidebar` | `#0d0c0a` | Sidebar background |
| `--border` | `rgba(255,255,255,0.06)` | Default borders |
| `--border-hover` | `rgba(200,149,108,0.25)` | Hover/focus borders |
| `--foreground` | `#f0ebe4` | Primary text |
| `--muted-foreground` | `#777` | Secondary text |
| `--subtle-foreground` | `#555` | Tertiary text |
| `--dim-foreground` | `#444` | Quaternary text |
| `--primary` | `#c8956c` | Primary accent (copper) |
| `--primary-muted` | `rgba(200,149,108,0.12)` | Primary accent bg |
| `--primary-subtle` | `rgba(200,149,108,0.06)` | Subtle accent bg |
| `--success` | `#4ea04e` | Success states |
| `--success-muted` | `rgba(78,160,78,0.1)` | Success bg |
| `--warning` | `#b8892e` | Warning states |
| `--warning-muted` | `rgba(184,137,46,0.1)` | Warning bg |
| `--destructive` | `#cc4040` | Error/destructive states |
| `--destructive-muted` | `rgba(204,64,64,0.1)` | Error bg |

### Color Tokens (Light Theme)

| Token | Value |
|-------|-------|
| `--background` | `#f6f3ee` |
| `--background-surface` | `#ffffff` |
| `--background-elevated` | `#f0ece6` |
| `--sidebar` | `#1a1714` (stays dark) |
| `--border` | `#e8e2d9` |
| `--border-hover` | `#c8956c` |
| `--foreground` | `#1a1714` |
| `--muted-foreground` | `#8a8278` |
| `--subtle-foreground` | `#aaa39a` |
| `--primary` | `#a07d52` |
| `--primary-muted` | `rgba(160,125,82,0.12)` |
| `--success` | `#3a8a3a` |
| `--destructive` | `#cc4040` |

### Component Patterns

All pages follow this consistent structure:

```
┌─────────────────────────────────────────┐
│ Page Title (Fraunces, 26px)             │
│ Subtitle (IBM Plex Sans, 13px, light)   │
├─────────────────────────────────────────┤
│ Tab1 (active) │ Tab2 │ Tab3            │
├─────────────────────────────────────────┤
│ [Search] [Filter chips]    [+ Action]   │
├─────────────────────────────────────────┤
│                                         │
│  ┌──────────┐  ┌──────────┐            │
│  │  Card 1  │  │  Card 2  │            │
│  └──────────┘  └──────────┘            │
│  ┌──────────┐                          │
│  │  Card 3  │                          │
│  └──────────┘                          │
│                                         │
│  ┌─ ? ────────────────────────────┐     │
│  │ Help box with contextual text  │     │
│  └────────────────────────────────┘     │
│                                         │
└─────────────────────────────────────────┘
```

### New CSS Component Classes

```
Page layout:
  .page-title          → Fraunces, 26px, --foreground
  .page-subtitle       → IBM Plex Sans, 13px, 300 weight, --muted-foreground
  .tab-bar             → Horizontal tab container
  .tab, .tab--active   → Tab items with active underline accent
  .tab__count          → Small count badge in tab
  .action-bar          → Search + filters + action button row

Cards:
  .agent-card          → Agent card with avatar, meta chips, footer
  .agent-card__avatar  → 44px gradient rounded icon
  .agent-card__meta    → Metadata chip row
  .ts-card             → Tool server card
  .pw-card             → Plugin/worker card
  .meta-chip           → Small metadata badge (model, skills, tools, schedule)
  .meta-chip--model/skills/mcps/schedule → Color variants
  .type-badge          → Protocol type badge (stdio/http/sse)
  .type-badge--stdio/http/sse → Color variants
  .source-badge        → Source type badge (plugin/worker)
  .source-badge--plugin/worker → Color variants

Help:
  .help-box            → Contextual help container
  .help-box__icon      → ? icon with accent background
  .info-tip            → Inline (i) tooltip icon

Slide-over:
  .slideover-backdrop  → Dark overlay
  .slideover           → Right-side panel (540px)
  .slideover__header   → Title + close button
  .slideover__body     → Scrollable content
  .slideover__section  → Section with uppercase label + line
  .slideover__footer   → Action buttons

Forms:
  .form-group          → Label + input wrapper
  .form-label          → Label with optional tooltip
  .form-input          → Text input (dark surface, border, accent focus)
  .form-textarea       → Textarea variant
  .form-select         → Select dropdown with custom chevron

Buttons:
  .btn-primary         → Copper accent, dark text
  .btn-ghost           → Transparent with border
  .btn-outline-success → Green outline
  .btn-destructive     → Red outline
  .icon-btn            → 28px square icon button

Empty states:
  .empty-state         → Centered empty content
  .empty-state__icon   → Large icon container
  .empty-state__title  → Fraunces heading
  .empty-state__desc   → Description text
  .empty-state__hint   → Install/setup hint with code

Lists:
  .skill-toggle        → Skill toggle row with switch
  .schedule-inline     → Inline schedule display
  .tool-item           → Tool detail row in slideover
  .detail-row          → Label + value detail row

Animations:
  .animate-fade-in-up  → Staggered card entrance (fadeInUp)
  .animate-pulse       → Status dot pulse
```

## Page Specifications

### 1. Settings Page (`/settings`)

**Purpose**: System setup — providers, preferences, logging. First stop for new users.

#### Tab: Providers (default)

- **Existing functionality preserved**: card grid, slideover view/edit/create, test connection, set default, delete, model chips with capabilities, preset cards for onboarding
- **Changes from current `/providers` page**:
  - Moves INTO Settings as the first tab
  - Page title becomes "Settings" with subtitle "Configure your AI providers, system preferences, and logging."
  - Help box below grid: "What are providers? Providers are the AI services that power your agents..."
  - Tab count shows number of providers
  - "Add another provider" link below grid
- **Empty state**: Preset cards (OpenAI, Anthropic, Ollama) with inline API key flow. Explanatory text: "You need at least one provider to get started."
- **Data**: `listProviders()`, `listModels()`, `createProvider()`, `updateProvider()`, `deleteProvider()`, `testProvider()`, `setDefaultProvider()`

#### Tab: General

- **System info section**: API host, WebSocket URL, data directory (read-only display)
- **Context protection section**:
  - Toggle: "Offload Large Results"
  - Input: Threshold (tokens)
  - Help text: "When an agent's response is very large, z-Bot can save it to disk instead of keeping it in memory. This prevents slowdowns."
- **Data**: `getToolSettings()`, `updateToolSettings()`

#### Tab: Logging

- **File logging toggle** + level/rotation/max files/suppress stdout
- **Restart note**: Inline text "Changes take effect after restarting the daemon" (not a scary warning banner)
- **Help text**: "Logs help you troubleshoot when something goes wrong. You usually don't need to change these unless you're debugging."
- **Data**: `getLogSettings()`, `updateLogSettings()`

### 2. Agents Page (`/agents`)

**Purpose**: Create and manage AI assistants. The core of the product.

#### Tab: My Agents (default)

- **Card grid** (responsive, `minmax(320px, 1fr)`)
- **Each agent card shows**:
  - Avatar (gradient icon, 44px, rounded-12) with online/idle dot
  - Agent name (Fraunces) + ID (mono)
  - Description (2-line clamp)
  - Metadata chips: model (mono, copper), skills count (purple), MCPs count (blue), schedule (green)
  - Footer: status + provider name, hover-reveal edit/duplicate buttons
  - Staggered fadeInUp animation on load
- **Action bar**: search input + "Create Agent" button
- **Click card → slide-over** with:
  - **Basic**: Display name, description
  - **Model**: Provider selector, model selector with capability profile (Thinking/Tools/Vision badges)
  - **Skills**: Toggle list with skill name + description for each
  - **Schedules**: Inline schedule display with enabled toggle + "Add schedule" link (creates schedule in context)
  - **Advanced** (collapsed): Temperature slider, max tokens, thinking toggle, voice toggle, instructions textarea, MCP multi-select
  - **Footer**: Cancel + Save buttons
- **Empty state**: "Agents are your AI assistants. Each agent can have its own personality, skills, and model. Create your first one to get started."
- **Help box**: "Think of agents like team members — each one has a role, a model (brain), skills (expertise), and tools (MCP connections)."
- **Data**: `listAgents()`, `listProviders()`, `listModels()`, `listSkills()`, `listMcps()`, `createAgent()`, `updateAgent()`, `deleteAgent()`

#### Tab: Skills Library

- **Card grid** (same layout as agents, consistent)
- **Each skill card shows**: name (Fraunces), category badge, description preview
- **Click → slide-over**: full skill details (ID, description, category, instructions in code block)
- **Action bar**: search + "Create Skill" button
- **Create skill**: slide-over form with name, display name, description, category, instructions textarea
- **Empty state**: "Skills are reusable instruction packages. They teach your agents how to handle specific tasks."
- **Help box**: "Skills are shared — create once, assign to any agent."
- **Data**: `listSkills()`, `createSkill()`, `deleteSkill()` (Note: `updateSkill()` exists in transport but skills are currently create-and-delete only. Skill editing is out of scope for this revamp — skills are viewed and deleted from the slide-over, created from the form.)

#### Tab: Schedules

- **List/card view** (simpler than agents — one column or compact cards)
- **Each schedule shows**: name, cron in plain English, target agent, enabled/paused toggle, last run, next run
- **Click → expand or modal** for edit
- **Action bar**: search + "Create Schedule" button
- **Create schedule**: modal with name, agent selector (NOT hardcoded to root), cron preset dropdown + custom input, message textarea, timezone, enabled toggle
- **Empty state**: "Schedules run your agents automatically on a timer. Example: 'Check my email every morning at 9 AM'."
- **Key change**: Schedule creation now includes agent selector. Defaults to "root" agent (pre-selected) but user can choose any agent from dropdown. Existing schedules display their `agent_id`. The `respond_to` field from `CronJobResponse` is not surfaced in the UI (internal use only).
- **Data**: `listCronJobs()`, `createCronJob()`, `updateCronJob()`, `deleteCronJob()`, `enableCronJob()`, `disableCronJob()`, `triggerCronJob()`, `listAgents()`

### 3. Integrations Page (`/integrations`)

**Purpose**: Connect z-Bot to external tools and services.

#### Tab: Tool Servers (default — replaces "MCPs")

- **Card grid** (consistent)
- **Each card shows**:
  - Icon + name (Fraunces)
  - Type badge: `stdio` (copper), `http` (blue), `sse` (teal)
  - Command or URL (mono)
  - Description (2-line clamp)
  - Metadata: tool count, enabled/disabled status
  - Footer: last tested time, hover-reveal test/edit buttons
- **Action bar**: search + type filter chips (All/stdio/http) + "Add Tool Server" button
- **Click card → slide-over**:
  - **View mode**: test result, details (type, command, args, env vars, description), discovered tools with descriptions, usage hint ("Go to Agents → Edit → MCPs to enable this server")
  - **Edit mode**: form with type selector, name, description, type-specific fields (command/args/env for stdio, URL for http/sse), enabled toggle
  - **Footer**: Delete, Test, Edit/Save buttons
- **Empty state**: "Tool Servers give your agents superpowers — access to files, databases, APIs, and more. They follow the Model Context Protocol (MCP) standard."
- **Help box**: "Think of tool servers like apps on a phone — each one gives your agents a new ability."
- **Data**: `listMcps()`, `getMcp()`, `createMcp()`, `updateMcp()`, `deleteMcp()`, `testMcp()`

#### Tab: Plugins & Workers

- **Unified card grid** — plugins and workers shown together
- **Each card shows**:
  - Icon + name (Fraunces)
  - Source badge: `Plugin` (green) or `Worker` (blue)
  - Origin path or "WebSocket"
  - Description
  - Metadata: capabilities count, resources count, running/connected status, auto-restart indicator
  - Footer: uptime, hover-reveal configure/restart buttons
- **Action bar**: search + source filter chips (All/Plugins/Workers)
- **Click → slide-over**: capabilities list, resources list, config/secrets for plugins, connection info for workers
- **Empty state**: "Plugins and workers connect z-Bot to external services like Slack, Discord, or custom APIs."
- **Help box**: "Plugins auto-start when z-Bot launches. Drop a plugin folder into `~/Documents/zbot/plugins/` and restart."
- **Install hint**: Inline tip with plugin directory path
- **Data**: `listBridgeWorkers()` with 5-second polling (preserved from current Workers page). Plugin-specific endpoints (restart, config) are deferred — current implementation only shows read-only status from bridge worker data. Configure/restart buttons shown in mockup are aspirational; for v1, these display info only.

## Help Text Strategy

Three layers, all present simultaneously:

1. **Page subtitle**: One sentence below each page title explaining what this page is for
2. **Inline tooltips**: Small `(i)` icons next to technical terms (temperature, thinking, MCP) — hover/click to explain
3. **Help boxes**: Contextual help at the bottom of each section. Always visible. Explains "what is this" and "why would I use it" in plain language. Links to docs where applicable.

**Empty states** become mini-tutorials: explain the concept, show an example use case, provide the primary action.

## Animations

- **Card entrance**: Staggered `fadeInUp` (0.4s, 50ms delay between cards)
- **Slide-over**: `translateX(100% → 0)` with cubic-bezier easing (0.35s)
- **Status dots**: Gentle `pulse` animation (2s infinite)
- **Card hover**: `translateY(-2px)` + box-shadow expansion
- **Tab transitions**: Border-bottom color transition (0.15s)
- **Button hover**: `translateY(-1px)` + brightness filter

## Backwards Compatibility

- Old routes (`/providers`, `/skills`, `/hooks`, `/connectors`, `/mcps`) redirect to new locations
- All existing API endpoints unchanged — only the UI layer changes
- All existing functionality preserved — nothing removed, only reorganized and enhanced
- CSS custom properties maintain the same semantic names where possible; new tokens added for new concepts

## Files Changed

### Modified
- `apps/ui/src/App.tsx` — Routes (add redirects, new paths), navigation structure, sidebar groups
- `apps/ui/src/styles/theme.css` — All design tokens replaced (see Token Migration section)
- `apps/ui/src/styles/components.css` — New component classes added
- `apps/ui/src/styles/fonts.css` — Fraunces, IBM Plex Sans, JetBrains Mono imports
- `apps/ui/src/features/settings/WebSettingsPanel.tsx` — Complete rewrite: absorbs providers, tabbed layout
- `apps/ui/src/features/agent/WebAgentsPanel.tsx` — Complete rewrite: absorbs skills/schedules, card grid
- `apps/ui/src/features/agent/AgentEditPanel.tsx` — Redesigned slide-over with skills/schedules sections
- `apps/ui/src/features/index.ts` — Update barrel exports for renamed/removed modules

### Moved (provider sub-components → settings feature)
- `apps/ui/src/features/integrations/ProviderCard.tsx` → `apps/ui/src/features/settings/ProviderCard.tsx`
- `apps/ui/src/features/integrations/ProviderSlideover.tsx` → `apps/ui/src/features/settings/ProviderSlideover.tsx`
- `apps/ui/src/features/integrations/ProvidersGrid.tsx` → `apps/ui/src/features/settings/ProvidersGrid.tsx`
- `apps/ui/src/features/integrations/ProvidersEmptyState.tsx` → `apps/ui/src/features/settings/ProvidersEmptyState.tsx`
- `apps/ui/src/features/integrations/ModelChip.tsx` → `apps/ui/src/shared/ui/ModelChip.tsx` (shared — used in settings + agents)
- `apps/ui/src/features/integrations/providerPresets.ts` → `apps/ui/src/features/settings/providerPresets.ts`

### New
- `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` — **New file** replacing the old providers-focused file. Unified integrations page (Tool Servers + Plugins & Workers)
- `apps/ui/src/features/integrations/ToolServerCard.tsx` — Tool server card component
- `apps/ui/src/features/integrations/ToolServerSlideover.tsx` — Tool server detail/edit slide-over
- `apps/ui/src/features/integrations/PluginWorkerCard.tsx` — Plugin/worker card component
- `apps/ui/src/components/Slideover.tsx` — Shared slide-over shell (extracted, reused across pages)
- `apps/ui/src/components/HelpBox.tsx` — Reusable help box component
- `apps/ui/src/components/TabBar.tsx` — Reusable tab bar component
- `apps/ui/src/components/ActionBar.tsx` — Search + filters + action button bar
- `apps/ui/src/components/MetaChip.tsx` — Metadata chip component with variants
- `apps/ui/src/components/EmptyState.tsx` — Enhanced empty state with help text

### Deleted (content absorbed into other pages)
- `apps/ui/src/features/skills/WebSkillsPanel.tsx` — Content moves to Agents page Skills tab
- `apps/ui/src/features/cron/WebCronPanel.tsx` — Content moves to Agents page Schedules tab
- `apps/ui/src/features/connectors/WebConnectorsPanel.tsx` — Content moves to Integrations Plugins & Workers tab
- `apps/ui/src/features/mcps/WebMcpsPanel.tsx` — Content moves to Integrations Tool Servers tab

### Route Redirects (in App.tsx)
Old routes use `<Navigate replace>` to preserve bookmarks:
- `/providers` → `/settings`
- `/skills` → `/agents?tab=skills`
- `/hooks` → `/agents?tab=schedules`
- `/connectors` → `/integrations?tab=plugins`
- `/mcps` → `/integrations`

## Token Migration Strategy

The existing `theme.css` tokens are **replaced wholesale**, not extended. This is a visual refresh, not an additive change.

### Token Mapping (existing → new)

| Existing Token | New Token | Notes |
|---|---|---|
| `--background` (`#F7F5F2` / `#191919`) | `--background` (`#f6f3ee` / `#141210`) | Warmer tones |
| `--card` (`#FFFFFF` / `#1E1E1E`) | `--background-surface` (`#ffffff` / `#1a1816`) | Renamed for clarity |
| `--popover` | `--background-elevated` (`#f0ece6` / `#201e1b`) | Renamed |
| `--sidebar` (`#EFECE7` / `#141414`) | `--sidebar` (`#1a1714` / `#0d0c0a`) | **Dark in both themes now** |
| `--primary` (`#C17D3F` / `#D4945A`) | `--primary` (`#a07d52` / `#c8956c`) | Similar copper, adjusted |
| `--primary-muted` | `--primary-muted` | Same concept, new value |
| (none) | `--primary-subtle` | New: very light accent bg |
| `--foreground` | `--foreground` | Same concept, new value |
| `--muted-foreground` | `--muted-foreground` | Same concept |
| (none) | `--subtle-foreground` | New: tertiary text level |
| (none) | `--dim-foreground` | New: quaternary text level |
| `--border` | `--border` | Same concept, new value |
| (none) | `--border-hover` | New: focus/hover border |
| `--font-sans` | `--font-body` | Renamed, value changes to IBM Plex Sans |
| `--font-mono` | `--font-mono` | Same name, value changes to JetBrains Mono |
| (none) | `--font-display` | New: Fraunces serif for headings |

### Migration approach
1. Replace `:root` and `.dark` blocks in `theme.css` with new values
2. Update the Tailwind `@theme inline` block to reference new tokens
3. Grep for any hardcoded old token references in components and update
4. The sidebar now uses dark colors in BOTH themes — sidebar foreground tokens (`--sidebar-foreground`) stay light in both

### Sidebar in Light Theme
The sidebar stays dark (`#1a1714`) in the light theme. This means:
- Sidebar foreground colors remain light (cream/white) regardless of theme
- The logo color stays `--primary` (copper) — no variant needed
- `ThemeToggle` in sidebar footer keeps light icon color
- The sidebar is effectively "theme-independent" — it has its own fixed palette

### Font Loading
Add to `fonts.css`:
```css
@import url('https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,300;9..144,400;9..144,500;9..144,700&family=IBM+Plex+Sans:wght@300;400;500;600&family=JetBrains+Mono:wght@400;500&display=swap');
```

### Type Badges
Four MCP transport types get badges: `stdio` (copper), `http` (blue), `sse` (teal), `streamable-http` (maps to `http` badge).

## Accessibility

- **Keyboard navigation**: Tab bar items are focusable and selectable via Enter/Space. Slide-over traps focus when open, Escape closes it.
- **ARIA**: Slide-over uses `role="dialog"` + `aria-modal="true"`. Tab bar uses `role="tablist"` / `role="tab"` / `role="tabpanel"`.
- **Reduced motion**: Card entrance animations and hover transforms respect `prefers-reduced-motion: reduce` — replaced with instant opacity transitions.
- **Color contrast**: All text colors meet WCAG AA against their backgrounds. The warm palette was validated for sufficient contrast ratios.

## Mockups

Interactive HTML mockups are available in `.superpowers/brainstorm/`:
- `hybrid-ac.html` — Design direction: dark + light theme comparison (Settings/Providers)
- `agents-page-mockup.html` — Full Agents page with card grid and slide-over
- `integrations-page-mockup.html` — Full Integrations page with Tool Servers and Plugins tabs
