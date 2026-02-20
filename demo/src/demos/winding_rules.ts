// Winding Rules Comparison demo — shows various shapes under all 5 rules
import { tessellate, WINDING_RULES, WindingRule } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';
import { persistControls } from '../state.ts';

interface ShapeOption {
  name: string;
  description: string;
  getContours(): { vertices: number[]; reversed: boolean }[];
}

function starPath(cx: number, cy: number, r: number, points: number): number[] {
  const verts: number[] = [];
  const step = (Math.PI * 2) / points;
  const skip = 2;
  for (let i = 0; i < points; i++) {
    const angle = -Math.PI / 2 + i * step * skip;
    verts.push(cx + Math.cos(angle) * r, cy + Math.sin(angle) * r);
  }
  return verts;
}

const SHAPE_OPTIONS: ShapeOption[] = [
  {
    name: 'Five-Pointed Star',
    description: `A pentagram drawn as a single self-intersecting contour.
      <strong>Odd/NonZero/Positive:</strong> fill the star arms.
      <strong>Negative:</strong> empty (all windings positive for CCW).
      <strong>AbsGeqTwo:</strong> only the doubly-wound central pentagon.`,
    getContours: () => [{ vertices: starPath(0, 0, 1, 5), reversed: false }],
  },
  {
    name: 'Bowtie (Self-Intersecting)',
    description: `A bowtie with edges that cross at the center. The tessellator detects the
      intersection and splits it into two lobes with opposite winding.
      <strong>Positive:</strong> one lobe. <strong>Negative:</strong> the other lobe.
      <strong>Odd/NonZero:</strong> both lobes.`,
    getContours: () => [{
      vertices: [-1.5, -1,  1.5, 1,  1.5, -1,  -1.5, 1],
      reversed: false,
    }],
  },
  {
    name: 'Nested Squares (3 layers)',
    description: `Three concentric squares: outer CCW (+1), middle CW (−1), inner CCW (+1).
      <strong>Odd:</strong> alternating rings. <strong>NonZero:</strong> all filled.
      <strong>AbsGeqTwo:</strong> only the innermost ring where |winding| ≥ 2.`,
    getContours: () => [
      { vertices: [-3, -3, 3, -3, 3, 3, -3, 3], reversed: false },
      { vertices: [-2, -2, -2, 2, 2, 2, 2, -2], reversed: true },
      { vertices: [-1, -1, 1, -1, 1, 1, -1, 1], reversed: false },
    ],
  },
  {
    name: 'Square with Hole',
    description: `An outer square (3×3) with an inner hole (1×1). The hole contour is reversed
      so its winding subtracts from the outer. Under <strong>Positive</strong>, the hole is empty;
      under <strong>Odd</strong>, winding alternates.`,
    getContours: () => [
      { vertices: [0, 0, 3, 0, 3, 3, 0, 3], reversed: false },
      { vertices: [1, 1, 2, 1, 2, 2, 1, 2], reversed: true },
    ],
  },
  {
    name: 'Overlapping Squares',
    description: `Two separate CCW squares that partially overlap. Overlapping region has winding 2.
      <strong>Odd:</strong> overlap excluded. <strong>NonZero:</strong> union filled.
      <strong>AbsGeqTwo:</strong> only the intersection.`,
    getContours: () => [
      { vertices: [-2, -2, 1, -2, 1, 1, -2, 1], reversed: false },
      { vertices: [-1, -1, 2, -1, 2, 2, -1, 2], reversed: false },
    ],
  },
  {
    name: 'Six-Pointed Star',
    description: `Two overlapping triangles forming a Star of David. The central hexagonal region
      has winding number 2, so <strong>AbsGeqTwo</strong> fills only the center while
      <strong>Odd</strong> excludes it.`,
    getContours: () => {
      const r = 2;
      const t1: number[] = [];
      const t2: number[] = [];
      for (let i = 0; i < 3; i++) {
        const a1 = -Math.PI / 2 + (i * 2 * Math.PI) / 3;
        t1.push(Math.cos(a1) * r, Math.sin(a1) * r);
        const a2 = Math.PI / 2 + (i * 2 * Math.PI) / 3;
        t2.push(Math.cos(a2) * r, Math.sin(a2) * r);
      }
      return [
        { vertices: t1, reversed: false },
        { vertices: t2, reversed: false },
      ];
    },
  },
];

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Winding Rules Comparison</h2>
      <p class="demo-description">
        The same shape rendered under all five winding rules simultaneously.
        Self-intersecting, nested, and overlapping contours reveal how each rule
        decides which regions are "inside" the shape.
      </p>
      <div class="controls" style="margin-bottom:16px">
        <div class="control-group">
          <label>Shape:</label>
          <select id="shape-sel">
            ${SHAPE_OPTIONS.map((s, i) => `<option value="${i}">${s.name}</option>`).join('')}
          </select>
        </div>
      </div>
      <div class="winding-grid" id="winding-grid"></div>
      <div class="info-box" id="info-box"></div>
    </div>`;

  const gridEl = container.querySelector('#winding-grid')!;
  const shapeSel = container.querySelector('#shape-sel') as HTMLSelectElement;
  const infoEl = container.querySelector('#info-box')!;
  const SIZE = 160;

  function render() {
    const shapeIdx = parseInt(shapeSel.value);
    const shape = SHAPE_OPTIONS[shapeIdx];
    const contours = shape.getContours();
    const inputContours = contours.map(c => c.vertices);

    gridEl.innerHTML = '';
    for (const wr of WINDING_RULES) {
      const cell = document.createElement('div');
      cell.className = 'winding-cell';

      const label = document.createElement('div');
      label.className = 'winding-name';
      label.textContent = wr;

      const canvas = makeCanvas(SIZE, SIZE);
      const result = tessellate(contours, wr);

      if (result && result.elementCount > 0) {
        drawTessellation(canvas, result.vertices, result.elements, {
          fillColor: COLORS.fill,
          wireframe: true,
          showInput: true,
          inputContours,
        });
      } else {
        const ctx = canvas.getContext('2d')!;
        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#aaa';
        ctx.textAlign = 'center';
        ctx.fillText('(empty)', SIZE / 2, SIZE / 2);
      }

      const triCount = document.createElement('div');
      triCount.className = 'winding-tri-count';
      triCount.textContent = result ? `${result.elementCount} tri` : 'failed';

      cell.appendChild(label);
      cell.appendChild(canvas);
      cell.appendChild(triCount);
      gridEl.appendChild(cell);
    }

    infoEl.innerHTML = shape.description;
  }

  persistControls('winding_rules', container);
  shapeSel.addEventListener('change', render);
  render();
}
