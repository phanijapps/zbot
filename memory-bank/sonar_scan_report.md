# SonarQube Scan Report

**Project:** phanijapps_zbot (z-Bot)
**Scanned:** main branch (as of 2026-04-11)
**Lines of Code:** 104,215

## Dashboard Summary

| Metric | Value | Rating |
|--------|-------|--------|
| Security | 0 vulnerabilities, 0 hotspots | A |
| Reliability | 23 bugs | C |
| Maintainability | 514 code smells | A |
| Coverage | 0.0% | — |
| Duplication | 4.0% | — |

**Note:** Many issues shown below are already fixed on `feature/test-coverage` branch but not yet merged to main.

---

## 1. Security

**Vulnerabilities: 0**
**Security Hotspots: 0** (8 were fixed on feature/test-coverage: ReDoS regex, Math.random, HTTPS enforcement)

---

## 2. Reliability — 81 Open Issues

### S6848 — Non-native interactive elements (MAJOR) — 16 issues

Clickable `<div>` elements missing `role="button"`, `tabIndex`, `onKeyDown`.

| File | Line | Status |
|------|------|--------|
| ChatInput.tsx | 117 | Fixed on branch |
| HeroInput.tsx | 124 | Fixed on branch |
| McpStep.tsx | 106-109 | Fixed on branch |
| WebIntegrationsPanel.tsx | 668 | Fixed on branch |
| WebIntegrationsPanel.tsx | 1216 | Fixed on branch |
| AgentEditPanel.tsx | 303-306 | Fixed on branch |
| AgentEditPanel.tsx | 322-325 | Fixed on branch |
| AgentEditPanel.tsx | 371-375 | Fixed on branch |
| WebAgentsPanel.tsx | 552-556 | Fixed on branch |
| WebAgentsPanel.tsx | 657-661 | Fixed on branch |
| ProviderSlideover.tsx | 291-294 | Fixed on branch |
| MemoryFactCard.tsx | 76-88 | Fixed on branch |
| WebOpsDashboard.tsx | 195-198 | Fixed on branch |
| WebOpsDashboard.tsx | 229 | Fixed on branch |

### S1082 — Missing keyboard listener (MINOR) — 14 issues

Paired with S6848 — every `onClick` fix above includes `onKeyDown`.

### S6853 — Form label not associated (MAJOR) — 35 issues

`<label>` elements not linked to their controls via `htmlFor`/`id`.

| File | Lines | Count | Status |
|------|-------|-------|--------|
| WebAgentsPanel.tsx | 798-1135 | 18 | Fixed on branch |
| WebIntegrationsPanel.tsx | 877-1019 | 6 | Fixed on branch |
| AgentEditPanel.tsx | 175-226 | 4 | Fixed on branch |
| WebSettingsPanel.tsx | 419-676 | 5 | Fixed on branch |
| ProvidersEmptyState.tsx | 144 | 1 | Fixed on branch |
| WebMemoryPanel.tsx | 364 | 1 | Fixed on branch |

### S6443 — Setter uses matching state variable (MAJOR) — 6 issues

| File | Lines | Status |
|------|-------|--------|
| WebSettingsPanel.tsx | 206, 209, 227, 230, 254, 258 | Fixed on branch (functional updater) |

### S6757 — `this` in functional component (MAJOR) — 2 issues

| File | Lines | Status |
|------|-------|--------|
| GraphCanvas.tsx | 264, 285 | Fixed on branch (arrow functions with nodes param) |

### S6439 — Leaked conditional value (MAJOR) — 1 issue

| File | Line | Status |
|------|------|--------|
| ProviderSlideover.tsx | 493 | Fixed on branch |

### S7758 — charCodeAt vs codePointAt (MINOR) — 3 issues

| File | Lines | Status |
|------|-------|--------|
| WebIntegrationsPanel.tsx | 49, 57 | Open — intentional hash function |
| WebAgentsPanel.tsx | 59 | Open — intentional hash function |

### S6772 — Ambiguous JSX spacing (MAJOR) — 1 issue

| File | Line | Status |
|------|------|--------|
| WebOpsDashboard.tsx | 696 | Open |

### S7773 — Number.parseInt/parseFloat (MINOR) — 3 issues

| File | Lines | Status |
|------|-------|--------|
| AgentEditPanel.tsx | 260, 277 | Fixed on branch |
| GenerativeCanvas.tsx | 262 | Fixed on branch |

### S3923 — Identical conditional blocks (MAJOR) — 1 issue

| File | Lines | Status |
|------|-------|--------|
| App.tsx | 90-94 | Fixed on branch |

### S4659 — Unknown CSS pseudo-class (MAJOR) — 1 issue

| File | Line | Status |
|------|------|--------|
| index.css | 93 | Fixed on branch (Tailwind v4 comment) |

### S6850 — Heading accessibility (MAJOR) — 1 issue

| File | Lines | Status |
|------|-------|--------|
| shared/ui/card.tsx | 33-37 | Fixed on branch |

---

## 3. Maintainability — 514 Code Smells (Top Issues)

### Cognitive Complexity (CRITICAL) — Rust

| File | Line | Complexity | Limit |
|------|------|-----------|-------|
| runtime/agent-runtime/src/executor.rs | 483 | **194** | 15 |
| gateway/gateway-execution/src/runner.rs | 3064 | 41 | 15 |
| runtime/agent-runtime/src/executor.rs | 1695 | 40 | 15 |
| gateway/gateway-execution/src/session_state.rs | 423 | 38 | 15 |
| gateway/gateway-templates/src/lib.rs | 257 | 28 | 15 |

### Cognitive Complexity (CRITICAL) — TypeScript

| File | Line | Complexity | Status |
|------|------|-----------|--------|
| fast-chat-hooks.ts | 267 | 51 | Fixed on branch → ~5 |
| SessionChatViewer.tsx | 480 | 45 | Open |
| WebSettingsPanel.tsx | 33 | 43 | Open |
| SetupWizard.tsx | 99 | 37 | Open |
| http.ts | 1075 | 33 | Open |
| ProviderCard.tsx | 17 | 30 | Open |
| ProviderSlideover.tsx | 47 | 26 | Open |
| mission-hooks.ts | 235 | 21 | Fixed on branch → ~3 |
| WebIntegrationsPanel.tsx | 117, 307 | 21, 17 | Open |
| WebAgentsPanel.tsx | 86, 153 | 21, 16 | Open |
| GraphView.tsx | 217 | 20 | Open |
| WebMemoryPanel.tsx | 31 | 20 | Open |
| web_reader.py | 221 | 45 | Open |
| ddg_search.py | 212 | 19 | Open |
| web_reader.py | 378 | 19 | Open |

---

## 4. Duplication — 4.0%

4.0% duplicated lines across the codebase. Within acceptable range (SonarQube threshold is typically 3-5%).

---

## Summary: What Merging `feature/test-coverage` Resolves

| Category | Before Merge | After Merge (estimated) |
|----------|-------------|------------------------|
| Security Hotspots | 0 | 0 |
| Reliability (bugs) | 81 | ~8 remaining |
| Maintainability (code smells) | 514 | ~500 remaining |
| Coverage | 0% | >0% (Rust LCOV from cargo-llvm-cov) |

**Action Required:** Merge `feature/test-coverage` PR to main, then re-scan.
