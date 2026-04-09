---
name: premium-report
description: >
  Transform any AI-generated or data-dump HTML report into a premium, visually distinctive,
  dark-themed report with professional typography, animated cards, gauges, data bars, and
  a coherent design system. Use this skill whenever a user wants to: enhance or beautify an
  existing HTML report, create a polished research report from raw data, produce a professional
  dashboard-style document, or make any data-heavy output look publication-ready. Triggers
  include: "enhance this report", "make this look professional", "beautify this HTML",
  "create a report for X", "turn this data into a report", "design a dashboard for",
  "deep research report", or any upload of a plain/ugly HTML report asking for improvements.
  Apply to any domain: finance, analytics, research, health, engineering, project status,
  sales, HR, product metrics, or anything else.
metadata:
  author: phani
  version: "1.0"
---

# Premium Report Skill

Convert any plain, AI-generated, or data-dump HTML into a **premium, self-contained report**
with a distinctive visual identity. The output is always a **single `.html` file** — no external
CSS, no JS frameworks, no missing asset references. Google Fonts are the only external dependency.

---

## Step 1 — Understand the Report Domain

Before writing any HTML, identify:

1. **Domain** — What is this report about? (Finance, analytics, health, project status, etc.)
2. **Primary metric** — What is the single most important number or status?
3. **Data shape** — Does it have KPIs, tables, time-series, rankings, text summaries, or risk/opportunity lists?
4. **Tone** — Executive briefing? Technical deep-dive? Operational dashboard?

Then pick an aesthetic direction. See [`references/aesthetics.md`](references/aesthetics.md) for
a full palette and font menu. Default: **dark terminal-editorial** (described below).

---

## Step 2 — Design System

Apply this CSS foundation. Swap accent colors to match domain tone:

```css
:root {
  /* Backgrounds */
  --bg:       #06080f;
  --surface:  #0d1220;
  --surface2: #121929;

  /* Borders */
  --border:        rgba(255,255,255,0.06);
  --border-accent: rgba(0,255,170,0.2);

  /* Semantic colors — adjust per domain */
  --accent:  #00ffaa;   /* positive / primary highlight */
  --accent2: #00c9ff;   /* secondary highlight */
  --warn:    #ff4d6d;   /* negative / alert */
  --gold:    #ffd166;   /* neutral / caution */

  /* Text */
  --text:   #e8eaf0;
  --muted:  #5a6278;
  --muted2: #8892a4;

  /* Typography */
  --font-display: 'Syne', sans-serif;
  --font-mono:    'DM Mono', monospace;
  --font-body:    'DM Sans', sans-serif;
}
```

**Domain color overrides** (swap `--accent` / `--warn`):
| Domain | Accent | Warn | Secondary |
|---|---|---|---|
| Finance / stocks | `#00ffaa` | `#ff4d6d` | `#00c9ff` |
| Health / medical | `#4fffb0` | `#ff6b35` | `#a78bfa` |
| Engineering / ops | `#00c9ff` | `#ff9500` | `#a3e635` |
| Sales / growth | `#ffd166` | `#ff4d6d` | `#06d6a0` |
| Research / academic | `#c084fc` | `#f87171` | `#38bdf8` |
| Project / HR | `#34d399` | `#fb923c` | `#60a5fa` |

---

## Step 3 — Layout & Components

Use these reusable components. Mix and match based on the data available.

### Header
Every report starts with a full-width header:
- **Entity badge** — mono font, accent color, pulsing dot (use `@keyframes pulse`)
- **Report title** — large display font, gradient white text, tight tracking
- **Subtitle / meta** — mono, muted, categorization info
- **Primary value / status** (right-aligned) — large, accent-colored, text-shadow glow
- Bottom border separator

### KPI Strip
4-cell grid of pill-cards. Each cell:
```html
<div class="kpi-cell">
  <div class="kpi-key">LABEL</div>
  <div class="kpi-value" style="color:var(--accent)">VALUE</div>
  <div class="kpi-sub">context blurb</div>
</div>
```
Use `clamp()` for font sizes. Collapse to 2×2 at 700px.

### Stat Row
For key-value pairs inside cards:
```html
<div class="stat">
  <span class="stat-key">Label</span>
  <span class="stat-val positive|negative|neutral">Value</span>
</div>
```
All `.stat-val` values: `font-family: var(--font-mono)`. Never use default font for numbers.

### Gauge Bar
For any percentage or 0–100 metric:
```html
<div class="gauge-label"><span>Metric Name</span><span>56.4</span></div>
<div class="gauge-track">
  <div class="gauge-fill gauge-accent" style="width:56.4%"></div>
</div>
```
Track: `height: 4–6px`, `background: rgba(255,255,255,0.06)`. Fill uses gradient.
Add `transition: width 1s ease` for animation on load.

### Range Bar with Position Dot
For any metric with a min/max/current (price ranges, scores, percentiles):
```html
<div class="range-track">
  <div class="range-fill" style="width:100%"></div>
  <!-- dot at: (current - min) / (max - min) * 100 -->
  <div class="range-dot" style="left:XX%"></div>
</div>
```

### Segmented Ownership / Breakdown Bar
For showing composition (% breakdown of categories):
```html
<div class="seg-row">
  <div class="seg-label">Category</div>
  <div class="seg-track"><div class="seg-fill" style="width:XX%;background:var(--accent)"></div></div>
  <div class="seg-pct">XX%</div>
</div>
```

### Opportunity / Risk Cards
For qualitative insight items:
```html
<div class="opp"><span class="opp-icon">▲</span> Insight text here</div>
<div class="risk"><span class="risk-icon">▼</span> Concern text here</div>
```
`.opp`: green-tinted bg + border. `.risk`: red-tinted bg + border. Never plain bullets.

### Tag Pills
For categorical ratings, statuses, or labels:
```html
<span class="tag tag-positive">Label</span>
<span class="tag tag-neutral">Label</span>
<span class="tag tag-negative">Label</span>
```

### Section Dividers
Between major sections:
```html
<div class="section-label">SECTION NAME</div>
```
Style: mono, muted, small caps, with a `::after` line extending to the right edge.

### Card Wrapper
All content blocks go in cards:
```css
.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 24px;
  position: relative;
  overflow: hidden;
  transition: border-color 0.3s ease, transform 0.2s ease;
  animation: fadeUp 0.6s ease both;
}
.card:hover { border-color: rgba(255,255,255,0.1); transform: translateY(-2px); }
.card::before {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,0.08), transparent);
}
```

---

## Step 4 — Atmosphere & Polish

Always include:

**Noise overlay** (subtle grain):
```css
body::before {
  content: '';
  position: fixed;
  inset: 0;
  background-image: url("data:image/svg+xml,..."); /* fractalNoise SVG data URI */
  opacity: 0.4;
  pointer-events: none;
  z-index: 0;
}
```

**Ambient glow** (2 radial blurs, top-right and bottom-left):
```css
.ambient { position: fixed; border-radius: 50%; filter: blur(120px); pointer-events: none; z-index: 0; }
.ambient-1 { background: radial-gradient(circle, rgba(0,255,170,0.04) 0%, transparent 70%); top: -200px; right: -100px; }
.ambient-2 { background: radial-gradient(circle, rgba(0,201,255,0.04) 0%, transparent 70%); bottom: 10%; left: -100px; }
```

**Animations:**
```css
@keyframes fadeUp {
  from { opacity: 0; transform: translateY(16px); }
  to   { opacity: 1; transform: translateY(0); }
}
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
```
Stagger cards: `.card:nth-child(N) { animation-delay: N * 0.05s; }`

---

## Step 5 — Report Structure Template

Adapt this order based on available data. Skip sections gracefully if data is absent.

```
1. Header          — entity name, primary value, meta
2. KPI Strip       — 4 most important numbers
3. [Section A]     — primary analytical content (3-col grid)
4. [Section B]     — secondary analytical content (3-col grid)
5. Opportunities   — qualitative positives (left col)
   Risks           — qualitative concerns (right col)
6. Description     — full-width narrative / about text
7. Footer          — date, source, disclaimer
```

For reports with **time-series data**: add a Chart.js line/bar chart card (vanilla JS, CDN from cdnjs.cloudflare.com).
For reports with **tables**: use styled `<table>` with mono values, striped rows via `:nth-child(even)`.
For reports with **rankings**: use numbered `.rank-row` components with position indicator.

---

## Step 6 — Footer (always required)

```html
<div class="footer">
  <div class="footer-text">
    Report generated [DATE] · Data sourced from [SOURCE] · For informational purposes only.
  </div>
  <div class="footer-badge">DEEP RESEARCH</div>
</div>
```

---

## Quality Checklist

Before saving the file, verify:
- [ ] Single self-contained `.html` file — no external CSS or JS files
- [ ] Google Fonts `<link>` in `<head>` (Syne + DM Mono + DM Sans or equivalent trio)
- [ ] All numeric values in `var(--font-mono)` font
- [ ] Positive values: `var(--accent)`, negative: `var(--warn)`, neutral: `var(--muted2)`
- [ ] Ambient divs and noise overlay present
- [ ] `fadeUp` animation on all cards with staggered delays
- [ ] Responsive breakpoints: 3-col → 2-col at 900px, 1-col at 560px
- [ ] Footer with date, source attribution, and disclaimer
- [ ] `z-index: 1` on `.container` so it sits above ambient/noise layers
- [ ] No generic fonts (no Inter, Roboto, Arial, system-ui as primary)
- [ ] No purple-gradient-on-white — commit to a dark theme

---

## Output Path Convention

```
/mnt/user-data/outputs/<subject>_report_enhanced.html
```

Use the subject name (ticker, project name, product name, etc.) as the prefix.

---

## See Also

- [`references/aesthetics.md`](references/aesthetics.md) — full font pairings, color palettes, and aesthetic modes
- [`references/components.md`](references/components.md) — copy-paste HTML snippets for every component
