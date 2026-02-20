// tess2-rust demo SPA — router + WASM initialization

import { initWasm } from './wasm.ts';

type DemoInit = (container: HTMLElement) => (() => void) | void;
const demoModules: Record<string, () => Promise<{ init: DemoInit }>> = {
  'basic_shapes':     () => import('./demos/basic_shapes.ts'),
  'polygon_with_hole':() => import('./demos/polygon_with_hole.ts'),
  'winding_rules':    () => import('./demos/winding_rules.ts'),
  'output_modes':     () => import('./demos/output_modes.ts'),
  'shape_gallery':    () => import('./demos/shape_gallery.ts'),
  'interactive':      () => import('./demos/interactive.ts'),
};

const main = document.getElementById('main-content')!;
let currentCleanup: (() => void) | null = null;

function setActiveLink(route: string): void {
  document.querySelectorAll('.nav-link').forEach(el => {
    el.classList.toggle('active', (el as HTMLElement).dataset.route === route);
  });
}

function showLoading(): void {
  main.innerHTML = '<div class="loading">Loading…</div>';
}

function showHome(): void {
  main.innerHTML = `
    <div class="home-page">
      <div class="home-hero">
        <h1>tess2<span>.rs</span></h1>
        <p>
          A pure Rust port of the original SGI <strong>libtess2</strong> polygon tessellation library.
          Compiles to WebAssembly for live, in-browser demos.
        </p>
        <p style="margin-top:12px">
          <a href="https://github.com/larsbrubaker/tess2-rust" target="_blank" style="color:var(--accent)">
            View source on GitHub →
          </a>
        </p>
      </div>
      <div class="home-grid">
        <a href="#/basic_shapes" class="home-card">
          <div class="home-card-icon">△</div>
          <div class="home-card-title">Basic Shapes</div>
          <div class="home-card-desc">Triangles, quads, pentagons, and concave shapes — the building blocks of tessellation.</div>
        </a>
        <a href="#/polygon_with_hole" class="home-card">
          <div class="home-card-icon">◯</div>
          <div class="home-card-title">Polygon with Hole</div>
          <div class="home-card-desc">An outer square with an inner square hole — contour reversal in action.</div>
        </a>
        <a href="#/winding_rules" class="home-card">
          <div class="home-card-icon">✕</div>
          <div class="home-card-title">Winding Rules</div>
          <div class="home-card-desc">All five winding rules compared on stars, bowties, nested shapes, and overlapping polygons.</div>
        </a>
        <a href="#/output_modes" class="home-card">
          <div class="home-card-icon">⬡</div>
          <div class="home-card-title">Output Modes</div>
          <div class="home-card-desc">Element types and polygon sizes — triangles, quads, connected polygons, and boundary contours.</div>
        </a>
        <a href="#/shape_gallery" class="home-card">
          <div class="home-card-icon">✦</div>
          <div class="home-card-title">Shape Gallery</div>
          <div class="home-card-desc">Real-world polygon datasets — dude, tank, spaceship, and more from poly2tri and GLU test suites.</div>
        </a>
        <a href="#/interactive" class="home-card">
          <div class="home-card-icon">✏</div>
          <div class="home-card-title">Interactive Editor</div>
          <div class="home-card-desc">Click to draw your own polygons and watch them tessellate in real time.</div>
        </a>
      </div>
    </div>`;
}

async function navigate(route: string): Promise<void> {
  if (currentCleanup) { currentCleanup(); currentCleanup = null; }

  setActiveLink(route === '' ? 'home' : route);

  if (route === '' || route === 'home') {
    showHome();
    return;
  }

  const loader = demoModules[route];
  if (!loader) {
    main.innerHTML = `<div class="demo-page"><div class="error-box">Unknown demo: ${route}</div></div>`;
    return;
  }

  showLoading();
  try {
    const mod = await loader();
    main.innerHTML = '';
    const cleanup = mod.init(main);
    if (typeof cleanup === 'function') currentCleanup = cleanup;
  } catch (err) {
    main.innerHTML = `<div class="demo-page"><div class="error-box">Failed to load demo: ${err}</div></div>`;
  }
}

function routeFromHash(): string {
  const hash = window.location.hash.replace(/^#\/?/, '');
  return hash;
}

window.addEventListener('hashchange', () => navigate(routeFromHash()));

// Boot
(async () => {
  try {
    await initWasm();
  } catch (err) {
    main.innerHTML = `<div class="demo-page"><div class="error-box">Failed to load WASM module: ${err}</div></div>`;
    return;
  }
  navigate(routeFromHash());
})();
