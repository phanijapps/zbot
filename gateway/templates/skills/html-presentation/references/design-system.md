# ARIA Design System Reference

## CSS Custom Properties

| Variable | Value | Usage |
|----------|-------|-------|
| `--cream` | `#faf9f5` | Light slide background, light text on dark |
| `--cream-dark` | `#f0eee6` | Subtle background variant, hover states |
| `--ink` | `#141413` | Dark slide background, heading text on light |
| `--ink-light` | `#3d3d3a` | Body text on light backgrounds |
| `--terracotta` | `#d97757` | Primary accent — labels, active states, progress bar |
| `--terracotta-deep` | `#c6613f` | Deeper terracotta for hover/emphasis |
| `--muted` | `#75869680` | Muted labels (50% opacity built-in) |
| `--sand` | `#e3dacc` | Borders, dividers, subtle separators |
| `--warm-gray` | `#b0aea5` | Secondary text, counters, metadata |
| `--sage` | `#bcd1ca` | Green accent — world/environment category |
| `--blush` | `#ebcece` | Pink accent — skill/comfort category |
| `--lavender` | `#cbcadb` | Purple accent — memory/state category |
| `--gold` | `#d4a27f` | Gold accent — plan mode, finish category |

## Font Variables

| Variable | Stack | Usage |
|----------|-------|-------|
| `--font-display` | `'Cormorant Garamond', Georgia, serif` | All headings (h1-h3), decorative numbers |
| `--font-body` | `'DM Sans', system-ui, sans-serif` | Body text, UI elements, version labels |
| `--font-mono` | `'JetBrains Mono', monospace` | Labels, pills, code, tables headers, tool names |

### Google Fonts URL
```
https://fonts.googleapis.com/css2?family=Cormorant+Garamond:ital,wght@0,300;0,400;0,500;0,600;0,700;1,300;1,400&family=DM+Sans:ital,opsz,wght@0,9..40,300;0,9..40,400;0,9..40,500;0,9..40,600;0,9..40,700;1,9..40,300;1,9..40,400&family=JetBrains+Mono:wght@300;400;500&display=swap
```

## Typography Scale

| Element | Font | Size | Weight | Line Height | Letter Spacing | Extra |
|---------|------|------|--------|-------------|----------------|-------|
| `h1` | display | 4.2rem | 400 | 1.15 | -.02em | — |
| `h2` | display | 3rem | 400 | 1.15 | -.01em | margin-bottom: .6em |
| `h3` | display | 1.9rem | 400 | 1.15 | — | margin-bottom: .4em |
| `p` | body | 1.25rem | 400 | 1.7 | — | max-width: 720px, color: ink-light |
| `.label` | mono | .98rem | 500 | — | .12em | uppercase, color: terracotta |
| `code` | mono | 1.25rem | 400 | — | — | — |
| `.subtitle` | body | 1.3rem | 400 | — | — | warm-gray, title slide only |

### Title Slide Overrides
- h1: 5.8rem, weight 300
- .label: terracotta
- .subtitle: warm-gray, 1.3rem, margin-top 12px
- .ver: font-body, weight 300, color terracotta

### Section Divider Overrides
- h1: 4rem, weight 300, white

## Spacing System

| Context | Value |
|---------|-------|
| Slide padding | 60px 80px (top/bottom, left/right) |
| Card padding | 28px |
| Card border-radius | 12px |
| Pill padding | 5px 16px |
| Pill border-radius | 20px |
| Code block padding | 24px 28px |
| Code block border-radius | 10px |
| Two-col gap | 60px |
| Three-col gap | 40px |
| Tag row gap | 8px |
| Divider | 48px wide, 2px tall, terracotta |
| Corner glyph font-size | 14rem |
| Grid lines | 80px x 80px |
| Progress bar height | 3px |

## Border Radius Values

| Element | Radius |
|---------|--------|
| Cards | 12px |
| Pills | 20px |
| Code blocks | 10px |
| Terminal | 12px |
| Flow nodes | 8px |
| Tooltip (code excerpts) | 6px |

## Decorative Elements

### Noise Texture
SVG data URI overlay at 35% opacity:
```css
.noise {
  position: absolute; inset: 0; pointer-events: none; opacity: .35;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.04'/%3E%3C/svg%3E");
}
```

### Grid Lines
80px grid with near-invisible lines:
```css
.grid-lines {
  position: absolute; inset: 0; pointer-events: none;
  background-image:
    linear-gradient(rgba(0,0,0,.02) 1px, transparent 1px),
    linear-gradient(90deg, rgba(0,0,0,.02) 1px, transparent 1px);
  background-size: 80px 80px;
}
/* Dark variant */
.slide--dark .grid-lines {
  background-image:
    linear-gradient(rgba(255,255,255,.02) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255,255,255,.02) 1px, transparent 1px);
}
```

### Corner Glyph
Large decorative character, positioned absolute:
```css
.corner-glyph {
  position: absolute;
  font-family: var(--font-display);
  font-size: 14rem;
  font-weight: 300;
  color: rgba(0,0,0,.025);
  line-height: 1;
  pointer-events: none;
}
.slide--dark .corner-glyph { color: rgba(255,255,255,.03) }
.corner-glyph--tr { top: -30px; right: -20px }
.corner-glyph--bl { bottom: -40px; left: -20px }
```

## Color Usage by Slide Variant

| Variant | Background | Text | Headings | Labels | Body Text |
|---------|-----------|------|----------|--------|-----------|
| Default | cream | ink | ink | terracotta | ink-light |
| Dark | ink | cream | cream | terracotta | warm-gray |
| Accent | terracotta | cream | cream (white) | rgba(255,255,255,.6) | rgba(255,255,255,.85) |
| Title | ink | cream | cream | terracotta | warm-gray |
| Image | (image) | cream | cream | terracotta | rgba(255,255,255,.8) |
