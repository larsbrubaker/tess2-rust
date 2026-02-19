// Polygon with Hole demo
import { tessellate } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Polygon with Hole</h2>
      <p class="demo-description">
        An outer square (3×3) with an inner square hole (1×1 centred at (1.5,1.5)).
        The sweep-line algorithm correctly partitions the donut region into
        <strong>8 triangles</strong> — the reference result from the original libtess2 test suite.
      </p>
      <div id="canvas-area"></div>
      <div class="controls">
        <div class="control-group">
          <label>Winding rule:</label>
          <select id="winding-sel">
            <option value="2" selected>Positive</option>
            <option value="0">Odd</option>
            <option value="1">NonZero</option>
          </select>
        </div>
      </div>
      <div class="stats-bar" id="stats"></div>
      <div class="info-box">
        <strong>Key insight:</strong> The hole contour is added with <code>ReverseContours=true</code>
        so that its winding is subtracted from the outer polygon, creating the hole effect under the
        <em>Positive</em> winding rule (inside = winding &gt; 0).
      </div>
    </div>`;

  const area = container.querySelector('#canvas-area')!;
  const statsEl = container.querySelector('#stats')!;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;

  const outer = [0,0, 3,0, 3,3, 0,3];
  const inner = [1,1, 2,1, 2,2, 1,2];
  const canvas = makeCanvas(360, 360);
  area.appendChild(canvas);

  function render() {
    const winding = ['Odd','NonZero','Positive','Negative','AbsGeqTwo'][parseInt(windingSel.value)] as any;
    const result = tessellate(
      [
        { vertices: outer, reversed: false },
        { vertices: inner, reversed: true },
      ],
      winding,
    );
    if (!result) { statsEl.textContent = 'Tessellation failed'; return; }
    drawTessellation(canvas, result.vertices, result.elements, {
      fillColor: COLORS.fill,
      showInput: true,
      fillInput: false,
      inputContours: [outer, inner],
    });
    statsEl.textContent = `${result.triangleCount} triangle(s) · ${result.vertices.length/2} vertices`;
  }

  windingSel.addEventListener('change', render);
  render();
}
