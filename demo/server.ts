// Dev server for local demo development.
// Usage: bun run server.ts
// Serves the demo directory with proper MIME types for WASM.

import { join } from "path";
import { existsSync, readFileSync } from "fs";

const DEMO_DIR = import.meta.dir;
const PORT = 3000;

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".css": "text/css",
  ".js": "application/javascript",
  ".mjs": "application/javascript",
  ".wasm": "application/wasm",
  ".ts": "application/javascript",
  ".json": "application/json",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

function extOf(path: string): string {
  const i = path.lastIndexOf(".");
  return i >= 0 ? path.slice(i) : "";
}

const server = Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);
    let pathname = url.pathname;
    if (pathname === "/" || pathname === "") pathname = "/index.html";

    const file = join(DEMO_DIR, pathname);
    if (existsSync(file)) {
      const ext = extOf(file);
      const mime = MIME[ext] ?? "application/octet-stream";
      return new Response(readFileSync(file), {
        headers: {
          "Content-Type": mime,
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
        },
      });
    }

    // SPA fallback
    return new Response(readFileSync(join(DEMO_DIR, "index.html")), {
      headers: { "Content-Type": "text/html" },
    });
  },
});

console.log(`Dev server running at http://localhost:${server.port}`);
console.log(`Make sure to run wasm-pack first: wasm-pack build demo/wasm --target web --out-dir ../public/pkg --no-typescript`);
