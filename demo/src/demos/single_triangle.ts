// Single Triangle demo
import { tessellate } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Single Triangle</h2>
      <p class="demo-description">
        The simplest possible polygon: three vertices, one triangle output.
        The tessellator correctly identifies the interior winding and produces exactly 1 triangle.
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
        <div class="control-group">
          <label><input type="checkbox" id="show-input" checked> Show input contour</label>
        </div>
      </div>
      <div class="stats-bar" id="stats"></div>
      <div class="info-box">
        <strong>Input:</strong> vertices (0,0), (0,1), (1,0) — a right triangle.<br>
        <strong>Output:</strong> 1 triangle, 3 vertices in the tessellated mesh.
      </div>
    </div>`;

  const area = container.querySelector('#canvas-area')!;
  const statsEl = container.querySelector('#stats')!;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;
  const showInputCb = container.querySelector('#show-input') as HTMLInputElement;

  const VERTS = [0, 0,  0, 1,  1, 0];
  const canvas = makeCanvas(320, 320);
  area.appendChild(canvas);

  function render() {
    const winding = ['Odd','NonZero','Positive','Negative','AbsGeqTwo'][parseInt(windingSel.value)] as any;
    const result = tessellate([{ vertices: VERTS, reversed: false }], winding);
    if (!result) { statsEl.textContent = 'Tessellation failed'; return; }
    drawTessellation(canvas, result.vertices, result.elements, {
      fillColor: COLORS.fill,
      showInput: showInputCb.checked,
      inputContours: [VERTS],
    });
    statsEl.textContent = `${result.elementCount} triangle(s) · ${result.vertices.length/2} vertices`;
  }

  windingSel.addEventListener('change', render);
  showInputCb.addEventListener('change', render);
  render();
}
