// WASM module loader for tess2-rust demos

let wasmModule: any = null;

export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  const wasmUrl = new URL('./public/pkg/tess2_wasm.js', window.location.href).href;
  const mod = await import(/* @vite-ignore */ wasmUrl);
  await mod.default();
  wasmModule = mod;
}

function getWasm(): any {
  if (!wasmModule) throw new Error('WASM not initialized. Call initWasm() first.');
  return wasmModule;
}

export type WindingRule = 'Odd' | 'NonZero' | 'Positive' | 'Negative' | 'AbsGeqTwo';
export const WINDING_RULES: WindingRule[] = ['Odd', 'NonZero', 'Positive', 'Negative', 'AbsGeqTwo'];

export type ElementTypeStr = 'Polygons' | 'ConnectedPolygons' | 'BoundaryContours';
export const ELEMENT_TYPES: ElementTypeStr[] = ['Polygons', 'ConnectedPolygons', 'BoundaryContours'];

export function windingRuleIndex(wr: WindingRule): number {
  return WINDING_RULES.indexOf(wr);
}

export function elementTypeIndex(et: ElementTypeStr): number {
  return ELEMENT_TYPES.indexOf(et);
}

export interface TessResult {
  vertices: Float32Array;
  elements: Uint32Array;
  elementCount: number;
  vertexCount: number;
}

export interface Contour {
  vertices: number[];
  reversed: boolean;
}

export interface TessOptions {
  winding?: WindingRule;
  elementType?: ElementTypeStr;
  polySize?: number;
}

/** Tessellate one or more contours with the given options. */
export function tessellate(contours: Contour[], windingOrOpts?: WindingRule | TessOptions): TessResult | null {
  const opts: TessOptions = typeof windingOrOpts === 'string'
    ? { winding: windingOrOpts }
    : (windingOrOpts ?? {});
  const winding = opts.winding ?? 'Positive';
  const elementType = opts.elementType ?? 'Polygons';
  const polySize = opts.polySize ?? 3;

  const w = getWasm();
  const t = new w.TessellatorJs();
  for (const c of contours) {
    t.set_option(1, c.reversed);
    t.add_contour(new Float32Array(c.vertices));
  }
  const ok = t.tessellate_full(
    windingRuleIndex(winding),
    elementTypeIndex(elementType),
    polySize,
  );
  if (!ok) { t.free(); return null; }

  const verts = new Float32Array(t.get_vertices());
  const elems = new Uint32Array(t.get_elements());
  const count = t.element_count();
  const vcount = t.vertex_count();
  t.free();
  return { vertices: verts, elements: elems, elementCount: count, vertexCount: vcount };
}
