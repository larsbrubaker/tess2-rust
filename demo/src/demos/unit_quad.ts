// Unit Quad demo
import { tessellate } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Unit Quad</h2>
      <p class="demo-description">
        A square polygon tessellated into two triangles — the standard result for any
        convex quadrilateral with the SGI tessellator.
      </p>
      <div id="canvas-area"></div>
      <div class="controls">
        <div class="control-group">
          <label>Winding rule:</label>
          <select id="winding-sel">
            <option value="0">Odd</option>
            <option value="2" selected>Positive</option>
            <option value="1">NonZero</option>
          </select>
        </div>
      </div>
      <div class="stats-bar" id="stats"></div>
      <div class="info-box">
        <strong>Input:</strong> 4 vertices forming a unit square (CCW).<br>
        <strong>Output:</strong> 2 triangles — the diagonal split is determined by the sweep line algorithm.
      </div>
    </div>`;

  const area = container.querySelector('#canvas-area')!;
  const statsEl = container.querySelector('#stats')!;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;

  const VERTS = [0, 0,  0, 1,  1, 1,  1, 0];
  const canvas = makeCanvas(320, 320);
  area.appendChild(canvas);

  function render() {
    const winding = ['Odd','NonZero','Positive','Negative','AbsGeqTwo'][parseInt(windingSel.value)] as any;
    const result = tessellate([{ vertices: VERTS, reversed: false }], winding);
    if (!result) { statsEl.textContent = 'Tessellation failed'; return; }
    drawTessellation(canvas, result.vertices, result.elements, {
      fillColor: COLORS.fill,
      showInput: true,
      inputContours: [VERTS],
    });
    statsEl.textContent = `${result.triangleCount} triangle(s) · ${result.vertices.length/2} vertices`;
  }

  windingSel.addEventListener('change', render);
  render();
}
