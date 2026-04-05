# Aesthetics Reference

Full menu of fonts, palettes, and aesthetic modes for premium reports.

---

## Aesthetic Modes

### 1. Dark Terminal-Editorial (default)
Best for: finance, analytics, engineering, research
- Background: `#06080f` → `#0d1220`
- Accent: teal/green (`#00ffaa`)
- Feel: Bloomberg terminal meets Wired magazine

### 2. Dark Luxury
Best for: executive briefings, premium brand reports
- Background: `#080608` → `#110f14`
- Accent: gold (`#d4a853`)
- Secondary: warm white (`#f5f0e8`)
- Feel: annual report for a luxury house

### 3. Dark Industrial
Best for: engineering, ops, infrastructure
- Background: `#080c0f` → `#0f1820`
- Accent: electric blue (`#00aaff`)
- Secondary: amber (`#ff9500`)
- Feel: monitoring dashboard, control room

### 4. Dark Botanical
Best for: health, environment, wellness
- Background: `#070d0a` → `#0d1a12`
- Accent: mint (`#4fffb0`)
- Secondary: lavender (`#a78bfa`)
- Warn: coral (`#ff6b35`)
- Feel: medical journal meets nature documentary

### 5. Dark Academic
Best for: research papers, scientific data, AI/ML
- Background: `#08090f` → `#10111e`
- Accent: violet (`#c084fc`)
- Secondary: sky (`#38bdf8`)
- Feel: arXiv meets Notion dark mode

---

## Font Pairings

| Display | Mono | Body | Mood |
|---|---|---|---|
| Syne | DM Mono | DM Sans | Techy editorial (default) |
| Barlow Condensed | JetBrains Mono | Barlow | Industrial bold |
| Fraunces | IBM Plex Mono | IBM Plex Sans | Academic warm |
| Bebas Neue | Fira Code | Lato | Brutalist data |
| Cabinet Grotesk | Space Mono | Cabinet Grotesk | Modern luxury |
| Playfair Display | Courier Prime | Source Sans 3 | Finance editorial |

Google Fonts `<link>` template:
```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Syne:wght@400;500;600;700;800&family=DM+Mono:wght@300;400;500&family=DM+Sans:ital,wght@0,300;0,400;0,500;1,300&display=swap" rel="stylesheet">
```

---

## Accent Color Quick Reference

| Color name | Hex | Use for |
|---|---|---|
| Teal | `#00ffaa` | Positive, go, growth |
| Electric blue | `#00c9ff` | Secondary highlight |
| Gold | `#ffd166` | Neutral, caution, premium |
| Coral red | `#ff4d6d` | Negative, alert, risk |
| Violet | `#c084fc` | Research, AI, creative |
| Amber | `#ff9500` | Ops, warning (non-critical) |
| Mint | `#4fffb0` | Health, environment |
| Sky | `#38bdf8` | Analytics, data |

---

## Noise Overlay SVG Data URI

```css
body::before {
  content: '';
  position: fixed;
  inset: 0;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.03'/%3E%3C/svg%3E");
  opacity: 0.4;
  pointer-events: none;
  z-index: 0;
}
```
