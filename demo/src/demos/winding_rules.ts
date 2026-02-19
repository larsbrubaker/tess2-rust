// Winding Rules Comparison demo — shows a self-intersecting polygon under all 5 rules
import { tessellate, WINDING_RULES, WindingRule } from '../wasm.ts';
import { makeCanvas, drawTessellation, COLORS } from '../render-canvas.ts';

// Self-intersecting star polygon (5-pointed, drawn as a continuous path)
function starPath(cx: number, cy: number, r: number, points: number): number[] {
  const verts: number[] = [];
  const step = (Math.PI * 2) / points;
  const skip = 2; // connect every-other vertex (pentagram)
  for (let i = 0; i < points; i++) {
    const angle = -Math.PI / 2 + i * step * skip;
    verts.push(cx + Math.cos(angle) * r, cy + Math.sin(angle) * r);
  }
  return verts;
}

// Nested rectangles: outer CCW, middle CW, inner CCW
function nestedRects(): number[][] {
  return [
    [-3,-2,  3,-2,  3,2,  -3,2],     // outer CCW
    [-2,-1,  -2,1,  2,1,  2,-1],     // middle CW (reversed)
    [-1,-0.5, 1,-0.5, 1,0.5, -1,0.5],// inner CCW
  ];
}

export function init(container: HTMLElement): void {
  const SHAPE_OPTIONS = ['5-pointed star', 'Self-intersecting quad', 'Nested rectangles'] as const;
  type ShapeKey = typeof SHAPE_OPTIONS[number];

  container.innerHTML = `
    <div class="demo-page">
      <h2 class="demo-title">Winding Rules Comparison</h2>
      <p class="demo-description">
        The same polygon rendered under all five winding rules simultaneously.
        Self-intersecting or nested contours reveal how each rule decides which
        regions are "inside" the shape.
      </p>
      <div class="controls" style="margin-bottom:16px">
        <div class="control-group">
          <label>Shape:</label>
          <select id="shape-sel">
            ${SHAPE_OPTIONS.map((s,i) => `<option value="${i}">${s}</option>`).join('')}
          </select>
        </div>
      </div>
      <div class="winding-grid" id="winding-grid"></div>
      <div class="info-box" id="info-box"></div>
    </div>`;

  const gridEl = container.querySelector('#winding-grid')!;
  const shapeSel = container.querySelector('#shape-sel') as HTMLSelectElement;
  const infoEl  = container.querySelector('#info-box')!;

  const SIZE = 140;

  const RULE_DESCRIPTIONS: Record<WindingRule, string> = {
    Odd:       'Inside if winding number is odd. Classic "even-odd" fill.',
    NonZero:   'Inside if winding number is non-zero. Standard fill rule.',
    Positive:  'Inside if winding number &gt; 0. CCW contours only.',
    Negative:  'Inside if winding number &lt; 0. CW contours only.',
    AbsGeqTwo: 'Inside if |winding| ≥ 2. Only doubly-wound regions.',
  };

  function getContours(shapeIdx: number): { vertices: number[]; reversed: boolean }[] {
    switch (shapeIdx) {
      case 0: return [{ vertices: starPath(0, 0, 1, 5), reversed: false }];
      case 1: {
        // Butterfly / figure-8: a self-intersecting bowtie contour
        const bv = [-1.5,-1, 0,0, 1.5,-1, 1.5,1, 0,0, -1.5,1];
        return [{ vertices: bv, reversed: false }];
      }
      case 2: {
        const r = nestedRects();
        return [
          { vertices: r[0], reversed: false },
          { vertices: r[1], reversed: true },
          { vertices: r[2], reversed: false },
        ];
      }
      default: return [{ vertices: starPath(0, 0, 1, 5), reversed: false }];
    }
  }

  function render() {
    const shapeIdx = parseInt(shapeSel.value);
    const contours = getContours(shapeIdx);

    gridEl.innerHTML = '';
    for (const wr of WINDING_RULES) {
      const cell = document.createElement('div');
      cell.className = 'winding-cell';

      const label = document.createElement('div');
      label.className = 'winding-name';
      label.textContent = wr;

      const canvas = makeCanvas(SIZE, SIZE);
      const result = tessellate(contours, wr);

      if (result && result.triangleCount > 0) {
        drawTessellation(canvas, result.vertices, result.elements, {
          fillColor: COLORS.fill,
          wireframe: true,
        });
      } else {
        // Nothing inside — draw input outline only
        const ctx = canvas.getContext('2d')!;
        ctx.clearRect(0, 0, SIZE, SIZE);
        ctx.strokeStyle = 'rgba(180,180,180,0.8)';
        ctx.lineWidth = 1;
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

    // Show description of selected shape
    infoEl.innerHTML = WINDING_RULES.map(wr =>
      `<strong>${wr}:</strong> ${RULE_DESCRIPTIONS[wr]}<br>`
    ).join('');
  }

  shapeSel.addEventListener('change', render);
  render();
}
