# Components Reference

Copy-paste HTML + CSS snippets for every reusable component.
All snippets assume the CSS design system from SKILL.md is loaded.

---

## Entity Badge (header)

```html
<div class="entity-badge">ENTITY NAME · CATEGORY</div>
```
```css
.entity-badge {
  display: inline-flex; align-items: center; gap: 8px;
  background: rgba(0,255,170,0.08);
  border: 1px solid rgba(0,255,170,0.25);
  border-radius: 4px; padding: 4px 12px;
  font-family: var(--font-mono); font-size: 0.75rem;
  color: var(--accent); letter-spacing: 0.12em; width: fit-content;
}
.entity-badge::before { content: '◉'; font-size: 0.6rem; animation: pulse 2s ease infinite; }
```

---

## KPI Cell

```html
<div class="kpi-cell">
  <div class="kpi-key">METRIC LABEL</div>
  <div class="kpi-value positive">42.7%</div>
  <div class="kpi-sub">supporting context</div>
</div>
```
```css
.kpi-grid { display: grid; grid-template-columns: repeat(4,1fr); gap: 12px; margin-bottom: 16px; }
.kpi-cell { background: var(--surface); border: 1px solid var(--border); border-radius: 10px; padding: 18px 16px; }
.kpi-key { font-family: var(--font-mono); font-size: 0.62rem; color: var(--muted); letter-spacing: 0.1em; text-transform: uppercase; margin-bottom: 8px; }
.kpi-value { font-family: var(--font-display); font-size: 1.5rem; font-weight: 700; line-height: 1; }
.kpi-sub { font-family: var(--font-mono); font-size: 0.65rem; color: var(--muted2); margin-top: 4px; }
@media (max-width: 700px) { .kpi-grid { grid-template-columns: repeat(2,1fr); } }
```

---

## Stat Row

```html
<div class="stat">
  <span class="stat-key">Label</span>
  <span class="stat-val positive">$4.04</span>
</div>
```
```css
.stat { display: flex; justify-content: space-between; align-items: center; padding: 10px 0; border-bottom: 1px solid var(--border); font-size: 0.85rem; }
.stat:last-child { border-bottom: none; }
.stat-key { color: var(--muted2); font-weight: 300; }
.stat-val { font-family: var(--font-mono); font-size: 0.82rem; font-weight: 500; }
.positive { color: var(--accent); }
.negative { color: var(--warn); }
.neutral  { color: var(--muted2); }
```

---

## Gauge Bar

```html
<div class="gauge-row">
  <div class="gauge-label"><span>Metric Name</span><span>56.4</span></div>
  <div class="gauge-track">
    <div class="gauge-fill gauge-gradient" style="width:56.4%"></div>
  </div>
</div>
```
```css
.gauge-row { margin: 12px 0 4px; }
.gauge-label { display: flex; justify-content: space-between; font-family: var(--font-mono); font-size: 0.68rem; color: var(--muted); margin-bottom: 6px; }
.gauge-track { width: 100%; height: 4px; background: rgba(255,255,255,0.06); border-radius: 2px; overflow: hidden; }
.gauge-fill { height: 100%; border-radius: 2px; transition: width 1s ease; }
.gauge-accent   { background: linear-gradient(90deg, rgba(0,255,170,0.6), var(--accent)); }
.gauge-warn     { background: linear-gradient(90deg, rgba(255,77,109,0.6), var(--warn)); }
.gauge-gradient { background: linear-gradient(90deg, var(--accent), var(--gold), var(--warn)); }
```

---

## Range Bar with Position Dot

```html
<div class="range-bar-labels"><span>MIN</span><span style="color:var(--accent)">NOW VALUE</span><span>MAX</span></div>
<div class="range-track">
  <div class="range-fill" style="width:100%"></div>
  <!-- dot left% = (current - min) / (max - min) * 100 -->
  <div class="range-dot" style="left:XX%"></div>
</div>
```
```css
.range-bar-labels { display: flex; justify-content: space-between; font-family: var(--font-mono); font-size: 0.68rem; color: var(--muted); margin-bottom: 8px; }
.range-track { position: relative; height: 6px; background: rgba(255,255,255,0.06); border-radius: 3px; }
.range-fill { position: absolute; height: 100%; background: linear-gradient(90deg, rgba(255,77,109,0.5), var(--gold), rgba(0,255,170,0.6)); border-radius: 3px; }
.range-dot { position: absolute; top: 50%; transform: translate(-50%,-50%); width: 12px; height: 12px; background: white; border-radius: 50%; border: 2px solid var(--accent); box-shadow: 0 0 10px rgba(0,255,170,0.5); }
```

---

## Segmented Breakdown Bar

```html
<div class="seg-row">
  <div class="seg-label">Category A</div>
  <div class="seg-track"><div class="seg-fill" style="width:72%;background:var(--accent);opacity:0.7"></div></div>
  <div class="seg-pct positive">72%</div>
</div>
```
```css
.seg-row { display: flex; align-items: center; gap: 12px; margin-bottom: 10px; font-size: 0.82rem; }
.seg-label { width: 120px; color: var(--muted2); flex-shrink: 0; font-size: 0.78rem; }
.seg-track { flex: 1; height: 5px; background: rgba(255,255,255,0.06); border-radius: 3px; overflow: hidden; }
.seg-fill { height: 100%; border-radius: 3px; }
.seg-pct { font-family: var(--font-mono); font-size: 0.72rem; width: 48px; text-align: right; }
```

---

## Opportunity / Risk Cards

```html
<div class="opp"><span class="opp-icon">▲</span> Positive insight text</div>
<div class="risk"><span class="risk-icon">▼</span> Risk or concern text</div>
```
```css
.opp, .risk { padding: 12px 14px; border-radius: 6px; margin-bottom: 8px; font-size: 0.84rem; line-height: 1.5; display: flex; align-items: flex-start; gap: 10px; }
.opp  { background: rgba(0,255,170,0.05); border: 1px solid rgba(0,255,170,0.12); }
.risk { background: rgba(255,77,109,0.08); border: 1px solid rgba(255,77,109,0.15); }
.opp-icon  { color: var(--accent); font-size: 0.7rem; margin-top: 3px; flex-shrink: 0; }
.risk-icon { color: var(--warn);   font-size: 0.7rem; margin-top: 3px; flex-shrink: 0; }
```

---

## Tag Pills

```html
<div class="tag-row">
  <span class="tag tag-positive">Confirmed</span>
  <span class="tag tag-neutral">Pending</span>
  <span class="tag tag-negative">At Risk</span>
</div>
```
```css
.tag-row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
.tag { font-family: var(--font-mono); font-size: 0.68rem; padding: 4px 10px; border-radius: 3px; letter-spacing: 0.06em; }
.tag-positive { background: rgba(0,255,170,0.1); color: var(--accent); border: 1px solid rgba(0,255,170,0.2); }
.tag-neutral  { background: rgba(255,209,102,0.1); color: var(--gold); border: 1px solid rgba(255,209,102,0.2); }
.tag-negative { background: rgba(255,77,109,0.1); color: var(--warn); border: 1px solid rgba(255,77,109,0.2); }
```

---

## Section Label Divider

```html
<div class="section-label">SECTION NAME</div>
```
```css
.section-label { font-family: var(--font-mono); font-size: 0.65rem; letter-spacing: 0.2em; color: var(--muted); text-transform: uppercase; display: flex; align-items: center; gap: 16px; margin: 28px 0 16px; }
.section-label::after { content: ''; flex: 1; height: 1px; background: var(--border); }
```

---

## Status/Signal Badge

```html
<div class="signal-badge">
  <div class="signal-word">ON TRACK</div>
  <div class="signal-sub">HIGH CONFIDENCE</div>
</div>
```
```css
.signal-badge { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 24px 16px; border-radius: 8px; background: var(--surface2); border: 1px solid var(--border); text-align: center; gap: 4px; }
.signal-word { font-family: var(--font-display); font-size: 1.5rem; font-weight: 700; letter-spacing: 0.06em; color: var(--gold); }
.signal-sub  { font-family: var(--font-mono); font-size: 0.65rem; color: var(--muted); letter-spacing: 0.1em; }
```

---

## Data Table

```html
<table class="data-table">
  <thead><tr><th>Column</th><th>Value</th><th>Delta</th></tr></thead>
  <tbody>
    <tr><td>Row label</td><td class="mono">42.7</td><td class="positive mono">+3.2%</td></tr>
  </tbody>
</table>
```
```css
.data-table { width: 100%; border-collapse: collapse; font-size: 0.83rem; }
.data-table th { font-family: var(--font-mono); font-size: 0.62rem; letter-spacing: 0.1em; color: var(--muted); text-transform: uppercase; padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border); }
.data-table td { padding: 9px 12px; border-bottom: 1px solid var(--border); color: var(--muted2); }
.data-table tbody tr:nth-child(even) { background: rgba(255,255,255,0.02); }
.data-table tbody tr:hover { background: rgba(255,255,255,0.04); }
.mono { font-family: var(--font-mono); }
```

---

## Grid Layouts

```css
.grid-3 { display: grid; grid-template-columns: repeat(3,1fr); gap: 16px; margin-bottom: 16px; }
.grid-2 { display: grid; grid-template-columns: repeat(2,1fr); gap: 16px; margin-bottom: 16px; }
@media (max-width: 900px) { .grid-3 { grid-template-columns: repeat(2,1fr); } }
@media (max-width: 560px) { .grid-3, .grid-2 { grid-template-columns: 1fr; } }
```
