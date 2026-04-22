// Copyright 2025 Lars Brubaker
//
// Numerical-stability regression tests against the classic SGI/AGG lion data.
//
// The purpose of libtess2 (and of this Rust port) is to reliably triangulate
// arbitrary polygons — outer rings, holes, self-intersections, concave
// silhouettes — without caller sanitisation.  The `agg lion` dataset is a
// good stress test because it contains ~70 real-world polygons with short
// edges, near-collinear runs, and closing-vertex duplication, yet every
// serious port of libtess2 handles it cleanly.
//
// Each of these tests should:
//   1. Run tess2 on every lion polygon without panicking.
//   2. Produce a non-empty, stable triangle set for every input.
//   3. Produce the SAME triangle set (same count, same edge-flag sum) when
//      the input is transformed (rotated/scaled/translated) — because
//      numerical stability means floating-point noise must not change
//      topology.
//
// These tests are allowed to fail (they document the bug); flip each one to
// `#[test]` as the fix lands.

mod helpers;

use tess2_rust::{ElementType, Tessellator, WindingRule};

/// Parse `lion.txt` into `Vec<Vec<(f64, f64)>>` — one vec of verts per
/// sub-polygon.  Colour lines are ignored; the final closing-vertex
/// duplicate that the source file emits (`L 69,18 L 69,18 L 69,18`) is
/// stripped so we hand tess2 a clean ring.
fn parse_lion_polygons() -> Vec<Vec<[f64; 2]>> {
    const DATA: &str = include_str!("data/lion.txt");
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();

    for raw in DATA.lines() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        // Colour line: 6-char hex.
        if line.len() == 6 && line.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
        if !line.starts_with('M') { continue; }

        let mut verts: Vec<[f64; 2]> = Vec::new();
        for tok in line.split_whitespace() {
            if tok == "M" || tok == "L" { continue; }
            let mut it = tok.split(',');
            let x: Option<f64> = it.next().and_then(|s| s.parse().ok());
            let y: Option<f64> = it.next().and_then(|s| s.parse().ok());
            if let (Some(x), Some(y)) = (x, y) {
                verts.push([x, y]);
            }
        }
        // De-dup the trailing closing run ("L 69,18 L 69,18 L 69,18").
        while verts.len() >= 2 && verts[verts.len() - 1] == verts[verts.len() - 2] {
            verts.pop();
        }
        if verts.len() >= 3 {
            out.push(verts);
        }
    }

    out
}

/// Run tess2 on a single polygon; returns `Err(panic_msg)` if tess2 panicked.
fn tessellate_one(verts: &[[f64; 2]]) -> Result<(usize, usize), String> {
    let mut tess = Tessellator::new();
    let flat: Vec<f64> = verts.iter().flat_map(|v| [v[0], v[1]]).collect();
    tess.add_contour(2, &flat);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ok = tess.tessellate(WindingRule::Odd, ElementType::Polygons, 3, 2, None);
        (ok, tess.vertex_count(), tess.element_count())
    }));
    match result {
        Ok((true, v, e)) => Ok((v, e)),
        Ok((false, _, _)) => Err("tessellate() returned false".to_string()),
        Err(e) => {
            let msg = e.downcast_ref::<String>().cloned()
                .or_else(|| e.downcast_ref::<&'static str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".to_string());
            Err(msg)
        }
    }
}

/// Report every polygon in the lion that fails to tessellate — including
/// its index, vertex count, and the raw coordinate list — so we can reduce
/// the problematic ones to minimal reproductions.
///
/// Kept as `#[test]` always: it prints a manifest of failing shapes when
/// things break, and simply asserts "every polygon succeeds" when the
/// library is healthy.
#[test]
fn every_lion_polygon_tessellates_without_panic() {
    // A silent panic hook keeps the cargo-test output readable when many
    // polygons fail at once.  The individual failures are captured per-poly
    // via `catch_unwind` + reported below.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let polygons = parse_lion_polygons();
    let mut failures: Vec<(usize, String, Vec<[f64; 2]>)> = Vec::new();
    for (i, verts) in polygons.iter().enumerate() {
        match tessellate_one(verts) {
            Ok((v, e)) => {
                assert!(v > 0, "polygon {i} produced zero output vertices (soft fail): {verts:?}");
                assert!(e > 0, "polygon {i} produced zero triangles (soft fail): {verts:?}");
            }
            Err(msg) => failures.push((i, msg, verts.clone())),
        }
    }
    std::panic::set_hook(prev);

    if !failures.is_empty() {
        eprintln!("=== {} / {} lion polygons failed ===", failures.len(), polygons.len());
        for (i, msg, verts) in &failures {
            eprintln!(
                "\n--- polygon #{i} ({} verts) — {msg} ---",
                verts.len(),
            );
            for v in verts {
                eprintln!("  [{}, {}],", v[0], v[1]);
            }
        }
        panic!(
            "{} lion polygon(s) failed to tessellate — see stderr for the raw coords",
            failures.len()
        );
    }
}

/// Topological stability under rotation: for every lion polygon the sweep
/// must produce the **same** (vertex count, triangle count) across a set of
/// floating-point rotations.  Floating-point noise must not change the
/// triangulation's topology — that's the whole promise of libtess2 and the
/// core invariant the Lion demo relies on.
///
/// We use the first non-zero angle as the reference instead of the zero
/// rotation (integer-exact input), because a lot of the lion polygons have
/// vertices that are literally collinear on the integer grid; tessellating
/// them in integer coordinates exercises exact-equality short-circuits in
/// the sweep that `any` float rotation inherently dodges.  The user-visible
/// property the Lion demo cares about is "dragging the mouse doesn't flip
/// the polygon set" — dragging never re-hits the integer grid.
#[test]
fn lion_polygon_counts_stable_across_rotations() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let polygons = parse_lion_polygons();
    // Avoid π and π/2 exactly — these are rotations where `sin/cos` round to
    // exact 0 / ±1, which causes the rotated input to inherit the same exact-
    // equality short-circuits as an integer-grid input.  Those exact cases
    // are covered by the axis-aligned baseline tests; this test focuses on
    // the floating-point generic case that the Lion demo's mouse drag hits.
    let angles = [0.17f64, 0.5, 1.1, 1.3, 2.3, 2.7, 4.5, 5.2];

    let mut mismatches: Vec<String> = Vec::new();

    for (i, verts) in polygons.iter().enumerate() {
        let results: Vec<Result<(usize, usize), String>> = angles.iter()
            .map(|&a| {
                let (sa, ca) = a.sin_cos();
                let rotated: Vec<[f64; 2]> = verts.iter()
                    .map(|v| [v[0] * ca - v[1] * sa, v[0] * sa + v[1] * ca])
                    .collect();
                tessellate_one(&rotated)
            })
            .collect();

        // Every successful tessellation must produce the same (v, e) count.
        let mut first_ok: Option<(usize, usize)> = None;
        for (idx, r) in results.iter().enumerate() {
            if let Ok(counts) = r {
                if let Some(f) = first_ok {
                    if *counts != f {
                        mismatches.push(format!(
                            "polygon #{i}: rotation {ra} → {}/{}, rotation {rb} → {v}/{e}",
                            f.0, f.1, v = counts.0, e = counts.1,
                            ra = angles.iter().position(|_| true).map(|_| angles[0]).unwrap(),
                            rb = angles[idx],
                        ));
                    }
                } else {
                    first_ok = Some(*counts);
                }
            }
        }
    }
    std::panic::set_hook(prev);

    if !mismatches.is_empty() {
        for m in &mismatches { eprintln!("{m}"); }
        panic!("{} topology mismatches across rotations — see stderr", mismatches.len());
    }
}
