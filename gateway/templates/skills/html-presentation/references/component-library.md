# ARIA Component Library

Reusable HTML component patterns. All components work on both light and dark slides (dark variants are handled by CSS automatically when placed inside `.slide--dark`).

## Cards

### Basic Card
```html
<div class="card">
  <h3>Card Title</h3>
  <p style="font-size:1rem">Card body text here.</p>
</div>
```

### Card with Accent Border
```html
<div class="card" style="border-top:3px solid var(--terracotta)">
  <h3>Card Title</h3>
  <p style="font-size:1rem">Highlighted card content.</p>
</div>
```

Color options for border-top: `--terracotta`, `--sage`, `--blush`, `--lavender`, `--gold`, `--ink`.

### Card with Left Border
```html
<div class="card" style="border-left:3px solid var(--sage);padding:18px 22px">
  <div style="font-family:var(--font-mono);font-size:.92rem;font-weight:600;color:var(--terracotta);margin-bottom:8px">function_name()</div>
  <p style="font-size:.78rem;color:var(--warm-gray);margin-bottom:10px">Description text</p>
  <div style="font-family:var(--font-mono);font-size:.75rem;color:var(--ink-light);line-height:1.7;background:rgba(0,0,0,.03);padding:10px 14px;border-radius:6px">
    Content body
  </div>
</div>
```

### Compact Card (for lists)
```html
<div class="card" style="padding:14px 18px;display:flex;align-items:center;gap:12px">
  <span class="pill pill--terra" style="font-size:.76rem;min-width:96px;text-align:center">Label</span>
  <span style="color:var(--warm-gray);font-size:.95rem">Description</span>
</div>
```

## Pills

### All Pill Variants
```html
<span class="pill pill--terra">Terracotta</span>
<span class="pill pill--sage">Sage</span>
<span class="pill pill--blush">Blush</span>
<span class="pill pill--lavender">Lavender</span>
<span class="pill pill--gold">Gold</span>
<span class="pill pill--dark">Dark</span>
<span class="pill pill--outline">Outline</span>
```

### Small Pill
```html
<span class="pill pill--terra" style="font-size:.72rem">Small</span>
```

### Tag Row (group of pills)
```html
<div class="tag-row">
  <span class="pill pill--terra" style="font-size:.76rem">Tag 1</span>
  <span class="pill pill--sage" style="font-size:.76rem">Tag 2</span>
  <span class="pill pill--outline" style="font-size:.76rem">Tag 3</span>
</div>
```

## Feature Lists

### Bullet List
```html
<ul class="feature-list">
  <li>First feature or point</li>
  <li>Second feature or point</li>
  <li>Third feature or point</li>
</ul>
```

Works on light, dark, and accent slides automatically.

## Tables

### Clean Table
```html
<table class="clean-table">
  <thead>
    <tr>
      <th>Column A</th>
      <th>Column B</th>
      <th>Column C</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>Value 1</td>
      <td>Value 2</td>
      <td>Value 3</td>
    </tr>
    <tr>
      <td>Value 4</td>
      <td>Value 5</td>
      <td>Value 6</td>
    </tr>
  </tbody>
</table>
```

## Code Blocks

### Syntax-Highlighted Code
```html
<div class="code-block">
<span class="kw">function</span> processData(input) {
  <span class="cmt">// Transform the input</span>
  <span class="kw">const</span> result = input.<span class="kw">map</span>(item => {
    <span class="kw">return</span> {
      name: item.name,
      value: <span class="num">42</span>,
      label: <span class="str">"processed"</span>
    };
  });
  <span class="kw">return</span> result;
}</div>
```

**Syntax classes:**
- `.kw` — keywords (terracotta)
- `.str` — strings (sage)
- `.cmt` — comments (warm-gray)
- `.num` — numbers (gold)

## Terminal UI

### Terminal with Typing Animation
```html
<div class="terminal">
  <div class="terminal-bar">
    <div class="terminal-dot terminal-dot--red"></div>
    <div class="terminal-dot terminal-dot--yellow"></div>
    <div class="terminal-dot terminal-dot--green"></div>
  </div>
  <div class="terminal-body">
    <div class="term-line" style="animation-delay:.3s">$ npm install aria-sdk</div>
    <div class="term-line" style="animation-delay:.8s"><span style="color:#28c840">+</span> added 42 packages</div>
    <div class="term-line" style="animation-delay:1.3s">$ npm run build</div>
    <div class="term-line" style="animation-delay:1.8s"><span style="color:var(--terracotta)">✓</span> Build complete (2.1s)</div>
    <div class="term-line" style="animation-delay:2.3s">$ _</div>
  </div>
</div>
```

**Notes:**
- Each `.term-line` uses `termFadeIn` animation with staggered delays
- Last line automatically gets a blinking cursor via CSS `::after`
- Set `animation-delay` on each line for staggered typing effect

### Pipeline Terminal (larger, scrollable)
```html
<div style="flex:1;background:#0a0a0a;overflow:hidden;display:flex;flex-direction:column;margin-top:8px;border:2px solid var(--ink)">
  <div style="display:flex;align-items:center;justify-content:space-between;padding:8px 14px;border-bottom:2px solid var(--ink);background:#0f0f0f;flex-shrink:0">
    <span style="font-family:var(--font-mono);font-size:.72rem;font-weight:600;color:#aaa;letter-spacing:.08em;text-transform:uppercase">Terminal Title</span>
    <span style="width:6px;height:6px;background:var(--terracotta);display:inline-block"></span>
  </div>
  <div style="flex:1;overflow-y:auto;padding:16px 20px">
    <!-- Terminal content lines -->
  </div>
</div>
```

## Number Callouts

### Large Number
```html
<div class="num-big">24</div>
<p>Tools available in the system</p>
```

### Number with Context (corner glyph style)
```html
<div class="corner-glyph corner-glyph--tr">24</div>
```

## Dividers

### Standard Divider
```html
<div class="divider"></div>
```

### Light Divider (for dark/accent slides)
```html
<div class="divider--light" style="width:48px;height:2px;margin:20px 0"></div>
```

### Centered Divider
```html
<div style="width:48px;height:2px;background:var(--terracotta);margin:24px auto"></div>
```

## Flow Arrows

### Between Elements
```html
<div class="flow-arrow">&darr;</div>
```
or
```html
<div class="flow-arrow">&rarr;</div>
```

## Flow Nodes

### Horizontal Flow
```html
<div style="display:flex;align-items:center;gap:16px">
  <div class="flow-node flow-node--primary">Input</div>
  <span class="flow-connector">&rarr;</span>
  <div class="flow-node flow-node--secondary">Process</div>
  <span class="flow-connector">&rarr;</span>
  <div class="flow-node flow-node--outline">Output</div>
</div>
```

## Tool Items

### Tool Name + Description List
```html
<div class="tool-item">
  <span class="tool-name">function_name</span>
  <span class="tool-desc">Description of what this function does</span>
</div>
<div class="tool-item">
  <span class="tool-name">another_fn</span>
  <span class="tool-desc">Another description here</span>
</div>
```

## Mode Circles

### Circular Mode Layout
```html
<div style="display:flex;justify-content:center;gap:60px;align-items:center">
  <div style="display:flex;flex-direction:column;align-items:center;gap:18px">
    <div style="width:180px;height:180px;border-radius:50%;background:var(--terracotta);display:flex;align-items:center;justify-content:center">
      <span style="font-family:var(--font-mono);font-size:1.2rem;font-weight:700;color:#fff">Mode A</span>
    </div>
    <span style="font-size:1.05rem;color:var(--ink-light);text-align:center">Description</span>
  </div>
  <!-- Repeat for each mode -->
</div>
```

### Circle with Outer Ring (emphasized)
```html
<div style="width:220px;height:220px;border-radius:50%;border:3px solid var(--gold);display:flex;align-items:center;justify-content:center;padding:7px">
  <div style="width:100%;height:100%;border-radius:50%;background:var(--gold);display:flex;align-items:center;justify-content:center">
    <span style="font-family:var(--font-mono);font-size:1.2rem;font-weight:700;color:#fff;text-align:center">Plan</span>
  </div>
</div>
```

## 2x2 Mode Grid

```html
<div class="mode-grid">
  <div class="mode-card">
    <div style="display:flex;align-items:center;gap:8px">
      <span class="pill pill--terra" style="font-size:.72rem">Mode A</span>
    </div>
    <div class="mode-excerpt">
      Code or excerpt here
    </div>
    <div class="mode-output">
      Description of the output
    </div>
  </div>
  <!-- Repeat for each mode -->
</div>
```

**CSS for mode-grid (add to slide-specific styles):**
```css
.mode-grid{display:grid;grid-template-columns:1fr 1fr;gap:20px;flex:1}
.mode-card{background:rgba(0,0,0,.03);border:1px solid rgba(0,0,0,.06);border-radius:12px;padding:22px 24px;display:flex;flex-direction:column;gap:10px}
.mode-card .mode-excerpt{font-family:var(--font-mono);font-size:.82rem;color:var(--ink-light);line-height:1.5;background:rgba(0,0,0,.03);border-radius:6px;padding:10px 14px}
.mode-card .mode-output{font-size:.88rem;color:var(--warm-gray)}
```

## Tool Category Grid

```html
<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:16px">
  <div class="card" style="padding:20px">
    <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
      <span class="pill pill--terra" style="font-size:.72rem">Category</span>
      <span style="font-size:.88rem;color:var(--warm-gray)">N tools</span>
    </div>
    <div style="font-family:var(--font-mono);font-size:.82rem;color:var(--ink-light);line-height:1.6;margin-top:8px">
      tool_one<br>tool_two<br>tool_three
    </div>
  </div>
  <!-- Repeat for each category -->
</div>
```

## Message Blocks (Chat/Pipeline Style)

### Role-labeled message
```html
<div class="msg-block" style="animation-delay:Ns">
  <div class="msg-role msg-role--system">SYSTEM</div>
  <div class="msg-line"><span class="hi">Bold text</span> normal text <span class="accent">highlighted</span></div>
</div>
```

**Role variants:** `msg-role--system` (green), `msg-role--user` (orange), `msg-role--assistant` (teal), `msg-role--tool` (gold).

**Text highlight classes:** `.hi` (white/bright), `.dim` (muted), `.accent` (orange), `.accent2` (teal), `.accent3` (lavender).
