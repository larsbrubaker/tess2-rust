// Five-Pointed Star demo
import { tessellate, WINDING_RULES } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

function starPath(cx: number, cy: number, r: number): number[] {
  const verts: number[] = [];
  const n = 5;
  const step = (Math.PI * 2) / n;
  const skip = 2;
  for (let i = 0; i < n; i++) {
    const angle = -Math.PI / 2 + i * step * skip;
    verts.push(cx + Math.cos(angle) * r, cy + Math.sin(angle) * r);
  }
  return verts;
}

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Five-Pointed Star</h2>
      <p class="demo-description">
        A classic pentagram drawn as a single self-intersecting contour. The five winding rule
        variants produce very different results — from the full star to just the central pentagon.
      </p>
      <div class="winding-grid" id="winding-grid"></div>
      <div class="info-box">
        <strong>Odd / NonZero:</strong> Fill includes the points of the star.<br>
        <strong>Positive:</strong> Same as NonZero for a CCW star.<br>
        <strong>Negative:</strong> Nothing (all windings are positive for CCW contour).<br>
        <strong>AbsGeqTwo:</strong> Only the doubly-wound central pentagon is filled.
      </div>
    </div>`;

  const gridEl = container.querySelector('#winding-grid')!;
  const VERTS = starPath(0, 0, 1);
  const SIZE = 140;

  for (const wr of WINDING_RULES) {
    const cell = document.createElement('div');
    cell.className = 'winding-cell';

    const label = document.createElement('div');
    label.className = 'winding-name';
    label.textContent = wr;

    const canvas = makeCanvas(SIZE, SIZE);
    const result = tessellate([{ vertices: VERTS, reversed: false }], wr);

    if (result && result.elementCount > 0) {
      drawTessellation(canvas, result.vertices, result.elements, {
        fillColor: COLORS.fill,
        showInput: true,
        inputContours: [VERTS],
      });
    } else {
      const ctx = canvas.getContext('2d')!;
      ctx.font = '11px sans-serif';
      ctx.fillStyle = '#aaa';
      ctx.textAlign = 'center';
      ctx.fillText('(empty)', SIZE/2, SIZE/2);
    }

    const triCount = document.createElement('div');
    triCount.className = 'winding-tri-count';
    triCount.textContent = result ? `${result.elementCount} tri` : 'failed';

    cell.appendChild(label);
    cell.appendChild(canvas);
    cell.appendChild(triCount);
    gridEl.appendChild(cell);
  }
}
