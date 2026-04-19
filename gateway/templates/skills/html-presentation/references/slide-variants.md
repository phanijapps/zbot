# ARIA Slide Variant Templates

HTML templates for each slide type. Copy and adapt for each slide in your presentation.

## 1. Title Slide

Dark background, centered, large serif heading. Use for the opening slide.

```html
<div class="slide slide--title active" data-slide="0">
  <div class="noise"></div>
  <div class="corner-glyph corner-glyph--tr" style="font-size:28rem;top:-80px;right:-40px;color:rgba(255,255,255,.02)">A</div>
  <div class="anim-1">
    <span class="label">LABEL TEXT &middot; 2026</span>
  </div>
  <h1 class="anim-2" style="margin-top:16px">Presentation Title</h1>
  <p class="subtitle anim-3">Subtitle or tagline goes here</p>
  <div style="margin-top:32px" class="anim-4">
    <span style="color:var(--cream);font-size:1.08rem;font-weight:500">Author Name</span>
    <span style="color:var(--warm-gray);font-size:1.1rem;margin-left:8px">&mdash; Team / Org</span>
  </div>
</div>
```

**Notes:**
- First slide gets `active` class (only this slide)
- Corner glyph character can be any letter/number
- The `.ver` class can style version numbers: `<span class="ver">v 2.0</span>`

## 2. Section Divider (Accent)

Terracotta background, centered. Use to separate major sections.

```html
<div class="slide slide--accent" data-slide="N" style="justify-content:center;align-items:center;text-align:center">
  <div class="noise"></div>
  <span class="label anim-1" style="color:rgba(255,255,255,.6)">Section 01</span>
  <h1 class="anim-2" style="font-size:4rem;font-weight:300;color:#fff;margin-top:12px">Section Title</h1>
  <div class="anim-3" style="width:48px;height:2px;background:rgba(255,255,255,.4);margin:20px auto"></div>
</div>
```

**Notes:**
- `style="justify-content:center;align-items:center;text-align:center"` overrides default slide alignment
- Section number in the label (Section 01, 02, etc.)

## 3. Content Slide (Light Background)

Default cream background. The most common slide type.

### Single Column
```html
<div class="slide" data-slide="N">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="anim-3">
    <!-- Content here: paragraphs, lists, cards, etc. -->
  </div>
</div>
```

### Two Column
```html
<div class="slide" data-slide="N">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="two-col anim-3">
    <div>
      <!-- Left column -->
    </div>
    <div>
      <!-- Right column -->
    </div>
  </div>
</div>
```

### Three Column
```html
<div class="slide" data-slide="N">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="three-col anim-3">
    <div class="card"><!-- Col 1 --></div>
    <div class="card"><!-- Col 2 --></div>
    <div class="card"><!-- Col 3 --></div>
  </div>
</div>
```

### Cards Grid
```html
<div class="slide" data-slide="N">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="three-col anim-3">
    <div class="card" style="border-top:3px solid var(--terracotta)">
      <h3>Card Title</h3>
      <p style="font-size:1rem">Card content</p>
    </div>
    <div class="card" style="border-top:3px solid var(--sage)">
      <h3>Card Title</h3>
      <p style="font-size:1rem">Card content</p>
    </div>
    <div class="card" style="border-top:3px solid var(--lavender)">
      <h3>Card Title</h3>
      <p style="font-size:1rem">Card content</p>
    </div>
  </div>
</div>
```

## 4. Dark Slide

Dark background with cream/warm-gray text. Use for emphasis or technical content.

```html
<div class="slide slide--dark" data-slide="N">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <div class="corner-glyph corner-glyph--tr">X</div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="anim-3">
    <!-- Content here -->
  </div>
</div>
```

**Notes:**
- Cards automatically get dark variant styling
- Feature lists, tool items, flow arrows all have dark variants
- Code blocks use a lighter background variant

## 5. Image Slide

Full-bleed background image with gradient overlay. Content positioned at bottom.

```html
<div class="slide slide--image" data-slide="N">
  <img src="data:image/jpeg;base64,..." style="position:absolute;inset:0;width:100%;height:100%;object-fit:cover" alt="">
  <div class="image-overlay"></div>
  <div class="slide-content">
    <span class="label anim-1">Category</span>
    <h2 class="anim-2">Slide Title</h2>
    <p class="anim-3">Description over the image</p>
  </div>
</div>
```

**Notes:**
- Image must be base64 encoded
- `.image-overlay` provides the gradient from transparent to dark
- `.slide-content` is z-indexed above the overlay

## 6. Multi-Step Slide

Any variant can be multi-step. Add `data-steps="N"` to reveal content progressively.

```html
<div class="slide" data-slide="N" data-steps="2">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Category</span>
  <h2 class="anim-2">Slide Title</h2>
  <div class="anim-3">
    <div class="base-content">
      <!-- Always visible content -->
    </div>
    <div class="step1-content" style="opacity:0;transform:translateX(30px);transition:opacity .45s ease,transform .45s ease">
      <!-- Revealed on step 1 -->
    </div>
    <div class="step2-content" style="opacity:0;transform:translateX(30px);transition:opacity .45s ease,transform .45s ease">
      <!-- Revealed on step 2 -->
    </div>
  </div>
</div>
```

**CSS for step visibility (add to slide-specific styles):**
```css
.slide.step-1 .step1-content { opacity: 1; transform: translateX(0); }
.slide.step-2 .step2-content { opacity: 1; transform: translateX(0); }
```

**Tips:**
- Use transitions on the hidden elements for smooth reveal
- Dim previous content with `opacity: .3` when new step appears
- Previous step content stays visible unless explicitly hidden

## 7. End / Thank-You Slide

Light background, centered, minimal.

```html
<div class="slide" data-slide="N" style="justify-content:center;align-items:center;text-align:center">
  <div class="noise"></div>
  <div class="grid-lines"></div>
  <span class="label anim-1">Thank You</span>
  <h1 class="anim-2" style="font-size:4.5rem;font-weight:300;color:var(--ink);margin-top:12px">
    Presentation Title
  </h1>
  <div class="anim-3" style="width:48px;height:2px;background:var(--terracotta);margin:24px auto"></div>
  <p class="anim-4" style="font-size:1.1rem;color:var(--ink-light);max-width:unset">
    Author Name &middot; Team
  </p>
  <p class="anim-5" style="font-size:1.1rem;color:var(--warm-gray);margin-top:8px">
    Questions &amp; Discussion
  </p>
</div>
```

## Common Patterns

### Slide with custom top padding (for tall diagrams)
```html
<div class="slide" data-slide="N" style="padding-top:40px;justify-content:flex-start">
```

### Slide with no side padding (for full-width content)
```html
<div class="slide" data-slide="N" style="padding:0;flex-direction:row">
```

### Agenda/Overview slide (dark, split layout)
```html
<div class="slide slide--dark" data-slide="N" style="padding:0;flex-direction:row">
  <div class="noise"></div>
  <div style="width:25%;height:100%;display:flex;flex-direction:column;justify-content:center;padding:48px 40px;position:relative;z-index:1">
    <span class="label anim-1">Agenda</span>
    <h2 class="anim-2" style="margin-top:8px;font-size:2.2rem">What We'll Cover</h2>
    <div class="anim-3" style="display:grid;grid-template-columns:1fr 1fr;gap:12px 48px;margin-top:20px">
      <p style="color:rgba(255,255,255,.9);font-size:1.08rem"><span style="color:var(--terracotta);font-family:var(--font-mono);margin-right:8px">01</span> Topic A</p>
      <p style="color:rgba(255,255,255,.9);font-size:1.08rem"><span style="color:var(--terracotta);font-family:var(--font-mono);margin-right:8px">02</span> Topic B</p>
    </div>
  </div>
  <div style="flex:1;position:relative">
    <!-- Right side: image, diagram, or decorative content -->
  </div>
</div>
```
