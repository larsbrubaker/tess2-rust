//! Discovery fuzzer aimed at the agg-gui glyph-tessellation crash family
//! (`tessellate_path_aa_texture`).  The real crashers are *stroked font
//! outlines*: tall and thin, coordinates clustered in a narrow x band over a
//! taller y band, with many near-coincident y values (so the sweep gets piles
//! of simultaneous events) and heavy self-overlap from the inner/outer stroke
//! edges.  Generic convex-blob fuzzing never reproduced them.
//!
//! This harness mimics that statistical profile, runs each shape through the
//! real `tessellate`, and groups any panics by source location so every
//! *distinct* sweep bug surfaces in one run instead of one crash per glyph.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use tess2_rust::{ElementType, Tessellator, WindingRule};

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1))
    }
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A stroked-glyph-like contour: `n` points whose x sits in a narrow band and
/// whose y is snapped to a coarse grid so many events share a sweep line — the
/// degenerate situation real glyph outlines create.
fn glyph_like(seed: u64, n: usize) -> Vec<f64> {
    let mut rng = Lcg::new(seed);
    let x0 = 11.0;
    let x_band = 2.0 + rng.next_f64() * 1.5; // ~2–3.5 wide (stroke width-ish)
    let y0 = 11.0;
    let y_span = 18.0 + rng.next_f64() * 12.0; // tall
    let grid = 1.0 / (2.0 + (rng.next_f64() * 4.0).floor()); // 1/2 .. 1/5 unit
    let mut c = Vec::with_capacity(n * 2);
    for k in 0..n {
        // Alternate the inner/outer stroke edge, with jitter.
        let side = (k % 2) as f64;
        let x = x0 + side * x_band + (rng.next_f64() - 0.5) * 0.6;
        // Snap y to a coarse grid → many coincident / near-coincident events.
        let raw = y0 + rng.next_f64() * y_span;
        let y = (raw / grid).round() * grid;
        c.push(x);
        c.push(y);
    }
    c
}

/// Tessellate a single seed's contour directly (no `catch_unwind`) so panics
/// propagate with their backtrace.  `GLYPH_FUZZ_SEED=<n> cargo test repro_one_seed -- --nocapture`.
#[test]
fn repro_one_seed() {
    let seed: u64 = match std::env::var("GLYPH_FUZZ_SEED").ok().and_then(|s| s.parse().ok()) {
        Some(s) => s,
        None => return, // no-op unless a seed is requested
    };
    let n = 60 + (seed as usize * 7) % 120;
    let contour = glyph_like(seed, n);
    eprintln!("repro seed={seed} n={n}");
    // Dump the contour bit-exactly (f64 bit patterns) so the C reference build
    // can tessellate the identical input — `GLYPH_FUZZ_DUMP=<path>`.
    if let Ok(path) = std::env::var("GLYPH_FUZZ_DUMP") {
        use std::fmt::Write as _;
        let mut s = format!("{}\n", contour.len() / 2);
        for v in &contour {
            let _ = writeln!(s, "{}", v.to_bits());
        }
        std::fs::write(&path, s).unwrap();
        eprintln!("dumped {n} points to {path}");
    }
    let mut tess = Tessellator::new();
    tess.add_contour(2, &contour);
    let ok = tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 3, 2, None);
    eprintln!("ok={ok} verts={} elems={}", tess.vertices().len(), tess.elements().len());
}

#[test]
fn discover_glyph_sweep_panics() {
    use std::io::Write;
    // Capture the panic *location* (the default payload for index-OOB doesn't
    // include it) so we can group distinct bugs.
    let loc: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let loc_hook = loc.clone();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(l) = info.location() {
            *loc_hook.lock().unwrap() = format!("{}:{}", l.file(), l.line());
        }
    }));

    // location -> (seed, n, message)
    let mut distinct: BTreeMap<String, (u64, usize, String)> = BTreeMap::new();
    let seeds: u64 = std::env::var("GLYPH_FUZZ_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(400);

    for seed in 0u64..seeds {
        if seed % 25 == 0 {
            eprint!("\rseed {seed}/{seeds} (distinct={})   ", distinct.len());
            let _ = std::io::stderr().flush();
        }
        let n = 60 + (seed as usize * 7) % 120; // 60..180 points
        let contour = glyph_like(seed, n);
        // NonZero is the rule agg-gui's glyph tessellation uses.
        loc.lock().unwrap().clear();
        let mut tess = Tessellator::new();
        tess.add_contour(2, &contour);
        let res = catch_unwind(AssertUnwindSafe(|| {
            tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 3, 2, None)
        }));
        if let Err(payload) = res {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "<unknown>".into());
            let where_ = loc.lock().unwrap().clone();
            distinct.entry(where_).or_insert((seed, n, msg));
        }
    }

    let _ = std::panic::take_hook();
    eprintln!("\n=== glyph fuzz: {seeds} seeds, {} distinct panic sites ===", distinct.len());
    for (where_, (seed, n, msg)) in &distinct {
        eprintln!("  {where_}  (seed={seed} n={n}): {msg}");
    }
    assert!(distinct.is_empty(), "{} distinct glyph-sweep panic sites", distinct.len());
}
