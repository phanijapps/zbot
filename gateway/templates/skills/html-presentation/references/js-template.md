# ARIA JavaScript Navigation Engine

Paste this into the first `<script>` tag, after the deck `</div>` closing tag.

**Before pasting, replace the template placeholders:**
- `SLIDE_SECTION_ARRAY` — array mapping each slide index to a section key string (or `null`)
- `SECTION_FIRST_SLIDE_MAP` — object mapping section key to first slide index
- `TOTAL_SLIDES` — the actual slide count for the counter display

```javascript
(function(){
  const deck = document.getElementById('deck');

  // ── Scale deck to fit viewport ──
  function resizeDeck(){
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const scale = Math.min(vw / 1920, vh / 1080);
    deck.style.transform = 'scale(' + scale + ')';
  }
  resizeDeck();
  window.addEventListener('resize', resizeDeck);

  // ── Slide state ──
  const slides = Array.from(deck.querySelectorAll('.slide'));
  const progress = document.getElementById('progress');
  const counter = document.getElementById('counter');
  const sectionNav = document.getElementById('sectionNav');
  const navItems = Array.from(sectionNav.querySelectorAll('.section-nav-item'));
  let current = 0;
  let currentStep = 0;
  const total = slides.length;

  // ── Section mapping ──
  // Map each slide index to a section key (null = hide nav)
  // REPLACE THIS with your actual mapping:
  const slideSection = SLIDE_SECTION_ARRAY;

  // Map section key to first slide index (for click-to-jump)
  // REPLACE THIS with your actual mapping:
  const sectionFirstSlide = SECTION_FIRST_SLIDE_MAP;

  // ── Section nav update ──
  function updateNav(n){
    const section = slideSection[n] || null;
    if(section){ sectionNav.classList.remove('hidden'); }
    else{ sectionNav.classList.add('hidden'); }
    // Dark mode detection
    const isDark = slides[n].classList.contains('slide--dark') ||
                   slides[n].classList.contains('slide--title') ||
                   slides[n].classList.contains('slide--accent') ||
                   slides[n].classList.contains('slide--image');
    sectionNav.classList.toggle('dark', isDark);
    // Highlight active section
    navItems.forEach(item => {
      item.classList.toggle('active', item.dataset.section === section);
    });
  }

  // Click on nav item to jump to section
  navItems.forEach(item => {
    item.addEventListener('click', () => {
      const target = sectionFirstSlide[item.dataset.section];
      if(target != null){ currentStep = 0; goTo(target); }
    });
  });

  // ── Navigation ──
  function goTo(n){
    if(n < 0 || n >= total || n === current) return;
    slides[current].classList.remove('active');
    slides[current].classList.remove('step-0','step-1','step-2','step-3','step-4');
    slides[current].classList.add('exit');
    const prev = current;
    current = n;
    slides[current].classList.add('active');
    if(currentStep > 0) slides[current].classList.add('step-' + currentStep);
    setTimeout(() => slides[prev].classList.remove('exit'), 650);
    progress.style.width = ((current / (total - 1)) * 100) + '%';
    counter.textContent = (current + 1) + ' / ' + total;
    const isDark = slides[current].classList.contains('slide--dark') ||
                   slides[current].classList.contains('slide--title') ||
                   slides[current].classList.contains('slide--accent') ||
                   slides[current].classList.contains('slide--image');
    counter.style.color = isDark ? 'rgba(255,255,255,.35)' : 'var(--warm-gray)';
    updateNav(current);
  }

  function next(){
    const slide = slides[current];
    const maxSteps = parseInt(slide.dataset.steps || '0', 10);
    if(maxSteps > 0 && currentStep < maxSteps){
      currentStep++;
      slide.classList.remove('step-0','step-1','step-2','step-3','step-4');
      slide.classList.add('step-' + currentStep);
      return;
    }
    currentStep = 0;
    goTo(current + 1);
  }

  function prev(){
    const slide = slides[current];
    const maxSteps = parseInt(slide.dataset.steps || '0', 10);
    if(maxSteps > 0 && currentStep > 0){
      currentStep--;
      slide.classList.remove('step-0','step-1','step-2','step-3','step-4');
      if(currentStep > 0) slide.classList.add('step-' + currentStep);
      return;
    }
    const pi = current - 1;
    if(pi >= 0){
      const ps = slides[pi];
      currentStep = parseInt(ps.dataset.steps || '0', 10);
      goTo(pi);
      if(currentStep > 0) ps.classList.add('step-' + currentStep);
    }
  }

  // ── Keyboard controls ──
  document.addEventListener('keydown', e => {
    // Skip navigation keys when diagram editor is active
    if(window._diagramEditMode && e.key !== 'e' && e.key !== 'E' && e.key !== 'd' && e.key !== 'D') return;
    if(e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown'){ e.preventDefault(); next(); }
    if(e.key === 'ArrowLeft' || e.key === 'PageUp'){ e.preventDefault(); prev(); }
    if(e.key === 'Home'){ e.preventDefault(); currentStep = 0; goTo(0); }
    if(e.key === 'End'){ e.preventDefault(); currentStep = 0; goTo(total - 1); }
  });

  // ── Touch swipe ──
  let touchX = 0;
  deck.addEventListener('touchstart', e => { touchX = e.touches[0].clientX; });
  deck.addEventListener('touchend', e => {
    const dx = e.changedTouches[0].clientX - touchX;
    if(Math.abs(dx) > 50) dx < 0 ? next() : prev();
  });

  // ── Click zones ──
  deck.addEventListener('click', e => {
    if(window._diagramEditMode) return;
    if(e.target.closest('a,button,code')) return;
    const x = e.clientX / window.innerWidth;
    x < 0.33 ? prev() : next();
  });

  // Initial state
  progress.style.width = '0%';
  counter.style.color = 'rgba(255,255,255,.35)';
})();
```

## How to Fill the Placeholders

### slideSection Array
Create an array with one entry per slide. Use `null` for slides where the nav should be hidden (title, end), and a string section key for all others:

```javascript
const slideSection = [
  null,       // 0: title
  null,       // 1: agenda
  'intro',    // 2: chapter divider
  'intro',    // 3: content
  'demo',     // 4: chapter divider
  'demo',     // 5: content
  null        // 6: thank you
];
```

### sectionFirstSlide Map
Map each section key to the first slide index in that section:

```javascript
const sectionFirstSlide = {
  intro: 2,
  demo: 4
};
```

### Section Nav HTML
The section nav HTML in the deck must match:
```html
<div class="section-nav hidden" id="sectionNav">
  <span class="section-nav-item" data-section="intro">Introduction</span>
  <span class="section-nav-sep"></span>
  <span class="section-nav-item" data-section="demo">Demo</span>
</div>
```

## Multi-Step Slide Notes

When a slide has `data-steps="N"`:
- Forward navigation (`next()`) increments `currentStep` and adds `step-N` class
- Backward navigation (`prev()`) decrements `currentStep`
- When stepping backward past step 0, it goes to the previous slide at its max step
- CSS uses `.slide.step-1`, `.slide.step-2`, etc. to control visibility
