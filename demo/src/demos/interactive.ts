// Interactive polygon editor demo
import { tessellate, WindingRule, WINDING_RULES } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

interface Point { x: number; y: number; }
type Contour = Point[];

export function init(container: HTMLElement): (() => void) {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Interactive Editor</h2>
      <p class="demo-description">Click to add vertices. Right-click or press Enter to close the current contour and start a new one.</p>
      <div class="editor-layout">
        <div class="editor-canvas-wrapper">
          <canvas id="editor-canvas" class="demo-canvas" width="480" height="480"></canvas>
        </div>
        <div class="editor-sidebar">
          <div class="editor-instructions">
            <strong>Controls:</strong><br>
            <kbd>Click</kbd> Add vertex<br>
            <kbd>Enter</kbd> / <kbd>Right-click</kbd> Close contour<br>
            <kbd>Backspace</kbd> Remove last vertex<br>
            <kbd>Esc</kbd> Clear all
          </div>
          <div class="controls" style="flex-direction:column; align-items:flex-start">
            <div class="control-group">
              <label>Winding rule:</label>
              <select id="winding-sel">
                ${WINDING_RULES.map((r,i) => `<option value="${i}"${r==='Positive'?' selected':''}>${r}</option>`).join('')}
              </select>
            </div>
            <div class="control-group">
              <label><input type="checkbox" id="reverse-cb"> Next contour is hole</label>
            </div>
            <button class="btn btn-outline" id="clear-btn" style="width:100%">Clear All</button>
            <button class="btn btn-outline" id="example-btn" style="width:100%">Load Example</button>
          </div>
          <div class="stats-bar" id="stats"></div>
          <ul class="contour-list" id="contour-list"></ul>
        </div>
      </div>
    </div>`;

  const canvas = container.querySelector('#editor-canvas') as HTMLCanvasElement;
  const ctx = canvas.getContext('2d')!;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;
  const reverseCb = container.querySelector('#reverse-cb') as HTMLInputElement;
  const clearBtn  = container.querySelector('#clear-btn') as HTMLButtonElement;
  const exBtn     = container.querySelector('#example-btn') as HTMLButtonElement;
  const statsEl   = container.querySelector('#stats')!;
  const listEl    = container.querySelector('#contour-list')!;

  const W = canvas.width, H = canvas.height;
  const PAD = 32;

  // State: list of closed contours + current open contour
  interface ContourState { pts: Point[]; reversed: boolean; }
  let contours: ContourState[] = [];
  let current: ContourState = { pts: [], reversed: false };

  function screenToWorld(ex: number, ey: number): Point {
    const rect = canvas.getBoundingClientRect();
    const sx = (ex - rect.left) * (W / rect.width);
    const sy = (ey - rect.top) * (H / rect.height);
    // Map [PAD..W-PAD] × [PAD..H-PAD] → [-5..5] × [-5..5]
    return {
      x: (sx - PAD) / ((W - 2*PAD) / 10) - 5,
      y: (sy - PAD) / ((H - 2*PAD) / 10) - 5,
    };
  }

  function worldToScreen(wx: number, wy: number): [number, number] {
    const sx = (wx + 5) * ((W - 2*PAD) / 10) + PAD;
    const sy = (wy + 5) * ((H - 2*PAD) / 10) + PAD;
    return [sx, sy];
  }

  function closeCurrentContour() {
    if (current.pts.length >= 3) {
      contours.push({ ...current, pts: [...current.pts] });
    }
    current = { pts: [], reversed: reverseCb.checked };
    updateList();
  }

  function updateList() {
    listEl.innerHTML = contours.map((c, i) =>
      `<li>Contour ${i+1}: ${c.pts.length} pts${c.reversed ? ' (hole)' : ''}</li>`
    ).join('') + (current.pts.length > 0 ? `<li><em>Open: ${current.pts.length} pts</em></li>` : '');
  }

  function render() {
    ctx.clearRect(0, 0, W, H);

    // Grid
    ctx.strokeStyle = 'rgba(200,200,200,0.3)';
    ctx.lineWidth = 0.5;
    for (let x = -5; x <= 5; x++) {
      const [sx] = worldToScreen(x, 0);
      ctx.beginPath(); ctx.moveTo(sx, 0); ctx.lineTo(sx, H); ctx.stroke();
    }
    for (let y = -5; y <= 5; y++) {
      const [, sy] = worldToScreen(0, y);
      ctx.beginPath(); ctx.moveTo(0, sy); ctx.lineTo(W, sy); ctx.stroke();
    }

    // Axes
    ctx.strokeStyle = 'rgba(0,0,0,0.15)';
    ctx.lineWidth = 1;
    const [ax] = worldToScreen(0, 0);
    const [, ay] = worldToScreen(0, 0);
    ctx.beginPath(); ctx.moveTo(ax, 0); ctx.lineTo(ax, H); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(0, ay); ctx.lineTo(W, ay); ctx.stroke();

    // Tessellate closed contours
    if (contours.length > 0) {
      const wr = WINDING_RULES[parseInt(windingSel.value)];
      const inputContours = contours.map(c => ({
        vertices: c.pts.flatMap(p => [p.x, p.y]),
        reversed: c.reversed,
      }));
      const result = tessellate(inputContours, wr);
      if (result && result.triangleCount > 0) {
        // Map tessellated vertices to screen
        const sv = new Float32Array(result.vertices.length);
        for (let i = 0; i < result.vertices.length; i += 2) {
          const [sx, sy] = worldToScreen(result.vertices[i], result.vertices[i+1]);
          sv[i] = sx; sv[i+1] = sy;
        }
        // Draw filled triangles directly (already in screen coords)
        ctx.fillStyle = 'rgba(26,122,94,0.45)';
        for (let i = 0; i < result.elements.length; i += 3) {
          const i0 = result.elements[i]*2, i1 = result.elements[i+1]*2, i2 = result.elements[i+2]*2;
          ctx.beginPath();
          ctx.moveTo(sv[i0], sv[i0+1]);
          ctx.lineTo(sv[i1], sv[i1+1]);
          ctx.lineTo(sv[i2], sv[i2+1]);
          ctx.closePath(); ctx.fill();
        }
        ctx.strokeStyle = 'rgba(255,255,255,0.6)';
        ctx.lineWidth = 0.7;
        for (let i = 0; i < result.elements.length; i += 3) {
          const i0 = result.elements[i]*2, i1 = result.elements[i+1]*2, i2 = result.elements[i+2]*2;
          ctx.beginPath();
          ctx.moveTo(sv[i0], sv[i0+1]);
          ctx.lineTo(sv[i1], sv[i1+1]);
          ctx.lineTo(sv[i2], sv[i2+1]);
          ctx.closePath(); ctx.stroke();
        }
        statsEl.textContent = `${result.triangleCount} triangle(s) · ${result.vertices.length/2} output vertices`;
      } else {
        statsEl.textContent = result ? '0 triangles' : 'Tessellation failed';
      }
    } else {
      statsEl.textContent = 'Add vertices to begin';
    }

    // Draw closed contours (outlines)
    for (const c of contours) {
      if (c.pts.length < 2) continue;
      ctx.strokeStyle = c.reversed ? 'rgba(200,60,20,0.8)' : 'rgba(26,122,94,0.8)';
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4,3]);
      ctx.beginPath();
      const [x0, y0] = worldToScreen(c.pts[0].x, c.pts[0].y);
      ctx.moveTo(x0, y0);
      for (let i = 1; i < c.pts.length; i++) {
        const [xi, yi] = worldToScreen(c.pts[i].x, c.pts[i].y);
        ctx.lineTo(xi, yi);
      }
      ctx.closePath(); ctx.stroke();
      ctx.setLineDash([]);
      // Vertices
      ctx.fillStyle = c.reversed ? 'rgba(200,60,20,0.9)' : 'rgba(26,122,94,0.9)';
      for (const p of c.pts) {
        const [sx, sy] = worldToScreen(p.x, p.y);
        ctx.beginPath(); ctx.arc(sx, sy, 3, 0, Math.PI*2); ctx.fill();
      }
    }

    // Draw current open contour
    if (current.pts.length > 0) {
      ctx.strokeStyle = 'rgba(80,80,80,0.7)';
      ctx.lineWidth = 1.5;
      ctx.setLineDash([3,3]);
      ctx.beginPath();
      const [x0, y0] = worldToScreen(current.pts[0].x, current.pts[0].y);
      ctx.moveTo(x0, y0);
      for (let i = 1; i < current.pts.length; i++) {
        const [xi, yi] = worldToScreen(current.pts[i].x, current.pts[i].y);
        ctx.lineTo(xi, yi);
      }
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = 'rgba(80,80,200,0.9)';
      for (const p of current.pts) {
        const [sx, sy] = worldToScreen(p.x, p.y);
        ctx.beginPath(); ctx.arc(sx, sy, 3.5, 0, Math.PI*2); ctx.fill();
      }
    }

    updateList();
  }

  canvas.addEventListener('click', (e) => {
    const pt = screenToWorld(e.clientX, e.clientY);
    current.pts.push(pt);
    render();
  });

  canvas.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    closeCurrentContour();
    render();
  });

  const keyHandler = (e: KeyboardEvent) => {
    if (e.key === 'Enter') { closeCurrentContour(); render(); }
    if (e.key === 'Backspace') {
      if (current.pts.length > 0) { current.pts.pop(); render(); }
      else if (contours.length > 0) { contours.pop(); render(); }
    }
    if (e.key === 'Escape') { contours = []; current = { pts: [], reversed: false }; render(); }
  };
  document.addEventListener('keydown', keyHandler);

  clearBtn.addEventListener('click', () => {
    contours = []; current = { pts: [], reversed: reverseCb.checked }; render();
  });

  exBtn.addEventListener('click', () => {
    contours = [
      { pts: [{x:-3,y:-3},{x:3,y:-3},{x:3,y:3},{x:-3,y:3}], reversed: false },
      { pts: [{x:-1,y:-1},{x:1,y:-1},{x:1,y:1},{x:-1,y:1}], reversed: true },
    ];
    current = { pts: [], reversed: false };
    render();
  });

  windingSel.addEventListener('change', render);
  reverseCb.addEventListener('change', () => { current.reversed = reverseCb.checked; });

  render();
  return () => { document.removeEventListener('keydown', keyHandler); };
}
