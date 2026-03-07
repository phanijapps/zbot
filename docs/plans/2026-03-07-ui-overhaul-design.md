# z-Bot UI Overhaul — "Warm Sand" Design

## Direction
- **Aesthetic**: Notion-inspired — warm, approachable, clean
- **Personality**: Warm & Intelligent — trusted companion on your desktop
- **Density**: Comfortable & Organized — balanced, information at a glance
- **Typography**: System fonts (native feel, zero load time)
- **Sidebar**: Unified with page (same tone family, subtle tint difference)
- **Mode**: Dark mode priority, light mode supported, user-configurable

## Color System

### Dark Mode (Primary)

```css
/* Background layers */
--background:        #191919;    /* app background */
--card:              #201F1D;    /* cards, panels */
--popover:           #252422;    /* dropdowns, tooltips */
--sidebar:           #161614;    /* sidebar, slightly darker */

/* Text */
--foreground:        #E8E4DF;    /* primary text */
--muted-foreground:  #9B9689;    /* secondary text */

/* Primary accent (amber/copper) */
--primary:           #D4945A;
--primary-hover:     #E0A46C;
--primary-foreground:#1A1714;
--primary-muted:     rgba(212, 148, 90, 0.12);

/* Secondary */
--secondary:         #252422;
--secondary-hover:   #2E2D2A;
--secondary-foreground: #E8E4DF;

/* Muted */
--muted:             #252422;

/* Accent */
--accent:            rgba(212, 148, 90, 0.08);
--accent-foreground: #D4945A;

/* Selection */
--selection:         rgba(212, 148, 90, 0.15);
--selection-border:  #D4945A;

/* Semantic */
--destructive:       #E5534B;
--destructive-hover: #F06560;
--destructive-foreground: #FFFFFF;
--destructive-muted: rgba(229, 83, 75, 0.12);

--success:           #6BC46D;
--success-hover:     #7DD47F;
--success-foreground: #FFFFFF;
--success-muted:     rgba(107, 196, 109, 0.12);

--warning:           #D4A04A;
--warning-hover:     #E0B05C;
--warning-foreground: #FFFFFF;
--warning-muted:     rgba(212, 160, 74, 0.12);

/* Borders */
--border:            #2E2E2B;
--input:             #2E2E2B;
--input-background:  #1E1D1B;

/* Focus */
--ring:              #D4945A;
--ring-muted:        rgba(212, 148, 90, 0.25);

/* Overlay */
--overlay:           rgba(0, 0, 0, 0.6);

/* Sidebar (unified, slightly darker) */
--sidebar-foreground:          #E8E4DF;
--sidebar-primary:             #D4945A;
--sidebar-primary-foreground:  #1A1714;
--sidebar-accent:              #1E1D1B;
--sidebar-accent-hover:        rgba(30, 29, 27, 0.5);
--sidebar-border:              #2E2E2B;
--sidebar-ring:                #D4945A;
--sidebar-muted:               #807A70;

/* Charts */
--chart-1: #D4945A;
--chart-2: #6BC46D;
--chart-3: #D4A04A;
--chart-4: #C47070;
--chart-5: #6BA3C4;
```

### Light Mode

```css
--background:        #F7F5F2;
--card:              #FFFFFF;
--popover:           #FFFFFF;
--sidebar:           #EFECE7;

--foreground:        #37352F;
--muted-foreground:  #787570;

--primary:           #C17D3F;
--primary-hover:     #A86A30;
--primary-foreground:#FFFFFF;
--primary-muted:     rgba(193, 125, 63, 0.08);

--secondary:         #F0EDE8;
--secondary-hover:   #E3DFD8;
--secondary-foreground: #787570;

--muted:             #F0EDE8;

--accent:            rgba(193, 125, 63, 0.06);
--accent-foreground: #C17D3F;

--selection:         rgba(193, 125, 63, 0.12);
--selection-border:  #C17D3F;

--destructive:       #CC4040;
--destructive-hover: #B83636;
--destructive-foreground: #FFFFFF;
--destructive-muted: rgba(204, 64, 64, 0.08);

--success:           #4EA04E;
--success-hover:     #429042;
--success-foreground: #FFFFFF;
--success-muted:     rgba(78, 160, 78, 0.08);

--warning:           #B8892E;
--warning-hover:     #A07824;
--warning-foreground: #FFFFFF;
--warning-muted:     rgba(184, 137, 46, 0.08);

--border:            #E3DFD8;
--input:             #E3DFD8;
--input-background:  #FFFFFF;

--ring:              #C17D3F;
--ring-muted:        rgba(193, 125, 63, 0.2);

--overlay:           rgba(0, 0, 0, 0.4);

--sidebar-foreground:          #37352F;
--sidebar-primary:             #C17D3F;
--sidebar-primary-foreground:  #FFFFFF;
--sidebar-accent:              #E8E4DC;
--sidebar-accent-hover:        rgba(232, 228, 220, 0.5);
--sidebar-border:              #E3DFD8;
--sidebar-ring:                #C17D3F;
--sidebar-muted:               #9B9689;

--chart-1: #C17D3F;
--chart-2: #4EA04E;
--chart-3: #B8892E;
--chart-4: #B85C5C;
--chart-5: #5C8EB8;
```

## Component Design Changes

### Sidebar
- **Unified tone**: Same background family as page, slightly tinted darker
- **Active state**: Subtle background highlight with left accent bar (amber), NOT solid color fill
- **Nav links**: Lighter text, no icon opacity tricks — just muted vs foreground color
- **Group labels**: Smaller, uppercase, extra letter-spacing (keep current)
- **Logo**: Keep current placement, ensure it works on both light/dark backgrounds
- **Hover**: Gentle background tint, not color change
- **Border**: Right border separating sidebar from content (subtle)

```css
.nav-link--active {
  color: var(--primary);
  background-color: var(--primary-muted);
  /* No solid fill — subtle highlight with accent text */
}
```

### Cards
- **Remove shadows in dark mode** — use subtle borders instead (Notion pattern)
- **Keep subtle shadows in light mode**
- **Interactive hover**: Gentle border color change, NOT translateY (too bouncy for a dashboard)
- **Stat cards**: Warmer icon backgrounds using primary-muted

```css
/* Dark mode: border-based cards */
.dark .card {
  box-shadow: none;
  border: 1px solid var(--border);
}
```

### Buttons
- **Primary**: Amber/copper fill, dark text
- **Secondary**: Muted background, standard text
- **Ghost**: No background, muted text, subtle hover
- **Destructive**: Softer red (not neon)
- **Border radius**: Keep `--radius-md` (8px) — not too round, not sharp

### Forms
- **Input backgrounds**: Slightly darker than card in dark mode
- **Focus ring**: Amber glow instead of purple
- **Select dropdowns**: Match new color scheme
- **Checkboxes**: Replace native checkboxes with styled toggles where used (settings page)

### Badges
- **Muted backgrounds**: Using semantic color with low opacity
- **Border radius**: `--radius-sm` (6px) — keep compact

### Chat Messages
- **User messages**: Primary (amber) bubble with dark text
- **Assistant messages**: Card background with subtle border
- **Code blocks**: Slightly darker background than card
- **Selection color**: Amber-tinted

### Modals
- **Backdrop**: Darker overlay in dark mode
- **Card-like appearance**: Rounded corners, border in dark mode
- **Shadow**: Keep in light mode, remove in dark mode

### Chat Slider
- **Backdrop**: Match new overlay color
- **Handle**: Warm tones matching sidebar
- **Border**: Left border using --border color

### Empty States
- **Icon container**: Warm muted background
- **Softer messaging**: Same pattern, just recolored

### Loading
- **Spinner**: Amber colored (primary)

### Alerts
- **Same pattern**: Semantic muted backgrounds, just recolored to new palette

## Typography Refinements
- Keep system font stack
- **Base size**: 15px (keep current — good for dashboard)
- **Heading weight**: 600 (keep)
- **Body text**: Regular weight, 1.5 line-height
- **Letter spacing on headings**: Keep -0.025em for tighter headings
- **Text selection**: Amber highlight instead of purple

## Shadows
```css
/* Light mode */
--shadow-card: 0 1px 2px rgba(55, 53, 47, 0.04), 0 3px 8px rgba(55, 53, 47, 0.04);
--shadow-card-hover: 0 2px 6px rgba(55, 53, 47, 0.06), 0 6px 16px rgba(55, 53, 47, 0.06);
--shadow-modal: 0 4px 16px rgba(55, 53, 47, 0.1), 0 12px 32px rgba(55, 53, 47, 0.12);
--shadow-dropdown: 0 4px 16px rgba(55, 53, 47, 0.08);

/* Dark mode — no shadows, use borders */
.dark {
  --shadow-card: none;
  --shadow-card-hover: none;
  --shadow-modal: 0 4px 24px rgba(0, 0, 0, 0.4);
  --shadow-dropdown: 0 4px 16px rgba(0, 0, 0, 0.3);
}
```

## Transitions
- All transitions: `0.15s ease` (keep current — snappy, not sluggish)
- No `translateY` on card hover (remove bouncy effect)
- Sidebar nav: Background color transition only

## Cleanup Tasks
1. Remove all inline `style={}` from App.tsx (Settings panel)
2. Remove hardcoded colors from ConnectionStatus.tsx
3. Remove hardcoded dark theme from GenerativeCanvas.tsx
4. Remove old HSL color system if present in index.css
5. Update `::selection` color from purple to amber
6. Remove `.list-item--active` hardcoded `#f3e8ff` and `#9333ea`
7. Ensure Toaster component respects theme (dark/light)

## Files to Modify

| File | Changes |
|------|---------|
| `styles/theme.css` | Full color overhaul, shadow changes, selection color |
| `styles/components.css` | Sidebar unified style, card dark-mode borders, nav active state, list-item active, hover tweaks |
| `App.tsx` | Extract inline styles to CSS classes, fix Toaster theme |
| `components/ConnectionStatus.tsx` | Replace hardcoded Tailwind colors with CSS variables |
| `features/agent/GenerativeCanvas.tsx` | Replace hardcoded dark theme with CSS variables |
| `shared/ui/button.tsx` | Verify CVA variants work with new colors (likely no change needed) |
| `components/ThemeToggle.tsx` | Verify works with unified sidebar |

## What NOT to Change
- Component structure / React code (unless removing inline styles)
- Routing
- API/transport layer
- Feature functionality
- BEM naming conventions
- Radix UI primitives
- File structure
