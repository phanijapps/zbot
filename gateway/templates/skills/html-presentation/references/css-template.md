# ARIA CSS Template

Paste the CSS below into the `<style>` tag of every generated presentation. Add any slide-specific CSS after this base.

```css
*,*::before,*::after{margin:0;padding:0;box-sizing:border-box}

:root{
  --cream:#faf9f5;
  --cream-dark:#f0eee6;
  --ink:#141413;
  --ink-light:#3d3d3a;
  --terracotta:#d97757;
  --terracotta-deep:#c6613f;
  --muted:#75869680;
  --sand:#e3dacc;
  --warm-gray:#b0aea5;
  --sage:#bcd1ca;
  --blush:#ebcece;
  --lavender:#cbcadb;
  --gold:#d4a27f;

  --font-display:'Cormorant Garamond',Georgia,serif;
  --font-body:'DM Sans',system-ui,sans-serif;
  --font-mono:'JetBrains Mono',monospace;

  --slide-w:1920px;
  --slide-h:1080px;
}

html{font-size:24px;scroll-behavior:smooth}
body{
  background:#1a1a18;
  color:var(--ink);
  font-family:var(--font-body);
  overflow:hidden;
  height:100vh;
  width:100vw;
  margin:0;
  display:flex;
  align-items:center;
  justify-content:center;
}

/* ── Presentation Container ── */
.deck{position:relative;width:var(--slide-w);height:var(--slide-h);overflow:hidden;transform-origin:center center;flex-shrink:0}

.slide{
  position:absolute;inset:0;
  display:flex;flex-direction:column;
  justify-content:center;
  padding:60px 80px;
  background:var(--cream);
  opacity:0;pointer-events:none;
  transition:opacity .6s cubic-bezier(.4,0,.2,1),transform .6s cubic-bezier(.4,0,.2,1);
  transform:translateY(12px);
  overflow:hidden;
}
.slide.active{opacity:1;pointer-events:auto;transform:translateY(0)}
.slide.exit{opacity:0;transform:translateY(-12px)}

/* ── Typography ── */
h1,h2,h3{font-family:var(--font-display);font-weight:400;line-height:1.15}
h1{font-size:4.2rem;letter-spacing:-.02em}
h2{font-size:3rem;letter-spacing:-.01em;margin-bottom:.6em}
h3{font-size:1.9rem;margin-bottom:.4em}
p,.body-text{font-size:1.25rem;line-height:1.7;color:var(--ink-light);max-width:720px}
.label{
  font-family:var(--font-mono);font-size:.98rem;font-weight:500;
  letter-spacing:.12em;text-transform:uppercase;color:var(--terracotta);
}
code,.code{font-family:var(--font-mono);font-size:1.25rem}

/* ── Slide Variants ── */
.slide--title{
  background:var(--ink);color:var(--cream);
  justify-content:center;align-items:center;text-align:center;
}
.slide--title h1{color:var(--cream);font-size:5.8rem;font-weight:300}
.slide--title .label{color:var(--terracotta)}
.slide--title .subtitle{color:var(--warm-gray);font-size:1.3rem;margin-top:12px}
.ver{font-family:var(--font-body);font-weight:300;color:var(--terracotta)}

.slide--dark{background:var(--ink);color:var(--cream)}
.slide--dark h2{color:var(--cream)}
.slide--dark p,.slide--dark .body-text{color:var(--warm-gray)}
.slide--dark .label{color:var(--terracotta)}

.slide--accent{background:var(--terracotta);color:var(--cream)}
.slide--accent h2{color:var(--cream)}
.slide--accent p{color:rgba(255,255,255,.85)}
.slide--accent .label{color:rgba(255,255,255,.6)}

.slide--image{padding:0;justify-content:flex-end;align-items:flex-start}
.slide--image .image-overlay{
  position:absolute;inset:0;
  background:linear-gradient(180deg,transparent 30%,rgba(20,20,19,.85) 100%);
  z-index:1;
}
.slide--image .slide-content{position:relative;z-index:2;padding:60px 80px}
.slide--image h2{color:var(--cream)}
.slide--image p{color:rgba(255,255,255,.8)}
.slide--image .label{color:var(--terracotta)}

/* ── Layout Helpers ── */
.two-col{display:grid;grid-template-columns:1fr 1fr;gap:60px;align-items:start}
.three-col{display:grid;grid-template-columns:1fr 1fr 1fr;gap:40px;align-items:start}

/* ── Cards ── */
.card{
  background:rgba(0,0,0,.03);
  border:1px solid rgba(0,0,0,.06);
  border-radius:12px;padding:28px;
}
.slide--dark .card{background:rgba(255,255,255,.05);border-color:rgba(255,255,255,.08)}

/* ── Pills ── */
.pill{
  display:inline-block;padding:5px 16px;border-radius:20px;
  font-family:var(--font-mono);font-size:1.08rem;font-weight:500;
  letter-spacing:.06em;
}
.pill--terra{background:var(--terracotta);color:#fff}
.pill--outline{border:1px solid var(--warm-gray);color:var(--ink-light)}
.pill--sage{background:var(--sage);color:var(--ink)}
.pill--blush{background:var(--blush);color:var(--ink)}
.pill--lavender{background:var(--lavender);color:var(--ink)}
.pill--gold{background:var(--gold);color:#fff}
.pill--dark{background:var(--ink);color:var(--cream)}

/* ── Misc Components ── */
.tag-row{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}

.num-big{
  font-family:var(--font-display);font-size:4.5rem;font-weight:300;
  color:var(--terracotta);line-height:1;
}
.slide--dark .num-big{color:var(--terracotta)}

.divider{width:48px;height:2px;background:var(--terracotta);margin:20px 0}
.divider--light{background:rgba(255,255,255,.2)}

.flow-arrow{
  font-family:var(--font-mono);color:var(--warm-gray);
  font-size:1.4rem;text-align:center;padding:8px 0;
}
.slide--dark .flow-arrow{color:rgba(255,255,255,.3)}

/* ── Code Block ── */
.code-block{
  background:var(--ink);color:var(--cream);
  font-family:var(--font-mono);font-size:1.02rem;
  padding:24px 28px;border-radius:10px;
  line-height:1.6;white-space:pre;overflow-x:auto;
  max-height:440px;
}
.slide--dark .code-block{background:rgba(255,255,255,.06)}
.code-block .kw{color:var(--terracotta)}
.code-block .str{color:var(--sage)}
.code-block .cmt{color:var(--warm-gray)}
.code-block .num{color:var(--gold)}

/* ── Table ── */
.clean-table{width:100%;border-collapse:collapse;font-size:1rem}
.clean-table th{
  font-family:var(--font-mono);font-size:1.25rem;font-weight:500;
  letter-spacing:.1em;text-transform:uppercase;
  text-align:left;padding:10px 16px;
  border-bottom:2px solid var(--terracotta);
  color:var(--terracotta);
}
.clean-table td{padding:10px 16px;border-bottom:1px solid rgba(0,0,0,.06)}
.slide--dark .clean-table td{border-bottom-color:rgba(255,255,255,.06)}
.slide--dark .clean-table th{border-bottom-color:var(--terracotta)}

/* ── Flow Nodes ── */
.flow-node{
  padding:10px 20px;border-radius:8px;font-size:1.02rem;
  font-family:var(--font-mono);font-weight:500;white-space:nowrap;
}
.flow-node--primary{background:var(--terracotta);color:#fff}
.flow-node--secondary{background:var(--ink);color:var(--cream)}
.flow-node--outline{border:1.5px solid var(--ink);color:var(--ink)}
.slide--dark .flow-node--outline{border-color:rgba(255,255,255,.25);color:var(--cream)}
.flow-connector{font-family:var(--font-mono);color:var(--warm-gray);font-size:1.2rem}

/* ── Terminal UI ── */
.terminal{
  background:#1e1e1e;border-radius:12px;padding:0;
  overflow:hidden;font-family:var(--font-mono);width:100%;
}
.terminal-bar{
  background:#333;padding:10px 16px;display:flex;gap:8px;align-items:center;
}
.terminal-dot{width:12px;height:12px;border-radius:50%}
.terminal-dot--red{background:#ff5f57}
.terminal-dot--yellow{background:#febc2e}
.terminal-dot--green{background:#28c840}
.terminal-body{
  padding:24px;color:#e0e0e0;font-size:.95rem;line-height:1.7;
}
.term-line{opacity:0}
.slide.active .term-line{animation:termFadeIn .45s ease both}
@keyframes termFadeIn{
  from{opacity:0;transform:translateY(10px)}
  to{opacity:1;transform:translateY(0)}
}
.term-line:last-child::after{
  content:'\u258C';color:var(--terracotta);
  animation:blink 1s step-end infinite;animation-delay:3s;opacity:0;
}
.slide.active .term-line:last-child::after{opacity:1}
@keyframes blink{0%,100%{opacity:1}50%{opacity:0}}

/* ── Feature List ── */
.feature-list{list-style:none;padding:0}
.feature-list li{
  position:relative;padding-left:20px;margin-bottom:10px;
  font-size:1.25rem;line-height:1.6;color:var(--ink-light);
}
.feature-list li::before{
  content:'';position:absolute;left:0;top:9px;
  width:8px;height:8px;border-radius:50%;background:var(--terracotta);
}
.slide--dark .feature-list li{color:var(--warm-gray)}
.slide--dark .feature-list li::before{background:var(--terracotta)}
.slide--accent .feature-list li{color:rgba(255,255,255,.85)}
.slide--accent .feature-list li::before{background:rgba(255,255,255,.5)}

/* ── Tool Items ── */
.tool-item{
  display:flex;align-items:baseline;gap:10px;
  font-size:1.25rem;padding:7px 0;border-bottom:1px solid rgba(0,0,0,.04);
}
.tool-item .tool-name{
  font-family:var(--font-mono);font-weight:500;font-size:1.08rem;
  color:var(--terracotta);white-space:nowrap;min-width:200px;
}
.tool-item .tool-desc{color:var(--ink-light)}
.slide--dark .tool-item{border-bottom-color:rgba(255,255,255,.04)}
.slide--dark .tool-item .tool-desc{color:var(--warm-gray)}

/* ── Progress Bar ── */
.progress{
  position:absolute;bottom:0;left:0;height:3px;
  background:var(--terracotta);z-index:100;
  transition:width .4s ease;
}
.slide-counter{
  position:absolute;bottom:20px;right:40px;z-index:100;
  font-family:var(--font-mono);font-size:.84rem;
  color:var(--warm-gray);letter-spacing:.05em;
}

/* ── Animations ── */
@keyframes fadeUp{from{opacity:0;transform:translateY(24px)}to{opacity:1;transform:translateY(0)}}
@keyframes fadeIn{from{opacity:0}to{opacity:1}}
.slide.active .anim-1{animation:fadeUp .7s .1s both}
.slide.active .anim-2{animation:fadeUp .7s .25s both}
.slide.active .anim-3{animation:fadeUp .7s .4s both}
.slide.active .anim-4{animation:fadeUp .7s .55s both}
.slide.active .anim-5{animation:fadeUp .7s .7s both}
.slide.active .anim-6{animation:fadeUp .7s .85s both}
.slide.active .anim-fade{animation:fadeIn .8s .3s both}

/* ── Section Nav Legend ── */
.section-nav{
  position:absolute;top:0;left:0;right:0;z-index:200;
  display:flex;justify-content:center;gap:0;
  padding:0;background:transparent;
  pointer-events:none;transition:opacity .4s ease;
}
.section-nav.hidden{opacity:0}
.section-nav-item{
  font-family:var(--font-mono);font-size:.7rem;font-weight:500;
  letter-spacing:.08em;text-transform:uppercase;
  padding:10px 16px 8px;
  color:var(--warm-gray);
  border-bottom:2px solid transparent;
  transition:color .3s,border-color .3s;
  pointer-events:auto;cursor:pointer;user-select:none;
}
.section-nav-item:hover{color:var(--ink-light)}
.section-nav-item.active{
  color:var(--terracotta);border-bottom-color:var(--terracotta);
}
.section-nav.dark .section-nav-item{color:rgba(255,255,255,.3)}
.section-nav.dark .section-nav-item:hover{color:rgba(255,255,255,.6)}
.section-nav.dark .section-nav-item.active{
  color:var(--terracotta);border-bottom-color:var(--terracotta);
}
.section-nav-sep{
  align-self:center;width:3px;height:3px;border-radius:50%;
  background:var(--warm-gray);opacity:.4;margin:0 2px;
}
.section-nav.dark .section-nav-sep{background:rgba(255,255,255,.25)}

/* ── Decorative ── */
.corner-glyph{
  position:absolute;font-family:var(--font-display);font-size:14rem;
  font-weight:300;color:rgba(0,0,0,.025);line-height:1;pointer-events:none;
}
.slide--dark .corner-glyph{color:rgba(255,255,255,.03)}
.corner-glyph--tr{top:-30px;right:-20px}
.corner-glyph--bl{bottom:-40px;left:-20px}

.grid-lines{
  position:absolute;inset:0;pointer-events:none;
  background-image:
    linear-gradient(rgba(0,0,0,.02) 1px,transparent 1px),
    linear-gradient(90deg,rgba(0,0,0,.02) 1px,transparent 1px);
  background-size:80px 80px;
}
.slide--dark .grid-lines{
  background-image:
    linear-gradient(rgba(255,255,255,.02) 1px,transparent 1px),
    linear-gradient(90deg,rgba(255,255,255,.02) 1px,transparent 1px);
}

.noise{
  position:absolute;inset:0;pointer-events:none;opacity:.35;
  background-image:url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.04'/%3E%3C/svg%3E");
}
```

## Adding Slide-Specific CSS

After the base CSS above, add any custom styles needed for specific slides. Common patterns:

### Multi-step slide visibility
```css
/* For a slide with data-steps="2" */
.slide.step-1 .step1-content { opacity: 1; transform: translateX(0); }
.slide.step-2 .step2-content { opacity: 1; transform: translateX(0); }
```

### Custom card accent borders
```css
.card--accent { border-top: 3px solid var(--terracotta); }
.card--sage { border-top: 3px solid var(--sage); }
```

### Mode grid (2x2 layout)
```css
.mode-grid{
  display:grid;grid-template-columns:1fr 1fr;gap:20px;flex:1;
}
.mode-card{
  background:rgba(0,0,0,.03);border:1px solid rgba(0,0,0,.06);
  border-radius:12px;padding:22px 24px;
  display:flex;flex-direction:column;gap:10px;
}
```
