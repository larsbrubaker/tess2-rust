//! Brute-force fuzzer that hammers the tessellator with shapes that
//! plausibly mimic real GUI rendering input.  Discovered the crash
//! pattern that the agg-gui demo's deployed wasm build hit on every
//! mouse-move (see `tess2-rust 1.1.1/src/mesh/mod.rs:446: index out
//! of bounds`).
//!
//! The contract we're holding the tessellator to is the same one the
//! original SGI / libtess2 promised: any sequence of contours, with
//! any winding rule, must either tessellate to a valid triangle set
//! or report failure — never panic.  These tests intentionally feed
//! it the kinds of degenerate / self-intersecting shapes the real
//! GUI renderer hands it: many overlapping rectangles (window
//! stacking), text-glyph-shaped paths with holes, thin slivers,
//! near-duplicate vertices, etc.
//!
//! Each shape is run under all six winding rules so we don't miss
//! a branch that only one rule exercises.

use std::panic::{catch_unwind, AssertUnwindSafe};

use tess2_rust::{ElementType, Tessellator, WindingRule};

const RULES: &[WindingRule] = &[
    WindingRule::Odd,
    WindingRule::NonZero,
    WindingRule::Positive,
    WindingRule::Negative,
    WindingRule::AbsGeqTwo,
];

fn try_tessellate(contours: &[Vec<f64>], rule: WindingRule) -> Result<bool, String> {
    let mut tess = Tessellator::new();
    for c in contours {
        tess.add_contour(2, c);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        tess.tessellate(rule, ElementType::Polygons, 3, 2, None)
    }));
    match result {
        Ok(ok) => Ok(ok),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "<unknown panic payload>".into());
            Err(msg)
        }
    }
}

fn assert_no_panic(name: &str, contours: &[Vec<f64>]) {
    for &rule in RULES {
        let result = try_tessellate(contours, rule);
        match result {
            Ok(_) => {}
            Err(msg) => panic!(
                "tessellator panicked for shape `{}` with winding rule {:?}: {}",
                name, rule, msg
            ),
        }
    }
}

/// Stack of overlapping rectangles, the way a GUI builds a window
/// chrome (background + frame + title-bar + button row).  The
/// contours intersect each other heavily.
#[test]
fn overlapping_rectangles_window_chrome_must_not_panic() {
    let r = |x: f64, y: f64, w: f64, h: f64| -> Vec<f64> {
        vec![x, y, x + w, y, x + w, y + h, x, y + h]
    };
    let contours = vec![
        r(0.0, 0.0, 200.0, 150.0),  // background
        r(2.0, 2.0, 196.0, 24.0),   // title bar
        r(2.0, 26.0, 196.0, 122.0), // body
        r(140.0, 4.0, 18.0, 18.0),  // close button
        r(120.0, 4.0, 18.0, 18.0),  // min button
        r(8.0, 32.0, 60.0, 110.0),  // sidebar
        r(72.0, 32.0, 124.0, 110.0),
    ];
    assert_no_panic("overlapping window chrome", &contours);
}

/// "Letter O" — a square with a square hole.  Two CCW contours, one
/// inside the other but with opposite winding (outer CCW, inner CW)
/// per the even-odd rule.
#[test]
fn ring_with_inner_hole_must_not_panic() {
    let outer = vec![0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0];
    let inner = vec![30.0, 30.0, 30.0, 70.0, 70.0, 70.0, 70.0, 30.0];
    assert_no_panic("ring with hole", &[outer, inner]);
}

/// Many concentric rings, the kind a UI sometimes paints when
/// drawing a focus halo or a stacked bullseye / target.
#[test]
fn many_concentric_rings_must_not_panic() {
    let mut contours = Vec::new();
    for i in 0..8 {
        let r = (i as f64) * 5.0 + 5.0;
        let mut ring = Vec::new();
        let n = 24;
        for k in 0..n {
            let theta = (k as f64) / (n as f64) * std::f64::consts::TAU;
            ring.push(50.0 + r * theta.cos());
            ring.push(50.0 + r * theta.sin());
        }
        contours.push(ring);
    }
    assert_no_panic("concentric rings", &contours);
}

/// Self-intersecting bowtie / figure-8 / pentagram — every
/// non-trivial winding rule has to choose a different fill region.
#[test]
fn self_intersecting_pentagram_must_not_panic() {
    // Five-pointed star drawn with a single self-intersecting contour.
    let mut star = Vec::new();
    let n = 5;
    let r = 50.0;
    for k in 0..n {
        let theta = (k as f64) * 2.0 * std::f64::consts::TAU / (n as f64) - std::f64::consts::FRAC_PI_2;
        star.push(50.0 + r * theta.cos());
        star.push(50.0 + r * theta.sin());
    }
    assert_no_panic("self-intersecting pentagram", &[star]);
}

/// Sliver triangle: very thin, near-collinear vertices.  Numerical
/// edge cases that would otherwise hit precision-driven branches.
#[test]
fn near_collinear_sliver_must_not_panic() {
    let contour = vec![
        0.0, 0.0, // a
        100.0, 0.0, // b
        100.0, 0.000_000_1, // c — essentially on the AB line
        50.0, 0.000_000_05,
    ];
    assert_no_panic("near-collinear sliver", &[contour]);
}

/// Many copies of the same point with tiny jitter — simulates the
/// kind of noise that comes out of curve flattening on a high-
/// resolution Bézier.
#[test]
fn jittered_quasi_duplicates_must_not_panic() {
    let mut c = Vec::new();
    let base = [50.0, 50.0];
    let n = 64;
    for k in 0..n {
        let theta = (k as f64) / (n as f64) * std::f64::consts::TAU;
        let r = 30.0 + 1e-9 * (k as f64).sin();
        c.push(base[0] + r * theta.cos());
        c.push(base[1] + r * theta.sin());
    }
    assert_no_panic("jittered quasi-duplicates", &[c]);
}

/// Random polygon ensemble — pseudo-random but seeded, so any
/// regression is reproducible.  This is the catch-all that's most
/// likely to find new panics.
#[test]
fn pseudo_random_polygon_swarm_must_not_panic() {
    // Tiny LCG so we don't pull in `rand` as a dev-dep.
    struct Lcg(u64);
    impl Lcg {
        fn next_f64(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }
    for seed in 0u64..32 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let n_contours = 1 + (rng.next_f64() * 5.0) as usize;
        let mut contours = Vec::new();
        for _ in 0..n_contours {
            let n = 3 + (rng.next_f64() * 12.0) as usize;
            let cx = rng.next_f64() * 100.0;
            let cy = rng.next_f64() * 100.0;
            let r = 5.0 + rng.next_f64() * 40.0;
            let mut c = Vec::new();
            for k in 0..n {
                let theta = (k as f64) / (n as f64) * std::f64::consts::TAU
                    + rng.next_f64() * 0.2;
                let rk = r * (0.5 + rng.next_f64());
                c.push(cx + rk * theta.cos());
                c.push(cy + rk * theta.sin());
            }
            contours.push(c);
        }
        assert_no_panic(&format!("random seed {seed}"), &contours);
    }
}
