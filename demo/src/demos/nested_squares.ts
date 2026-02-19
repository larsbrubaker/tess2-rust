// Nested Squares demo
import { tessellate, WINDING_RULES } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

// Three nested squares, alternating CW/CCW
const OUTER = [-3,-3, 3,-3, 3,3, -3,3];    // CCW
const MIDDLE = [-2,-2, -2,2, 2,2, 2,-2];   // CW (reversed)
const INNER  = [-1,-1, 1,-1, 1,1, -1,1];   // CCW

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Nested Squares</h2>
      <p class="demo-description">
        Three concentric squares with alternating winding directions.
        The winding rule determines which "rings" are filled.
        Under <em>Odd</em> rule only alternating rings fill. Under <em>NonZero</em> all fill.
      </p>
      <div class="winding-grid" id="winding-grid"></div>
      <div class="info-box">
        <strong>Outer:</strong> CCW (+1), <strong>Middle:</strong> CW (−1 reversal), <strong>Inner:</strong> CCW (+1)<br>
        At any point the winding = sum of windings of enclosing rings.
      </div>
    </div>`;

  const gridEl = container.querySelector('#winding-grid')!;
  const SIZE = 140;
  const contours = [
    { vertices: OUTER,  reversed: false },
    { vertices: MIDDLE, reversed: true  },
    { vertices: INNER,  reversed: false },
  ];

  for (const wr of WINDING_RULES) {
    const cell = document.createElement('div');
    cell.className = 'winding-cell';

    const label = document.createElement('div');
    label.className = 'winding-name';
    label.textContent = wr;

    const canvas = makeCanvas(SIZE, SIZE);
    const result = tessellate(contours, wr);

    if (result && result.triangleCount > 0) {
      drawTessellation(canvas, result.vertices, result.elements, { fillColor: COLORS.fill });
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
