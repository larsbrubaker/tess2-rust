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

export function windingRuleIndex(wr: WindingRule): number {
  return WINDING_RULES.indexOf(wr);
}

export interface TessResult {
  vertices: Float32Array;  // [x0,y0, x1,y1, ...]
  elements: Uint32Array;   // [i0,i1,i2, ...]  (triangle indices)
  triangleCount: number;
}

export interface Contour {
  vertices: number[];  // [x0,y0, x1,y1, ...]
  reversed: boolean;
}

/** Tessellate one or more contours with the given winding rule. */
export function tessellate(contours: Contour[], winding: WindingRule): TessResult | null {
  const w = getWasm();
  const t = new w.TessellatorJs();
  for (const c of contours) {
    t.set_option(1, c.reversed);  // 1 = ReverseContours
    t.add_contour(new Float32Array(c.vertices));
  }
  const ok = t.tessellate(windingRuleIndex(winding));
  if (!ok) { t.free(); return null; }

  const verts = new Float32Array(t.get_vertices());
  const elems = new Uint32Array(t.get_elements());
  const count = t.element_count();
  t.free();
  return { vertices: verts, elements: elems, triangleCount: count };
}
