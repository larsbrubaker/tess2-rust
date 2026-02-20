// Basic Shapes demo — a range of shapes from simple to multi-contour,
// chosen so each winding rule produces visibly different results.
import { tessellate, WINDING_RULES, WindingRule, Contour } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';
import { persistControls } from '../state.ts';

interface ShapeDef {
  name: string;
  contours: Contour[];
  description: string;
}

function starPath(cx: number, cy: number, r: number): number[] {
  const v: number[] = [];
  const step = (Math.PI * 2) / 5;
  for (let i = 0; i < 5; i++) {
    const a = -Math.PI / 2 + i * step * 2;
    v.push(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  return v;
}

const SHAPES: ShapeDef[] = [
  {
    name: 'Triangle',
    contours: [{ vertices: [0, 0,  0, 1,  1, 0], reversed: false }],
    description: 'Simplest polygon. All rules agree.',
  },
  {
    name: 'L-Shape',
    contours: [{ vertices: [0, 0,  0, 3,  1, 3,  1, 1,  3, 1,  3, 0], reversed: false }],
    description: 'Concave single contour. All rules agree.',
  },
  {
    name: 'Square with Hole',
    contours: [
      { vertices: [0, 0, 4, 0, 4, 4, 0, 4], reversed: false },
      { vertices: [1, 1, 3, 1, 3, 3, 1, 3], reversed: true },
    ],
    description: 'Hole via reversed inner contour. Negative fills only the hole region.',
  },
  {
    name: 'Overlapping Squares',
    contours: [
      { vertices: [-2, -2, 1, -2, 1, 1, -2, 1], reversed: false },
      { vertices: [-1, -1, 2, -1, 2, 2, -1, 2], reversed: false },
    ],
    description: 'Two separate CCW squares overlap. Odd excludes the overlap; AbsGeqTwo fills only it.',
  },
  {
    name: 'Pentagram',
    contours: [{ vertices: starPath(0, 0, 2), reversed: false }],
    description: 'Self-intersecting star. The center has winding 2, so AbsGeqTwo fills only that.',
  },
  {
    name: 'Bowtie',
    contours: [{
      vertices: [-1.5, -1, 0, 0, 1.5, -1, 1.5, 1, 0, 0, -1.5, 1],
      reversed: false,
    }],
    description: 'Self-intersecting figure-8. Negative fills nothing (all windings positive for CCW).',
  },
  {
    name: 'Nested Rings',
    contours: [
      { vertices: [-3, -3, 3, -3, 3, 3, -3, 3], reversed: false },
      { vertices: [-2, -2, -2, 2, 2, 2, 2, -2], reversed: true },
      { vertices: [-1, -1, 1, -1, 1, 1, -1, 1], reversed: false },
    ],
    description: 'Three layers: CCW, CW, CCW. Odd fills alternating rings. NonZero fills all.',
  },
  {
    name: 'Star of David',
    contours: (() => {
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
    })(),
    description: 'Two overlapping CCW triangles. AbsGeqTwo fills only the central hexagon.',
  },
];

export function init(container: HTMLElement): void {
  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Basic Shapes</h2>
      <p class="demo-description">
        A range of shapes from simple single contours to overlapping and self-intersecting polygons.
        Switch the winding rule to see how each one determines which regions are "inside."
      </p>
      <div class="controls" style="margin-bottom:16px">
        <div class="control-group">
          <label>Winding rule:</label>
          <select id="winding-sel">
            ${WINDING_RULES.map((r, i) => `<option value="${i}"${r === 'Positive' ? ' selected' : ''}>${r}</option>`).join('')}
          </select>
        </div>
        <div class="control-group">
          <label><input type="checkbox" id="show-input" checked> Show input contour</label>
        </div>
      </div>
      <div class="winding-grid" id="shapes-grid"></div>
    </div>`;

  const gridEl = container.querySelector('#shapes-grid')!;
  const windingSel = container.querySelector('#winding-sel') as HTMLSelectElement;
  const showInputCb = container.querySelector('#show-input') as HTMLInputElement;
  const SIZE = 180;

  function render() {
    gridEl.innerHTML = '';
    const wr = WINDING_RULES[parseInt(windingSel.value)] as WindingRule;

    for (const shape of SHAPES) {
      const cell = document.createElement('div');
      cell.className = 'winding-cell';

      const label = document.createElement('div');
      label.className = 'winding-name';
      label.textContent = shape.name;

      const canvas = makeCanvas(SIZE, SIZE);
      const result = tessellate(shape.contours, wr);
      const inputContours = shape.contours.map(c => c.vertices);

      if (result && result.elementCount > 0) {
        drawTessellation(canvas, result.vertices, result.elements, {
          fillColor: COLORS.fill,
          showInput: showInputCb.checked,
          inputContours,
        });
      } else {
        const ctx = canvas.getContext('2d')!;
        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#aaa';
        ctx.textAlign = 'center';
        ctx.fillText('(empty)', SIZE / 2, SIZE / 2);
      }

      const info = document.createElement('div');
      info.className = 'winding-tri-count';
      info.textContent = result ? `${result.elementCount} tri` : 'failed';

      const desc = document.createElement('div');
      desc.style.cssText = 'font-size:11px; color:#888; margin-top:4px; text-align:center; max-width:180px;';
      desc.textContent = shape.description;

      cell.appendChild(label);
      cell.appendChild(canvas);
      cell.appendChild(info);
      cell.appendChild(desc);
      gridEl.appendChild(cell);
    }
  }

  persistControls('basic_shapes', container);
  windingSel.addEventListener('change', render);
  showInputCb.addEventListener('change', render);
  render();
}
