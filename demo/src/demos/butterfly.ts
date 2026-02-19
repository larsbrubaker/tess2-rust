// Butterfly (self-intersecting bowtie) demo
import { tessellate, WINDING_RULES } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

// A figure-8 / butterfly drawn as a single closed contour (self-intersecting)
const BUTTERFLY = [
  -1.5, -1,   0, 0,   1.5, -1,   1.5, 1,   0, 0,   -1.5, 1
];

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Butterfly (Self-Intersecting)</h2>
      <p class="demo-description">
        A figure-8 / bowtie shape defined by a single self-intersecting contour.
        The tessellator correctly handles the intersection point and applies the chosen winding rule.
      </p>
      <div class="winding-grid" id="winding-grid"></div>
      <div class="info-box">
        The contour crosses itself at the origin. Depending on the winding rule, the resulting
        filled region may be one or two separate triangles.
      </div>
    </div>`;

  const gridEl = container.querySelector('#winding-grid')!;
  const SIZE = 140;

  for (const wr of WINDING_RULES) {
    const cell = document.createElement('div');
    cell.className = 'winding-cell';

    const label = document.createElement('div');
    label.className = 'winding-name';
    label.textContent = wr;

    const canvas = makeCanvas(SIZE, SIZE);
    const result = tessellate([{ vertices: BUTTERFLY, reversed: false }], wr);

    if (result && result.triangleCount > 0) {
      drawTessellation(canvas, result.vertices, result.elements, {
        fillColor: COLORS.fill,
        showInput: true,
        inputContours: [BUTTERFLY],
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
    triCount.textContent = result ? `${result.triangleCount} tri` : 'failed';

    cell.appendChild(label);
    cell.appendChild(canvas);
    cell.appendChild(triCount);
    gridEl.appendChild(cell);
  }
}
