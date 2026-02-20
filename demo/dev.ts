// Development server with watch mode and live reload.
// Usage: bun run dev.ts
//
// Watches:
//   - Rust files (src/, demo/wasm/src/)  → wasm-pack rebuild + browser reload
//   - TypeScript (demo/src/)             → Bun bundle + browser reload
//   - HTML / CSS (demo/)                 → browser reload only
//
// Injects a tiny SSE-based live-reload client into HTML responses.

import { watch } from "fs";
import { join, resolve } from "path";
import { existsSync, readFileSync, statSync } from "fs";

const DEMO_DIR = import.meta.dir;
const PROJECT_ROOT = resolve(DEMO_DIR, "..");
const PORT = 3000;

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".css": "text/css",
  ".js": "application/javascript",
  ".mjs": "application/javascript",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

function extOf(path: string): string {
  const i = path.lastIndexOf(".");
  return i >= 0 ? path.slice(i) : "";
}

// ---------------------------------------------------------------------------
// Live-reload via Server-Sent Events
// ---------------------------------------------------------------------------

const encoder = new TextEncoder();
const reloadClients = new Set<ReadableStreamDefaultController<Uint8Array>>();

function notifyReload() {
  const msg = encoder.encode("data: reload\n\n");
  for (const c of reloadClients) {
    try {
      c.enqueue(msg);
    } catch {
      reloadClients.delete(c);
    }
  }
}

// Keep SSE connections alive with periodic heartbeat comments
setInterval(() => {
  const ping = encoder.encode(": ping\n\n");
  for (const c of reloadClients) {
    try {
      c.enqueue(ping);
    } catch {
      reloadClients.delete(c);
    }
  }
}, 30_000);

const RELOAD_SNIPPET = `<script>(function(){var e=new EventSource("/__dev_reload");e.onmessage=function(m){if(m.data==="reload")location.reload()}})()</script>`;

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

let wasmBuilding = false;
let tsBuilding = false;

async function buildWasm(): Promise<boolean> {
  if (wasmBuilding) return false;
  wasmBuilding = true;
  const t0 = performance.now();
  process.stdout.write("[wasm] building...");

  try {
    const proc = Bun.spawn(
      [
        "wasm-pack",
        "build",
        "demo/wasm",
        "--target",
        "web",
        "--out-dir",
        "../public/pkg",
        "--no-typescript",
      ],
      { cwd: PROJECT_ROOT, stdout: "pipe", stderr: "pipe" },
    );
    const code = await proc.exited;
    const elapsed = ((performance.now() - t0) / 1000).toFixed(1);

    if (code === 0) {
      console.log(` done (${elapsed}s)`);
      return true;
    }

    const stderr = await new Response(proc.stderr).text();
    console.error(` FAILED (${elapsed}s)\n${stderr}`);
    return false;
  } finally {
    wasmBuilding = false;
  }
}

async function buildTs(): Promise<boolean> {
  if (tsBuilding) return false;
  tsBuilding = true;
  const t0 = performance.now();
  process.stdout.write("[ts]   bundling...");

  try {
    const result = await Bun.build({
      entrypoints: [join(DEMO_DIR, "src", "main.ts")],
      outdir: join(DEMO_DIR, "public", "dist"),
      write: true,
      splitting: false,
      format: "esm",
      minify: false,
      sourcemap: "external",
      target: "browser",
    });

    const elapsed = ((performance.now() - t0) / 1000).toFixed(1);

    if (result.success) {
      console.log(` done (${elapsed}s)`);
      return true;
    }

    console.error(` FAILED (${elapsed}s)`);
    for (const log of result.logs) console.error("  ", log);
    return false;
  } finally {
    tsBuilding = false;
  }
}

// ---------------------------------------------------------------------------
// Debounced watchers
// ---------------------------------------------------------------------------

function debounce(fn: () => void, ms: number): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(fn, ms);
  };
}

const onRustChange = debounce(async () => {
  if (await buildWasm()) notifyReload();
}, 300);

const onTsChange = debounce(async () => {
  if (await buildTs()) notifyReload();
}, 200);

const onStaticChange = debounce(() => {
  console.log("[static] change detected, reloading");
  notifyReload();
}, 200);

function startWatching() {
  const rustDirs = [
    join(PROJECT_ROOT, "src"),
    join(PROJECT_ROOT, "demo", "wasm", "src"),
  ];
  for (const dir of rustDirs) {
    if (existsSync(dir)) {
      watch(dir, { recursive: true }, (_ev, file) => {
        if (file && file.endsWith(".rs")) {
          console.log(`[wasm] change: ${file}`);
          onRustChange();
        }
      });
    }
  }

  watch(join(DEMO_DIR, "src"), { recursive: true }, (_ev, file) => {
    if (file && (file.endsWith(".ts") || file.endsWith(".js"))) {
      console.log(`[ts]   change: ${file}`);
      onTsChange();
    }
  });

  // HTML at demo root
  watch(DEMO_DIR, { recursive: false }, (_ev, file) => {
    if (file && file.endsWith(".html")) {
      console.log(`[html] change: ${file}`);
      onStaticChange();
    }
  });

  // CSS
  const stylesDir = join(DEMO_DIR, "styles");
  if (existsSync(stylesDir)) {
    watch(stylesDir, { recursive: true }, (_ev, file) => {
      if (file && file.endsWith(".css")) {
        console.log(`[css]  change: ${file}`);
        onStaticChange();
      }
    });
  }
}

// ---------------------------------------------------------------------------
// HTTP server
// ---------------------------------------------------------------------------

function isFile(p: string): boolean {
  try {
    return statSync(p).isFile();
  } catch {
    return false;
  }
}

const server = Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);
    let pathname = url.pathname;

    // SSE live-reload endpoint
    if (pathname === "/__dev_reload") {
      let ctrl: ReadableStreamDefaultController<Uint8Array>;
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          ctrl = controller;
          reloadClients.add(controller);
          controller.enqueue(encoder.encode(": connected\n\n"));
        },
        cancel() {
          reloadClients.delete(ctrl);
        },
      });

      req.signal.addEventListener("abort", () => {
        reloadClients.delete(ctrl);
        try { ctrl.close(); } catch { /* already closed */ }
      });

      return new Response(stream, {
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
        },
      });
    }

    if (pathname === "/" || pathname === "") pathname = "/index.html";

    const file = join(DEMO_DIR, pathname);
    if (isFile(file)) {
      const ext = extOf(file);
      const mime = MIME[ext] ?? "application/octet-stream";
      let body: Uint8Array | string = readFileSync(file);

      if (ext === ".html") {
        body = body.toString().replace("</body>", `${RELOAD_SNIPPET}</body>`);
      }

      return new Response(body, {
        headers: {
          "Content-Type": mime,
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
          "Cache-Control": "no-store",
        },
      });
    }

    // SPA fallback
    let html = readFileSync(join(DEMO_DIR, "index.html")).toString();
    html = html.replace("</body>", `${RELOAD_SNIPPET}</body>`);
    return new Response(html, {
      headers: {
        "Content-Type": "text/html",
        "Cache-Control": "no-store",
      },
    });
  },
});

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

console.log("=== tess2-rust dev server ===");
console.log(`    http://localhost:${server.port}\n`);
console.log("Watching for changes:");
console.log("  Rust  (src/, demo/wasm/src/)  -> wasm-pack rebuild");
console.log("  TS    (demo/src/)             -> Bun bundle");
console.log("  HTML  (demo/*.html)           -> browser reload");
console.log("  CSS   (demo/styles/)          -> browser reload");
console.log("");

// Initial TS bundle
await buildTs();

// Check whether WASM pkg exists; build if missing
const wasmPkg = join(DEMO_DIR, "public", "pkg", "tess2_wasm_bg.wasm");
if (!existsSync(wasmPkg)) {
  console.log("[wasm] No WASM package found — running initial build...");
  await buildWasm();
} else {
  console.log("[wasm] Existing WASM package found, skipping initial build");
}

startWatching();
console.log("\nReady! Waiting for changes...\n");
