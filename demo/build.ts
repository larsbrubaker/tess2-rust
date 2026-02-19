// Build script for tess2-rust demo
// Usage: bun run build.ts
//
// Output: dist/ directory:
//   dist/index.html
//   dist/styles/main.css
//   dist/public/dist/main.js
//   dist/public/pkg/   (WASM — must be built separately via wasm-pack first)

import { join } from "path";
import { mkdirSync, cpSync, existsSync, rmSync } from "fs";

const DEMO_DIR = import.meta.dir;
const DIST_DIR = join(DEMO_DIR, "dist");
const PUBLIC_DIST_DIR = join(DEMO_DIR, "public", "dist");

// Step 1: Bundle TypeScript
if (existsSync(PUBLIC_DIST_DIR)) {
  rmSync(PUBLIC_DIST_DIR, { recursive: true, force: true });
}
mkdirSync(PUBLIC_DIST_DIR, { recursive: true });

const result = await Bun.build({
  entrypoints: ["./src/main.ts"],
  outdir: "./public/dist",
  write: true,
  splitting: false,
  format: "esm",
  minify: false,
  sourcemap: "external",
  target: "browser",
});

if (!result.success) {
  console.error("Build failed:");
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
console.log(`Bundled ${result.outputs.length} JS file(s) to public/dist/`);

// Step 2: Assemble dist/
if (existsSync(DIST_DIR)) {
  rmSync(DIST_DIR, { recursive: true, force: true });
}
mkdirSync(DIST_DIR, { recursive: true });

cpSync(join(DEMO_DIR, "index.html"), join(DIST_DIR, "index.html"));
console.log("  Copied index.html");

cpSync(join(DEMO_DIR, "styles"), join(DIST_DIR, "styles"), { recursive: true });
console.log("  Copied styles/");

cpSync(join(DEMO_DIR, "public", "dist"), join(DIST_DIR, "public", "dist"), { recursive: true });
console.log("  Copied public/dist/ (JS)");

const pkgDir = join(DEMO_DIR, "public", "pkg");
if (existsSync(pkgDir)) {
  cpSync(pkgDir, join(DIST_DIR, "public", "pkg"), { recursive: true });
  console.log("  Copied public/pkg/ (WASM)");
} else {
  console.warn("  WARNING: public/pkg/ not found — run wasm-pack first!");
}

console.log("\nBuild complete → dist/");
