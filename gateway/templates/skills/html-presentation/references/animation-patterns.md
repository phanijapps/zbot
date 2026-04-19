# ARIA Animation Patterns

Advanced animation patterns beyond the basic `fadeUp` and `fadeIn` used in the base CSS. Use these when slides need dynamic visual storytelling — process flows, timelines, technical diagrams, etc.

## Base Animations (already in css-template.md)

```css
@keyframes fadeUp   { from{opacity:0;transform:translateY(24px)} to{opacity:1;transform:translateY(0)} }
@keyframes fadeIn   { from{opacity:0} to{opacity:1} }
@keyframes termFadeIn { from{opacity:0;transform:translateY(10px)} to{opacity:1;transform:translateY(0)} }
@keyframes blink    { 0%,100%{opacity:1} 50%{opacity:0} }
```

## Rail Draw (clip-path reveal)

Horizontal line that draws from left to right. Great for timelines, process flows.

```css
/* Background track (always visible, low opacity) */
.rail-bg { opacity: .18; }

/* Active track that draws in */
.rail-active { clip-path: inset(0 100% 0 0); }

@keyframes railDraw {
  from { clip-path: inset(0 100% 0 0); }
  to   { clip-path: inset(0 0% 0 0); }
}
.slide.active .rail-active {
  animation: railDraw .8s 0s cubic-bezier(.4,0,.2,1) both;
}
```

**HTML:**
```html
<!-- Background rail -->
<div class="diagram-el rail rail-bg" style="position:absolute;top:180px;left:60px;width:1140px;height:6px;background:var(--terracotta);border-radius:3px"></div>
<!-- Active rail (animates) -->
<div class="diagram-el rail rail-active" style="position:absolute;top:180px;left:60px;width:1140px;height:6px;background:var(--terracotta);border-radius:3px"></div>
```

### Rail Dim (during focus shift)
```css
@keyframes railDim {
  0%,24%  { opacity: 1; }
  28%     { opacity: .2; }
  68%     { opacity: .2; }
  72%     { opacity: 1; }
  100%    { opacity: 1; }
}
```

## Node Appear (spring scale)

Circles/nodes that pop in with a spring easing. Use on timeline nodes, graph points.

```css
@keyframes nodeAppear {
  from { transform: scale(0); opacity: 0; }
  to   { transform: scale(1); opacity: 1; }
}

/* Stagger per node */
.slide.active .node--1 { animation: nodeAppear .3s .3s cubic-bezier(.34,1.56,.64,1) both; }
.slide.active .node--2 { animation: nodeAppear .3s .5s cubic-bezier(.34,1.56,.64,1) both; }
.slide.active .node--3 { animation: nodeAppear .3s .7s cubic-bezier(.34,1.56,.64,1) both; }
```

**HTML:**
```html
<div class="diagram-el node node--1" style="position:absolute;top:172px;left:72px;width:22px;height:22px;border-radius:50%;background:var(--cream);border:3px solid var(--terracotta);z-index:3"></div>
```

### End Node Pulse
```css
@keyframes endPulse {
  0%,88% { box-shadow: 0 0 0 0 rgba(217,119,87,.4); }
  92%    { box-shadow: 0 0 0 10px rgba(217,119,87,0); }
  100%   { box-shadow: 0 0 0 0 rgba(217,119,87,0); }
}
/* Combine with nodeAppear */
.slide.active .node--end {
  animation: nodeAppear .3s .7s cubic-bezier(.34,1.56,.64,1) both,
             endPulse 10s 0s linear both;
}
```

## Runner Dot Path

An animated dot that follows a multi-point path. Perfect for showing data flow, process traversal.

```css
.runner {
  position: absolute;
  width: 14px; height: 14px;
  border-radius: 50%;
  background: var(--terracotta);
  z-index: 10;
  opacity: 0;
}

@keyframes runnerPath {
  0%   { left:76px;  top:176px; opacity:0; background:var(--terracotta); }
  9%   { left:76px;  top:176px; opacity:1; }
  22%  { left:394px; top:176px; opacity:1; }
  30%  { left:394px; top:260px; opacity:1; }
  35%  { left:394px; top:385px; opacity:1; background:var(--gold); }
  /* Color changes to gold when entering sub-process */
  56%  { left:734px; top:385px; opacity:1; background:var(--gold); }
  70%  { left:734px; top:176px; opacity:1; background:var(--terracotta); }
  82%  { left:1170px;top:176px; opacity:1; }
  100% { left:1170px;top:176px; opacity:1; }
}

.slide.active .runner {
  animation: runnerPath 9s .8s linear both;
}
```

**Tips:**
- Use percentage keyframes to control timing of each segment
- Change `background` at keyframes to indicate context change
- Use `linear` timing for predictable movement
- Long duration (7-10s) for complex paths

## Connector Draw (vertical lines)

Dashed vertical connectors that grow in height. Great for showing relationships between parallel tracks.

```css
.connector {
  position: absolute;
  width: 2px; height: 0;
  z-index: 1;
  border: none;
  border-left: 2px dashed var(--gold);
}

@keyframes connectorDraw {
  from { height: 0; }
  to   { height: 76px; }
}

.slide.active .connector--down {
  animation: connectorDraw .4s 2.0s cubic-bezier(.4,0,.2,1) both;
}
```

## Box Lifecycle (scale + opacity timeline)

A container that appears, stays, then dims. Used for transient sub-processes.

```css
@keyframes childBoxLife {
  0%,22%  { transform: scale(.3); opacity: 0; }
  25%     { transform: scale(1.05); opacity: 1; }
  27%     { transform: scale(1); opacity: 1; }
  72%     { transform: scale(1); opacity: 1; }
  78%     { transform: scale(1); opacity: .35; }
  100%    { transform: scale(1); opacity: .35; }
}
```

## Glow Pulse

Box-shadow glow that pulses when a runner dot visits an element.

```css
@keyframes glowPulse {
  0%,33%  { box-shadow: 0 2px 8px rgba(0,0,0,.04); }
  37%     { box-shadow: 0 0 20px 5px rgba(212,162,127,.5), 0 2px 8px rgba(0,0,0,.04); }
  43%     { box-shadow: 0 2px 8px rgba(0,0,0,.04); }
  100%    { box-shadow: 0 2px 8px rgba(0,0,0,.04); }
}

/* Time the glow to match runner arrival */
.slide.active .tool-card--1 {
  animation: fadeUp .4s 2.7s both, glowPulse 9s .8s linear both;
}
```

## Dot Pulse (ellipsis animation)

Three dots that pulse in sequence. Shows "processing" or "loading".

```css
@keyframes dotPulse {
  0%,100% { opacity: .3; transform: scale(.8); }
  50%     { opacity: 1; transform: scale(1.3); }
}

.ellipsis .dot {
  width: 12px; height: 12px;
  border-radius: 50%;
  background: var(--gold);
  animation: dotPulse .9s ease-in-out infinite;
}
.ellipsis .dot:nth-child(2) { animation-delay: .2s; }
.ellipsis .dot:nth-child(3) { animation-delay: .4s; }
```

## Terminal Typing (staggered termFadeIn)

Lines that appear one by one, simulating terminal output.

```css
/* Already in base CSS */
@keyframes termFadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to   { opacity: 1; transform: translateY(0); }
}

/* Apply with staggered delays */
.slide.active .term-line { animation: termFadeIn .45s ease both; }
```

**Stagger via inline style:**
```html
<div class="term-line" style="animation-delay:.3s">Line 1</div>
<div class="term-line" style="animation-delay:.8s">Line 2</div>
<div class="term-line" style="animation-delay:1.3s">Line 3</div>
```

Increment by ~0.5s per line for natural typing speed.

## Cursor Blink

Blinking block cursor for terminal last line. Already in base CSS:
```css
.term-line:last-child::after {
  content: '\u258C';
  color: var(--terracotta);
  animation: blink 1s step-end infinite;
  animation-delay: 3s; /* start after lines finish typing */
}
```

## Ping Ring (expanding border ring)

A ring that expands outward from a point, then fades. Used on status indicators.

```css
@keyframes pingRing {
  0%   { transform: scale(.8); opacity: .7; }
  100% { transform: scale(2.2); opacity: 0; }
}

.indicator::before {
  content: '';
  position: absolute; inset: -2px;
  border-radius: 50%;
  border: 1px solid currentColor;
  opacity: 0;
  animation: pingRing 2s ease-out infinite;
}
```

## Swimlane Pattern

Multiple parallel tracks with runner dots crossing between them via vertical connectors. Combine:

1. **Rails** (horizontal) with `railDraw` for each lane
2. **Nodes** with `nodeAppear` at intersection points
3. **Connectors** (vertical dashed) with `connectorDraw` between lanes
4. **Runner dots** with custom path keyframes per lane
5. **Highlighted segments** with `fadeIn` to show active portions

**Lane structure:**
```html
<div class="diagram-editable" style="position:relative;width:1100px;height:310px;margin:0 auto">
  <!-- Lane A (top) -->
  <div class="diagram-el" style="position:absolute;left:0;top:50px;width:110px;font-family:var(--font-mono);font-size:.72rem;font-weight:600;text-align:right">AGENT A</div>
  <div class="diagram-el rail-a rail-bg" style="position:absolute;left:120px;top:54px;width:940px;height:4px;background:var(--ink);opacity:.12;border-radius:2px"></div>
  <div class="diagram-el rail-a rail-active" style="position:absolute;left:120px;top:54px;width:940px;height:4px;background:var(--ink);border-radius:2px;clip-path:inset(0 100% 0 0)"></div>

  <!-- Middle lane (GM/Process) -->
  <!-- ... same pattern ... -->

  <!-- Lane B (bottom) -->
  <!-- ... same pattern ... -->

  <!-- Vertical connectors between lanes -->
  <div class="diagram-el connector connector--down" style="position:absolute;left:389px;top:62px;width:2px;height:0;border-left:2px dashed var(--terracotta)"></div>

  <!-- Runner dots -->
  <div class="diagram-el runner runner--a" style="position:absolute;width:12px;height:12px;border-radius:50%;background:var(--ink);z-index:10;opacity:0"></div>
</div>
```

## Combining Animations

You can combine multiple animations on one element:
```css
.slide.active .element {
  animation:
    fadeUp .4s 2.7s both,           /* entrance */
    glowPulse 9s .8s linear both;   /* recurring effect */
}
```

**Timing tips:**
- Start complex animations 0.5-1s after slide entry to let fadeUp complete
- Runner dots: 7-10s duration for complex paths
- Node appears: stagger by 0.2-0.3s between nodes
- Rail draw: 0.8-1.2s duration with cubic-bezier(.4,0,.2,1)
- Use `both` fill mode to hold final state
