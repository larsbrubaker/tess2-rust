// Output Modes demo — element types (Polygons, ConnectedPolygons, BoundaryContours) and polygon sizes
import { tessellate, WINDING_RULES, ELEMENT_TYPES, ElementTypeStr } from '../wasm.ts';
import { makeCanvas, COLORS, rgba } from '../render-canvas.ts';
import { persistControls } from '../state.ts';

function hexagonVerts(cx: number, cy: number, r: number): number[] {
  const v: number[] = [];
  for (let i = 0; i < 6; i++) {
    const a = (i * 2 * Math.PI) / 6;
    v.push(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  return v;
}

const DEMO_CONTOURS = [
  { vertices: [0, 0, 6, 0, 6, 6, 0, 6], reversed: false },
  { vertices: [1.5, 1.5, 4.5, 1.5, 4.5, 4.5, 1.5, 4.5], reversed: true },
];
const INPUT_CONTOURS = DEMO_CONTOURS.map(c => c.vertices);

function computeTransform(
  vertices: Float32Array,
  W: number,
  H: number,
  pad: number,
  extraContours?: number[][],
): { toScreen: (x: number, y: number) => [number, number] } {
  let xmin = Infinity, xmax = -Infinity, ymin = Infinity, ymax = -Infinity;
  for (let i = 0; i < vertices.length; i += 2) {
    xmin = Math.min(xmin, vertices[i]);
    xmax = Math.max(xmax, vertices[i]);
    ymin = Math.min(ymin, vertices[i + 1]);
    ymax = Math.max(ymax, vertices[i + 1]);
  }
  if (extraContours) {
    for (const c of extraContours) {
      for (let i = 0; i < c.length; i += 2) {
        xmin = Math.min(xmin, c[i]);
        xmax = Math.max(xmax, c[i]);
        ymin = Math.min(ymin, c[i + 1]);
        ymax = Math.max(ymax, c[i + 1]);
      }
    }
  }
  const rangeX = xmax - xmin || 1;
  const rangeY = ymax - ymin || 1;
  const scale = Math.min((W - pad * 2) / rangeX, (H - pad * 2) / rangeY);
  const offX = pad + ((W - pad * 2) - rangeX * scale) / 2 - xmin * scale;
  const offY = pad + ((H - pad * 2) - rangeY * scale) / 2 - ymin * scale;
  return {
    toScreen: (x: number, y: number) => [x * scale + offX, y * scale + offY],
  };
}

const POLYGON_COLORS = [
  'rgba(26,122,94,0.45)',
  'rgba(26,80,160,0.45)',
  'rgba(160,60,20,0.35)',
  'rgba(100,26,160,0.35)',
  'rgba(26,160,80,0.35)',
  'rgba(160,160,26,0.35)',
];

function drawPolygonsOutput(
  canvas: HTMLCanvasElement,
  vertices: Float32Array,
  elements: Uint32Array,
  polySize: number,
  showInput: boolean,
) {
  const ctx = canvas.getContext('2d')!;
  const W = canvas.width, H = canvas.height;
  ctx.clearRect(0, 0, W, H);

  const { toScreen } = computeTransform(vertices, W, H, 24, showInput ? INPUT_CONTOURS : undefined);

  const numPolys = elements.length / polySize;
  for (let p = 0; p < numPolys; p++) {
    const off = p * polySize;
    const color = POLYGON_COLORS[p % POLYGON_COLORS.length];

    ctx.fillStyle = color;
    ctx.beginPath();
    let started = false;
    for (let j = 0; j < polySize; j++) {
      const idx = elements[off + j];
      if (idx === 0xFFFFFFFF) break;
      const [sx, sy] = toScreen(vertices[idx * 2], vertices[idx * 2 + 1]);
      if (!started) { ctx.moveTo(sx, sy); started = true; }
      else ctx.lineTo(sx, sy);
    }
    ctx.closePath();
    ctx.fill();

    ctx.strokeStyle = 'rgba(255,255,255,0.8)';
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  if (showInput) {
    for (const c of INPUT_CONTOURS) {
      ctx.strokeStyle = rgba(COLORS.inputStroke);
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      const [sx, sy] = toScreen(c[0], c[1]);
      ctx.moveTo(sx, sy);
      for (let i = 2; i < c.length; i += 2) {
        const [px, py] = toScreen(c[i], c[i + 1]);
        ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}

function drawConnectedOutput(
  canvas: HTMLCanvasElement,
  vertices: Float32Array,
  elements: Uint32Array,
  polySize: number,
  showInput: boolean,
) {
  const ctx = canvas.getContext('2d')!;
  const W = canvas.width, H = canvas.height;
  ctx.clearRect(0, 0, W, H);

  const { toScreen } = computeTransform(vertices, W, H, 24, showInput ? INPUT_CONTOURS : undefined);

  const stride = polySize * 2;
  const numPolys = elements.length / stride;

  for (let p = 0; p < numPolys; p++) {
    const off = p * stride;
    const color = POLYGON_COLORS[p % POLYGON_COLORS.length];

    ctx.fillStyle = color;
    ctx.beginPath();
    let started = false;
    for (let j = 0; j < polySize; j++) {
      const idx = elements[off + j];
      if (idx === 0xFFFFFFFF) break;
      const [sx, sy] = toScreen(vertices[idx * 2], vertices[idx * 2 + 1]);
      if (!started) { ctx.moveTo(sx, sy); started = true; }
      else ctx.lineTo(sx, sy);
    }
    ctx.closePath();
    ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,0.7)';
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  // Draw neighbor connections
  ctx.strokeStyle = 'rgba(0,0,0,0.5)';
  ctx.lineWidth = 1.5;
  for (let p = 0; p < numPolys; p++) {
    const off = p * stride;
    let cx = 0, cy = 0, cn = 0;
    for (let j = 0; j < polySize; j++) {
      const idx = elements[off + j];
      if (idx === 0xFFFFFFFF) break;
      cx += vertices[idx * 2];
      cy += vertices[idx * 2 + 1];
      cn++;
    }
    cx /= cn;
    cy /= cn;
    const [scx, scy] = toScreen(cx, cy);

    for (let j = 0; j < polySize; j++) {
      const nei = elements[off + polySize + j];
      if (nei === 0xFFFFFFFF) continue;
      if (nei < p) continue;

      const noff = nei * stride;
      let ncx = 0, ncy = 0, ncn = 0;
      for (let k = 0; k < polySize; k++) {
        const nidx = elements[noff + k];
        if (nidx === 0xFFFFFFFF) break;
        ncx += vertices[nidx * 2];
        ncy += vertices[nidx * 2 + 1];
        ncn++;
      }
      ncx /= ncn;
      ncy /= ncn;
      const [sncx, sncy] = toScreen(ncx, ncy);

      ctx.beginPath();
      const dx = sncx - scx;
      const dy = sncy - scy;
      ctx.moveTo(scx, scy);
      ctx.quadraticCurveTo(scx + dx * 0.5 + dy * 0.3, scy + dy * 0.5 - dx * 0.3, sncx, sncy);
      ctx.stroke();
    }

    ctx.fillStyle = 'rgba(0,0,0,0.6)';
    ctx.beginPath();
    ctx.arc(scx, scy, 3, 0, Math.PI * 2);
    ctx.fill();
  }

  if (showInput) {
    for (const c of INPUT_CONTOURS) {
      ctx.strokeStyle = rgba(COLORS.inputStroke);
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      const [sx, sy] = toScreen(c[0], c[1]);
      ctx.moveTo(sx, sy);
      for (let i = 2; i < c.length; i += 2) {
        const [px, py] = toScreen(c[i], c[i + 1]);
        ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}

function drawBoundaryOutput(
  canvas: HTMLCanvasElement,
  vertices: Float32Array,
  elements: Uint32Array,
  showInput: boolean,
) {
  const ctx = canvas.getContext('2d')!;
  const W = canvas.width, H = canvas.height;
  ctx.clearRect(0, 0, W, H);

  const { toScreen } = computeTransform(vertices, W, H, 24, showInput ? INPUT_CONTOURS : undefined);

  for (let i = 0; i < elements.length; i += 2) {
    const start = elements[i];
    const count = elements[i + 1];
    const color = POLYGON_COLORS[(i / 2) % POLYGON_COLORS.length];

    ctx.fillStyle = color;
    ctx.beginPath();
    for (let j = 0; j < count; j++) {
      const idx = start + j;
      const [sx, sy] = toScreen(vertices[idx * 2], vertices[idx * 2 + 1]);
      if (j === 0) ctx.moveTo(sx, sy);
      else ctx.lineTo(sx, sy);
    }
    ctx.closePath();
    ctx.fill();

    ctx.strokeStyle = 'rgba(255,255,255,0.8)';
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  if (showInput) {
    for (const c of INPUT_CONTOURS) {
      ctx.strokeStyle = rgba(COLORS.inputStroke);
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      const [sx, sy] = toScreen(c[0], c[1]);
      ctx.moveTo(sx, sy);
      for (let i = 2; i < c.length; i += 2) {
        const [px, py] = toScreen(c[i], c[i + 1]);
        ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Output Modes</h2>
      <p class="demo-description">
        The tessellator supports three element types and configurable polygon sizes.
        A square with a hole is tessellated with different output settings to show
        what each mode produces.
      </p>
      <div class="controls" style="margin-bottom:16px">
        <div class="control-group">
          <label>Element type:</label>
          <select id="et-sel">
            ${ELEMENT_TYPES.map((e, i) => `<option value="${i}">${e}</option>`).join('')}
          </select>
        </div>
        <div class="control-group" id="ps-group">
          <label>Polygon size:</label>
          <select id="ps-sel">
            <option value="3" selected>3 (triangles)</option>
            <option value="4">4 (quads)</option>
            <option value="6">6 (hexagons)</option>
            <option value="10">10</option>
            <option value="16">16</option>
          </select>
        </div>
        <div class="control-group">
          <label>Winding rule:</label>
          <select id="winding-sel">
            ${WINDING_RULES.map((r, i) => `<option value="${i}"${r === 'Positive' ? ' selected' : ''}>${r}</option>`).join('')}
          </select>
        </div>
        <div class="control-group">
          <label><input type="checkbox" id="show-input" checked> Show input</label>
        </div>
      </div>
      <div id="canvas-area" style="display:flex;justify-content:center"></div>
      <div class="stats-bar" id="stats"></div>
      <div class="info-box" id="info-box"></div>
    </div>`;

  const canvasArea = container.querySelector('#canvas-area')!;
  const etSel = container.querySelector('#et-sel') as HTMLSelectElement;
  const psSel = container.querySelector('#ps-sel') as HTMLSelectElement;
  const psGroup = container.querySelector('#ps-group') as HTMLElement;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;
  const showInputCb = container.querySelector('#show-input') as HTMLInputElement;
  const statsEl = container.querySelector('#stats')!;
  const infoEl = container.querySelector('#info-box')!;
  const canvas = makeCanvas(400, 400);
  canvasArea.appendChild(canvas);

  const DESCRIPTIONS: Record<ElementTypeStr, string> = {
    Polygons: `<strong>Polygons:</strong> Output is a flat list of convex polygons (typically triangles when poly_size=3).
      Each polygon is defined by up to <em>poly_size</em> vertex indices. Unused slots are set to -1.
      Increasing poly_size merges adjacent triangles into larger convex polygons where possible.`,
    ConnectedPolygons: `<strong>Connected Polygons:</strong> Same as Polygons, but each polygon also stores neighbor info —
      for each edge, the index of the adjacent polygon (-1 if boundary). The curved lines show neighbor connections.
      Useful for pathfinding or mesh traversal.`,
    BoundaryContours: `<strong>Boundary Contours:</strong> Instead of triangles, the output is a set of closed contour loops
      describing the boundary of the filled region. Elements are (start_vertex, count) pairs.
      Polygon size is ignored. Useful for rendering outlines or computing silhouettes.`,
  };

  function render() {
    const et = ELEMENT_TYPES[parseInt(etSel.value)] as ElementTypeStr;
    const polySize = parseInt(psSel.value);
    const wr = WINDING_RULES[parseInt(windingSel.value)];
    const showInput = showInputCb.checked;

    psGroup.style.opacity = et === 'BoundaryContours' ? '0.4' : '1';

    const result = tessellate(DEMO_CONTOURS, { winding: wr, elementType: et, polySize });

    if (!result || result.elementCount === 0) {
      const ctx = canvas.getContext('2d')!;
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.font = '14px sans-serif';
      ctx.fillStyle = '#aaa';
      ctx.textAlign = 'center';
      ctx.fillText(result ? '(empty)' : 'Tessellation failed', canvas.width / 2, canvas.height / 2);
      statsEl.textContent = result ? '0 elements' : 'Failed';
      infoEl.innerHTML = DESCRIPTIONS[et];
      return;
    }

    if (et === 'Polygons') {
      drawPolygonsOutput(canvas, result.vertices, result.elements, polySize, showInput);
    } else if (et === 'ConnectedPolygons') {
      drawConnectedOutput(canvas, result.vertices, result.elements, polySize, showInput);
    } else {
      drawBoundaryOutput(canvas, result.vertices, result.elements, showInput);
    }

    statsEl.textContent = `${result.elementCount} element(s) · ${result.vertexCount} vertices · poly_size=${et === 'BoundaryContours' ? 'N/A' : polySize}`;
    infoEl.innerHTML = DESCRIPTIONS[et];
  }

  persistControls('output_modes', container);
  etSel.addEventListener('change', render);
  psSel.addEventListener('change', render);
  windingSel.addEventListener('change', render);
  showInputCb.addEventListener('change', render);
  render();
}
