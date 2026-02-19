// Canvas 2D rendering helpers for tessellated polygons

export interface Color { r: number; g: number; b: number; a: number; }

export const COLORS = {
  fill:     { r: 26,  g: 122, b: 94,  a: 0.55 },
  fillAlt:  { r: 26,  g: 80,  b: 160, a: 0.55 },
  stroke:   { r: 26,  g: 122, b: 94,  a: 1.0  },
  wireframe:{ r: 255, g: 255, b: 255, a: 0.7  },
  input:    { r: 200, g: 40,  b: 20,  a: 0.25 },
  inputStroke: { r: 200, g: 40, b: 20, a: 0.9 },
  hole:     { r: 160, g: 60,  b: 20,  a: 0.35 },
};

export function rgba(c: Color): string {
  return `rgba(${c.r},${c.g},${c.b},${c.a})`;
}

export function makeCanvas(width: number, height: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = width;
  c.height = height;
  c.className = 'demo-canvas';
  return c;
}

export interface DrawOptions {
  fillColor?: Color;
  wireframe?: boolean;
  showInput?: boolean;
  inputContours?: number[][];   // [x0,y0,x1,y1,...] per contour
  padding?: number;
}

/** Draw tessellated triangles onto a canvas, auto-scaling to fit. */
export function drawTessellation(
  canvas: HTMLCanvasElement,
  vertices: Float32Array,
  elements: Uint32Array,
  opts: DrawOptions = {},
): void {
  const ctx = canvas.getContext('2d')!;
  const W = canvas.width, H = canvas.height;
  ctx.clearRect(0, 0, W, H);

  const fill   = opts.fillColor ?? COLORS.fill;
  const pad    = opts.padding ?? 24;

  // Compute bounding box from all vertices (tessellated + input contours)
  let xmin = Infinity, xmax = -Infinity, ymin = Infinity, ymax = -Infinity;
  for (let i = 0; i < vertices.length; i += 2) {
    xmin = Math.min(xmin, vertices[i]);
    xmax = Math.max(xmax, vertices[i]);
    ymin = Math.min(ymin, vertices[i+1]);
    ymax = Math.max(ymax, vertices[i+1]);
  }
  if (opts.inputContours) {
    for (const c of opts.inputContours) {
      for (let i = 0; i < c.length; i += 2) {
        xmin = Math.min(xmin, c[i]);
        xmax = Math.max(xmax, c[i]);
        ymin = Math.min(ymin, c[i+1]);
        ymax = Math.max(ymax, c[i+1]);
      }
    }
  }

  if (!isFinite(xmin)) return;

  const rangeX = xmax - xmin || 1;
  const rangeY = ymax - ymin || 1;
  const scaleX = (W - pad * 2) / rangeX;
  const scaleY = (H - pad * 2) / rangeY;
  const scale  = Math.min(scaleX, scaleY);
  const offX   = pad + ((W - pad * 2) - rangeX * scale) / 2 - xmin * scale;
  const offY   = pad + ((H - pad * 2) - rangeY * scale) / 2 - ymin * scale;

  function toScreen(x: number, y: number): [number, number] {
    return [x * scale + offX, y * scale + offY];
  }

  // Draw filled triangles
  if (elements.length > 0) {
    ctx.fillStyle = rgba(fill);
    for (let i = 0; i < elements.length; i += 3) {
      const i0 = elements[i] * 2, i1 = elements[i+1] * 2, i2 = elements[i+2] * 2;
      const [x0, y0] = toScreen(vertices[i0], vertices[i0+1]);
      const [x1, y1] = toScreen(vertices[i1], vertices[i1+1]);
      const [x2, y2] = toScreen(vertices[i2], vertices[i2+1]);
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.closePath();
      ctx.fill();
    }

    // Wireframe edges
    if (opts.wireframe !== false) {
      ctx.strokeStyle = rgba(COLORS.wireframe);
      ctx.lineWidth = 0.8;
      for (let i = 0; i < elements.length; i += 3) {
        const i0 = elements[i] * 2, i1 = elements[i+1] * 2, i2 = elements[i+2] * 2;
        const [x0, y0] = toScreen(vertices[i0], vertices[i0+1]);
        const [x1, y1] = toScreen(vertices[i1], vertices[i1+1]);
        const [x2, y2] = toScreen(vertices[i2], vertices[i2+1]);
        ctx.beginPath();
        ctx.moveTo(x0, y0);
        ctx.lineTo(x1, y1);
        ctx.lineTo(x2, y2);
        ctx.closePath();
        ctx.stroke();
      }
    }
  }

  // Draw input contours
  if (opts.showInput !== false && opts.inputContours) {
    for (const c of opts.inputContours) {
      if (c.length < 4) continue;
      ctx.fillStyle = rgba(COLORS.input);
      ctx.strokeStyle = rgba(COLORS.inputStroke);
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      const [sx, sy] = toScreen(c[0], c[1]);
      ctx.moveTo(sx, sy);
      for (let i = 2; i < c.length; i += 2) {
        const [px, py] = toScreen(c[i], c[i+1]);
        ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.fill();
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}
