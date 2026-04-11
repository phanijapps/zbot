# SonarQube Fix Report

## Security Scan
**Result: 0 vulnerabilities.** 8 security hotspots resolved:

| Hotspot ID | File | Category | Fix |
|------------|------|----------|-----|
| AZ13h4m3 | GraphCanvas.tsx:224-225 | ReDoS | Made regex non-greedy + bounded character classes |
| AZ13h4kk (x2) | providerPresets.ts:104 | ReDoS | Replaced `/\/+$/` regex with `trimTrailingSlashes()` helper |
| AZ13h4ns (x2) | ProvidersStep.tsx:19,124 | ReDoS | Replaced `/\/+$/` regex with `trimSlashes()` helper |
| AZ13h4hu | GraphView.tsx:93 | Weak PRNG | Replaced `Math.random()` with deterministic `(i * 7919) % 100` |
| AZ19C8BM (x2) | install.sh:80,94 | HTTP downgrade | Added `--proto '=https'` to curl to enforce HTTPS on redirects |

## Reliability Scan

### Fixed

| Issue ID | File | Rule | Fix |
|----------|------|------|-----|
| AZ17CbUc | web_reader.py:43-44 | S1656 (self-assignment) | Fixed constants — were self-assigning instead of setting values |
| Multiple (17) | 16 UI files | S6848 (non-native interactive) | Added `role="button"`, `tabIndex={0}`, `onKeyDown` to all `<div onClick>` |
| Multiple (17) | 16 UI files | S1082 (missing keyboard) | Added Enter/Space keyboard handlers alongside onClick |
| AZ17Ca-C | ArtifactSlideOut.tsx:96 | S1090 (iframe title) | Added `title="Artifact preview"` |
| AZ17Ca_9 | ArtifactsPanel.tsx:141 | S1090 (iframe title) | Added `title="Artifact preview"` |
| Multiple (17) | 4 UI files | S7773 (Number.parseInt) | Replaced `parseInt`/`parseFloat` with `Number.parseInt`/`Number.parseFloat` |

### Skipped (Low Risk)

| Issue ID | File | Rule | Reason |
|----------|------|------|--------|
| Multiple (5) | ArtifactSlideOut, ArtifactsPanel | S4084 (media captions) | Audio/video elements are agent-generated artifacts — captions not available |
| Multiple (2) | GraphCanvas.tsx | S6757 (this in FC) | D3.js callback pattern requires `this` context — not a React anti-pattern |
| Multiple (7) | LearningHealthBar.tsx | S6772 (ambiguous spacing) | Intentional JSX whitespace for inline text formatting |
| Multiple (15) | WebSettingsPanel.tsx | S6853 (label association) | Settings form labels — would require htmlFor+id refactor across all form fields |
| Multiple (2) | WebSettingsPanel.tsx | S6443 (setter with state) | Intentional error recovery pattern — resets to previous state on save failure |
| 1 | ProviderSlideover.tsx | S6439 (leaked value) | Conditional JSX rendering — `{condition && <Component>}` is standard React |
| 1 | IntelligenceFeed.tsx:99 | S6772 (ambiguous spacing) | Intentional inline text |
| Multiple (2) | WebIntegrationsPanel.tsx | S7758 (codePointAt) | Hash function uses charCodeAt intentionally — codePointAt changes behavior |

## Maintainability Scan

### Fixed (Previous Commits)

| Issue ID | File | Rule | Fix |
|----------|------|------|-----|
| Multiple (3) | mission-hooks.ts | S3776 (complexity 119,24,17) | Extracted 22 event handlers into named functions |
| 1 | useSessionTrace.ts | S3776 (complexity 38) | Extracted 3 log processor functions |
| 1 | trace-types.ts | S3776 (complexity 18) | Refactored to lookup table |
| 1 | TraceNodeComponent.tsx | S3776 (complexity 17) | Extracted helper functions |
| 1 | fast-chat-hooks.ts | S3776 (complexity 51) | Extracted 12 event handlers |
| 10 | Shell scripts, Python files, TS files | Various | Quick wins: `[` → `[[`, Math.trunc, await fix, string constants |

### Skipped (Requires Component Refactoring)

| Issue ID | File | Rule | Reason |
|----------|------|------|--------|
| AZ13h4lK | WebSettingsPanel.tsx:33 | S3776 (complexity 43) | Requires splitting into sub-components — visual testing needed |
| AZ13h4nU | SetupWizard.tsx:99 | S3776 (complexity 37) | Large wizard — requires step extraction |
| AZ13h4ee | http.ts:1075 | S3776 (complexity 33) | WebSocket message handler — deep refactor |
| AZ13h4lT | ProviderCard.tsx:17 | S3776 (complexity 30) | Capability aggregation logic |
| AZ13h4k- | ProviderSlideover.tsx:47 | S3776 (complexity 26) | Form component — needs section extraction |
| AZ13h4kZ | WebIntegrationsPanel.tsx:117,307 | S3776 (complexity 21,17) | Large panel — needs section components |
| AZ13h4p1 | WebAgentsPanel.tsx:86,153 | S3776 (complexity 21,16) | Agent panel — needs sub-components |
| AZ13h4hu | GraphView.tsx:217 | S3776 (complexity 20) | D3 rendering logic |
| AZ13h4hb | WebMemoryPanel.tsx:31,333 | S3776/S2004 (complexity 20, nesting 4+) | Memory panel — needs extraction |
| AZ13h4fA | SessionChatViewer.tsx:480 | S3776 (complexity 45) | Legacy chat viewer — large refactor |
| AZ13h4nK | graph-hooks.ts:147 | S3776 (complexity 17) | Graph data processing |
| AZ13h4oW | ReviewStep.tsx:63 | S3776 (complexity 16) | Barely over threshold |
| AZ13h4t0 | ddg_search.py:212 | S3776 (complexity 19) | Python script — search function |
| AZ13h4ts | web_reader.py:221,378 | S3776 (complexity 45,19) | Python script — content extraction |

## Duplication Check
Deferred — no duplicated_lines_density metric query performed. To be addressed in a future pass.
