//! Lion-rotation stress test — repro for the panic the agg-gui demo's
//! deployed wasm build hit on every mouse move.
//!
//! The agg-gui `LionView` widget runs `tess2-rust` fresh every frame
//! on the classic AGG lion (~130 self-intersecting coloured paths)
//! while the user rotates / scales / skews the figure with the mouse.
//! At certain transforms one of those paths reaches `kill_face` with
//! `INVALID` (== `u32::MAX`) and panics with
//!
//!     index out of bounds: the len is 172 but the index is 4294967295
//!
//! That's a porting bug — libtess2's C original never reaches
//! `KillFace(NULL, _)` for any input shape, so neither should we.
//! The contract is: any sequence of contours, any winding rule, must
//! either tessellate to a valid triangle list or report failure via
//! `tessellate() -> false` — never panic.
//!
//! This test sweeps angle / scale / skew along a deterministic grid
//! that mirrors the demo's interactive transform space.  When it
//! catches a panic, it prints the offending sub-path index, the
//! transform, and the contour itself so the regression case can be
//! pinned down without any wasm round-trip.

use std::panic::{catch_unwind, AssertUnwindSafe};

use tess2_rust::{ElementType, Tessellator, WindingRule};

const LION: &str = include_str!("lion_data/lion.txt");

#[derive(Debug, Clone)]
struct LionPath {
    /// Raw lion coords in the file's Y-down system.  The widget
    /// applies an additional mirror; we don't need to here because
    /// tessellation is rotation-invariant — only the *transform*
    /// matters for triggering the bug.
    verts: Vec<[f64; 2]>,
}

fn parse_lion() -> (Vec<LionPath>, (f64, f64, f64, f64)) {
    let mut out: Vec<LionPath> = Vec::new();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for line in LION.lines() {
        let line = line.trim();
        if !line.starts_with('M') {
            continue;
        }
        let mut verts: Vec<[f64; 2]> = Vec::new();
        for tok in line.split_whitespace() {
            if tok == "M" || tok == "L" {
                continue;
            }
            let mut it = tok.split(',');
            if let (Some(xs), Some(ys)) = (it.next(), it.next()) {
                if let (Ok(x), Ok(y)) = (xs.parse::<f64>(), ys.parse::<f64>()) {
                    verts.push([x, y]);
                    if x < min_x {
                        min_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }
        if verts.len() >= 3 {
            out.push(LionPath { verts });
        }
    }
    (out, (min_x, min_y, max_x, max_y))
}

fn try_tessellate(contour: &[f64], rule: WindingRule) -> Result<bool, String> {
    let mut tess = Tessellator::new();
    tess.add_contour(2, contour);
    let r = catch_unwind(AssertUnwindSafe(|| {
        tess.tessellate(rule, ElementType::Polygons, 3, 2, None)
    }));
    match r {
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

/// Sweeps the lion through the transform space the agg-gui demo
/// exposes and asserts no contour ever panics the tessellator.
///
/// If a transform crashes, the test panics with full repro info:
/// path index, angle / scale / skew, winding rule, raw contour.
#[test]
fn lion_rotation_must_never_panic() {
    let (paths, (min_x, min_y, max_x, max_y)) = parse_lion();
    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;

    // Match the LionView interactive transform shape:
    //   r = rotate(angle), s = scale(mouse_scale), k = skew(skew_x, skew_y)
    //   p' = (k ∘ r ∘ s)(p - centre) + centre
    let angles: Vec<f64> = (0..36).map(|i| (i as f64) * std::f64::consts::TAU / 36.0).collect();
    let scales: &[f64] = &[0.25, 0.5, 1.0, 2.5, 7.0];
    let skews: &[(f64, f64)] = &[(0.0, 0.0), (0.05, 0.0), (0.0, 0.05), (-0.1, 0.1), (0.2, -0.2)];

    let rules = [
        WindingRule::Odd,
        WindingRule::NonZero,
        WindingRule::Positive,
        WindingRule::Negative,
        WindingRule::AbsGeqTwo,
    ];

    let mut crashes: Vec<String> = Vec::new();

    for &angle in &angles {
        let (sin_a, cos_a) = angle.sin_cos();
        for &scale in scales {
            for &(skew_x, skew_y) in skews {
                for (path_idx, path) in paths.iter().enumerate() {
                    let mut flat = Vec::with_capacity(path.verts.len() * 2);
                    for &[x, y] in &path.verts {
                        let px = (x - cx) * scale;
                        let py = (y - cy) * scale;
                        let rx = px * cos_a - py * sin_a;
                        let ry = px * sin_a + py * cos_a;
                        let sx = rx + ry * skew_x;
                        let sy = ry + rx * skew_y;
                        flat.push(sx);
                        flat.push(sy);
                    }
                    for &rule in &rules {
                        if let Err(msg) = try_tessellate(&flat, rule) {
                            crashes.push(format!(
                                "path #{path_idx} angle={angle:.4} scale={scale} \
                                 skew=({skew_x},{skew_y}) rule={rule:?}: {msg}\n  contour: {:?}",
                                path.verts
                            ));
                            // One repro per (path, rule) is enough — keep
                            // sweeping so we see every crash class.
                        }
                    }
                }
            }
        }
    }

    if !crashes.is_empty() {
        panic!(
            "lion rotation sweep produced {} tessellator panics:\n{}",
            crashes.len(),
            crashes.join("\n---\n")
        );
    }
}
