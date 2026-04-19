# ARIA Diagram Editor

A generic, self-contained diagram editing system for any presentation slide that contains positioned/diagrammatic layouts. Allows the user to visually drag and resize elements, then copy the resulting CSS positions to clipboard.

## Convention

To make any diagram editable:

1. **Container**: Add `class="diagram-editable"` and set `position:relative` with explicit `width` and `height`:
```html
<div class="diagram-editable" style="position:relative;width:1300px;height:580px;margin:0 auto">
  <!-- positioned children here -->
</div>
```

2. **Children**: Add `class="diagram-el"` to each positioned element with `position:absolute`:
```html
<div class="diagram-el" style="position:absolute;left:100px;top:200px;width:280px;height:110px">
  Content here
</div>
```

3. **Include the editor script** as the second `<script>` block (after the navigation engine).

## Keyboard Controls

- **`e`** — Enable edit mode (only when current slide has a `.diagram-editable` container)
- **`d`** — End edit mode, serialize all positions, copy CSS to clipboard

## Editor Features

When active:
- All animations are frozen (`animation: none`)
- Hidden elements made visible (`opacity: 1`)
- Dashed terracotta outlines shown on all `.diagram-el` elements
- **Drag center** of element to move it
- **Drag edge/corner** (within 10px zone) to resize it
- Live coordinate tooltip follows cursor
- Info panel appears top-right with keybinding reminders
- Container border highlighted with solid terracotta outline
- Container itself can be resized by dragging its edges

When ended (`d`):
- All `.diagram-el` positions serialized as CSS
- CSS string copied to `navigator.clipboard`
- Outlines and panel removed
- Animations restored (elements may re-animate if slide is re-entered)

## Script

Paste this as a self-contained IIFE in the second `<script>` tag:

```javascript
(function(){
  window._diagramEditMode = false;
  let active = false;
  let container = null;
  let mode = null; // 'move' | 'resize-el' | 'resize-container'
  let target = null;
  let startX, startY, origLeft, origTop, origW, origH;
  const EDGE = 10; // px from edge = resize zone

  // Find the diagram container on the current active slide
  function getContainer(){
    const activeSlide = document.querySelector('.slide.active');
    if(!activeSlide) return null;
    return activeSlide.querySelector('.diagram-editable');
  }

  // ── Tooltip ──
  let tip = null;
  function showTip(text, x, y){
    if(!tip){
      tip = document.createElement('div');
      Object.assign(tip.style, {
        position:'fixed', zIndex:'999999',
        background:'rgba(0,0,0,.82)', color:'#fff',
        fontFamily:'monospace', fontSize:'11px',
        padding:'3px 8px', borderRadius:'4px',
        pointerEvents:'none', whiteSpace:'pre'
      });
      document.body.appendChild(tip);
    }
    tip.textContent = text;
    tip.style.left = (x + 14) + 'px';
    tip.style.top = (y + 14) + 'px';
  }
  function hideTip(){ if(tip){ tip.remove(); tip = null; } }

  // ── Resize zone detection ──
  function isResizeZone(el, mx, my){
    const r = el.getBoundingClientRect();
    return (r.right - mx < EDGE) || (r.bottom - my < EDGE);
  }
  function resizeCursor(el, mx, my){
    const r = el.getBoundingClientRect();
    const nearR = r.right - mx < EDGE;
    const nearB = r.bottom - my < EDGE;
    if(nearR && nearB) return 'nwse-resize';
    if(nearR) return 'ew-resize';
    if(nearB) return 'ns-resize';
    return null;
  }

  // ── Class name helper ──
  function clsName(el){
    return Array.from(el.classList).filter(c => c !== 'diagram-el' && c !== 'diagram-editable').join(' ') || el.tagName;
  }

  // ── Enable edit mode ──
  function enable(){
    container = getContainer();
    if(!container) return;
    active = true;
    window._diagramEditMode = true;

    // Freeze all animations, reveal hidden elements
    container.style.animation = 'none';
    container.querySelectorAll('*').forEach(el => {
      el.style.animation = 'none';
      const cs = getComputedStyle(el);
      if(cs.opacity === '0') el.style.opacity = '1';
    });

    // Add outlines to editable elements
    container.querySelectorAll('.diagram-el').forEach(el => {
      el.style.cursor = 'grab';
      el.style.outline = '1px dashed rgba(217,119,87,0.5)';
    });
    container.style.outline = '2px solid var(--terracotta)';
    showPanel();
  }

  // ── Disable edit mode ──
  function disable(){
    active = false;
    window._diagramEditMode = false;
    if(!container) return;
    // Remove outlines
    container.querySelectorAll('.diagram-el').forEach(el => {
      el.style.cursor = '';
      el.style.outline = '';
    });
    container.style.outline = '';
    // Export CSS
    exportCSS();
    hidePanel();
    hideTip();
  }

  // ── Info panel ──
  let panel = null;
  function showPanel(){
    if(panel) return;
    panel = document.createElement('div');
    panel.innerHTML = '<div style="font-weight:700;margin-bottom:6px;color:#d97757">Diagram Editor</div>' +
      '<div style="font-size:.72rem;line-height:1.6;color:#555">' +
      '<b>Drag</b> center &rarr; move<br>' +
      '<b>Drag</b> edge/corner &rarr; resize<br>' +
      '<b>Drag</b> container border &rarr; resize container<br>' +
      '<b>D</b> &rarr; end edit &amp; copy CSS to clipboard</div>' +
      '<div id="de-info" style="margin-top:8px;font-family:monospace;font-size:11px;color:#333;min-height:3em"></div>';
    Object.assign(panel.style, {
      position:'fixed', top:'10px', right:'10px', zIndex:'99999',
      background:'#fff', border:'2px solid #d97757', borderRadius:'10px',
      padding:'14px 18px', boxShadow:'0 4px 20px rgba(0,0,0,.15)',
      fontFamily:'system-ui', fontSize:'.85rem', maxWidth:'300px'
    });
    document.body.appendChild(panel);
  }
  function hidePanel(){ if(panel){ panel.remove(); panel = null; } }
  function setInfo(t){ const el = document.getElementById('de-info'); if(el) el.textContent = t; }

  // ── Export CSS ──
  function exportCSS(){
    if(!container) return;
    const lines = [];
    lines.push('/* Container: ' + container.offsetWidth + 'x' + container.offsetHeight + ' */');
    container.querySelectorAll('.diagram-el').forEach(el => {
      const cs = getComputedStyle(el);
      const cls = clsName(el);
      const l = Math.round(parseFloat(cs.left));
      const t = Math.round(parseFloat(cs.top));
      const w = Math.round(parseFloat(cs.width));
      const h = Math.round(parseFloat(cs.height));
      lines.push('.' + cls.replace(/\s+/g, '.') + ' { left:' + l + 'px; top:' + t + 'px; width:' + w + 'px; height:' + h + 'px; }');
    });
    const out = lines.join('\n');
    console.log('%c Diagram Layout Export \n', 'color:#d97757;font-weight:bold', out);
    navigator.clipboard.writeText(out).then(() => setInfo('Copied to clipboard!')).catch(() => {});
  }

  // ── Keyboard handler ──
  document.addEventListener('keydown', e => {
    if(e.ctrlKey || e.metaKey || e.altKey) return;
    const cont = getContainer();
    if(!cont) return;
    // E = enable edit
    if((e.key === 'e' || e.key === 'E') && !active){
      e.preventDefault(); e.stopPropagation();
      enable();
    }
    // D = end edit (copy to clipboard)
    if((e.key === 'd' || e.key === 'D') && active){
      e.preventDefault(); e.stopPropagation();
      disable();
    }
  });

  // ── Mouse move: update cursors / perform drag ──
  document.addEventListener('mousemove', e => {
    if(!active || !container) return;
    if(mode){
      e.preventDefault();
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;

      if(mode === 'move'){
        target.style.left = Math.round(origLeft + dx) + 'px';
        target.style.top = Math.round(origTop + dy) + 'px';
        target.style.right = 'auto';
        target.style.transform = 'translateX(0)';
        const cs = getComputedStyle(target);
        showTip(clsName(target) + '\nleft: ' + Math.round(parseFloat(cs.left)) + 'px\ntop: ' + Math.round(parseFloat(cs.top)) + 'px', e.clientX, e.clientY);
      }
      else if(mode === 'resize-el'){
        const newW = Math.max(20, origW + dx);
        const newH = Math.max(10, origH + dy);
        target.style.width = Math.round(newW) + 'px';
        target.style.height = Math.round(newH) + 'px';
        showTip(clsName(target) + '\nwidth: ' + Math.round(newW) + 'px\nheight: ' + Math.round(newH) + 'px', e.clientX, e.clientY);
      }
      else if(mode === 'resize-container'){
        const newW = Math.max(200, origW + dx);
        const newH = Math.max(100, origH + dy);
        container.style.width = Math.round(newW) + 'px';
        container.style.height = Math.round(newH) + 'px';
        showTip('container\nwidth: ' + Math.round(newW) + 'px\nheight: ' + Math.round(newH) + 'px', e.clientX, e.clientY);
      }
      return;
    }

    // Hover cursor hints for container edges
    const cr = container.getBoundingClientRect();
    const nearContR = Math.abs(e.clientX - cr.right) < EDGE && e.clientY >= cr.top && e.clientY <= cr.bottom;
    const nearContB = Math.abs(e.clientY - cr.bottom) < EDGE && e.clientX >= cr.left && e.clientX <= cr.right;
    if(nearContR && nearContB){ document.body.style.cursor = 'nwse-resize'; return; }
    if(nearContR){ document.body.style.cursor = 'ew-resize'; return; }
    if(nearContB){ document.body.style.cursor = 'ns-resize'; return; }

    // Hover cursor hints for elements
    const el = e.target.closest('.diagram-el');
    if(el && container.contains(el)){
      const rc = resizeCursor(el, e.clientX, e.clientY);
      el.style.cursor = rc || 'grab';
    }
    document.body.style.cursor = '';
  });

  // ── Mouse down: start drag ──
  document.addEventListener('mousedown', e => {
    if(!active || !container) return;

    // Check container edge first
    const cr = container.getBoundingClientRect();
    const nearContR = Math.abs(e.clientX - cr.right) < EDGE && e.clientY >= cr.top && e.clientY <= cr.bottom;
    const nearContB = Math.abs(e.clientY - cr.bottom) < EDGE && e.clientX >= cr.left && e.clientX <= cr.right;
    if(nearContR || nearContB){
      e.preventDefault(); e.stopPropagation();
      mode = 'resize-container';
      startX = e.clientX; startY = e.clientY;
      origW = container.offsetWidth; origH = container.offsetHeight;
      return;
    }

    // Check diagram elements
    const el = e.target.closest('.diagram-el');
    if(!el || !container.contains(el)) return;
    e.preventDefault(); e.stopPropagation();
    target = el;
    startX = e.clientX; startY = e.clientY;

    const rc = resizeCursor(el, e.clientX, e.clientY);
    if(rc){
      mode = 'resize-el';
      origW = el.offsetWidth; origH = el.offsetHeight;
    } else {
      mode = 'move';
      const cs = getComputedStyle(el);
      origLeft = parseFloat(cs.left) || 0;
      origTop = parseFloat(cs.top) || 0;
      el.style.cursor = 'grabbing';
    }
  }, true);

  // ── Mouse up: end drag ──
  document.addEventListener('mouseup', () => {
    if(target) target.style.cursor = 'grab';
    mode = null; target = null;
    document.body.style.cursor = '';
    hideTip();
  });
})();
```

## Usage Example

A slide with an editable process diagram:

```html
<div class="slide" data-slide="5">
  <div class="noise"></div>
  <span class="label anim-1">Architecture</span>
  <h2 class="anim-2">System Overview</h2>
  <div class="diagram-editable anim-3" style="position:relative;width:1200px;height:500px;margin:0 auto">
    <div class="diagram-el input-box" style="position:absolute;left:50px;top:100px;width:200px;height:80px;background:var(--terracotta);color:#fff;border-radius:10px;display:flex;align-items:center;justify-content:center;font-family:var(--font-mono);font-size:1rem">
      Input
    </div>
    <div class="diagram-el arrow-1" style="position:absolute;left:270px;top:125px;font-family:var(--font-mono);font-size:1.4rem;color:var(--warm-gray)">
      &rarr;
    </div>
    <div class="diagram-el process-box" style="position:absolute;left:320px;top:80px;width:300px;height:140px;border:2px dashed var(--gold);border-radius:14px;background:rgba(212,162,127,.06);display:flex;align-items:center;justify-content:center;font-family:var(--font-mono);font-size:1rem;color:var(--ink-light)">
      Processing
    </div>
    <div class="diagram-el arrow-2" style="position:absolute;left:640px;top:125px;font-family:var(--font-mono);font-size:1.4rem;color:var(--warm-gray)">
      &rarr;
    </div>
    <div class="diagram-el output-box" style="position:absolute;left:700px;top:100px;width:200px;height:80px;background:var(--ink);color:var(--cream);border-radius:10px;display:flex;align-items:center;justify-content:center;font-family:var(--font-mono);font-size:1rem">
      Output
    </div>
  </div>
</div>
```

After the user presses `e`, drags elements to desired positions, then presses `d`, the editor copies CSS like:
```
/* Container: 1200x500 */
.input-box { left:50px; top:100px; width:200px; height:80px; }
.arrow-1 { left:270px; top:125px; width:30px; height:34px; }
.process-box { left:320px; top:80px; width:300px; height:140px; }
...
```

The user can then paste these values back into the HTML to lock in the positions.
