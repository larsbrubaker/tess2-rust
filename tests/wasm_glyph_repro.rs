//! Regression test for the `tessellate_path_aa` panic the agg-gui demo
//! wasm build hit on the deployed page.  Captured by the agg-gui
//! `tess2_bridge` panic hook (commit `635fbd0`):
//!
//! ```text
//! tess2 repro for tessellate_path_aa — winding=NonZero, contours=1, points=94:
//!   contour[0] (n=94) [(11.250000, 14.000638), …]
//! ```
//!
//! Symptom (release wasm32 build of `tess2-rust 1.1.3`):
//! ```text
//! panicked at tess/mod.rs:1310:23:
//! index out of bounds: the len is 636 but the index is 4294967295
//! ```
//!
//! The panic location is in `check_for_right_splice`, reading
//! `mesh.verts[e_up_org as usize]` where `e_up_org == INVALID`.  That
//! means a sweep region's `e_up` still points at an edge whose origin
//! vertex was wiped — only `mesh::delete_edge`'s `kill_vertex(_,
//! INVALID)` path can produce that — and the sweep didn't update or
//! drop the region.  The fix has to live in tess.rs / sweep.rs (the
//! invariant violation), not in `check_for_right_splice` (the
//! detector).
//!
//! Shape: a tall thin pre-stroked outline (looks like a glyph from
//! demo-ui's screenshot label, fed through agg-rust's
//! `ConvStroke`).  The vertex spread is ~2.3 × 47 logical px with
//! many sub-pixel features around y=55 and y=58, plus a handful of
//! near-collinear runs that tess2's sweep is likely to pair into the
//! same region.

use tess2_rust::{ElementType, Tessellator, WindingRule};

/// 94-vertex contour captured from a wasm-only failing
/// `tessellate_path_aa` call.  Coordinates are `f64` but are exactly
/// representable as `f32` so they round-trip through the agg-gui
/// bridge's `[f32; 2]` buffer the same way they would on wasm.
const GLYPH_CONTOUR: &[(f64, f64)] = &[
    (11.250000, 14.000638),
    (12.750000, 13.999362),
    (12.785714, 56.005501),
    (11.294978, 55.888622),
    (11.788916, 52.775249),
    (12.465559, 52.787842),
    (12.851738, 55.987007),
    (11.379949, 55.893345),
    (11.396679, 55.827061),
    (11.551047, 52.074005),
    (12.738207, 52.060570),
    (12.962763, 55.578892),
    (11.464309, 55.620796),
    (11.523443, 48.059597),
    (12.980536, 48.060562),
    (13.035699, 56.554405),
    (11.535856, 56.544666),
    (11.629756, 51.725201),
    (13.021822, 51.727222),
    (13.105529, 56.777279),
    (13.117938, 56.824211),
    (11.653900, 56.887707),
    (12.189571, 53.800686),
    (12.863833, 53.822754),
    (13.210046, 57.064934),
    (11.760931, 56.884197),
    (12.422991, 55.095737),
    (13.216375, 56.810310),
    (11.806660, 56.949261),
    (12.524994, 53.973866),
    (12.828382, 53.984516),
    (13.346745, 57.065144),
    (11.890469, 56.968506),
    (12.600926, 54.665466),
    (13.353376, 56.833614),
    (13.330414, 56.800533),
    (13.070786, 56.568329),
    (13.106501, 56.587624),
    (12.311625, 56.638931),
    (12.286334, 56.657150),
    (12.441468, 56.489521),
    (13.165848, 56.359070),
    (13.480438, 56.755653),
    (12.638704, 56.516125),
    (12.863604, 56.435123),
    (13.215260, 56.498451),
    (13.356501, 56.574760),
    (12.643499, 57.894470),
    (12.607785, 57.875172),
    (12.831356, 57.953445),
    (12.795641, 57.947014),
    (13.182724, 57.914513),
    (12.640109, 58.109951),
    (12.269562, 57.642826),
    (12.990073, 57.914856),
    (12.954359, 57.921288),
    (13.371877, 57.692577),
    (13.285094, 57.786350),
    (12.807014, 58.130741),
    (12.200253, 57.802929),
    (12.003767, 57.519844),
    (11.934314, 57.319717),
    (13.359531, 57.294903),
    (12.467383, 60.186916),
    (12.349630, 60.179104),
    (11.831826, 57.101791),
    (13.300483, 57.153355),
    (12.759393, 59.394592),
    (11.819340, 57.363056),
    (13.203354, 57.308464),
    (12.158965, 60.129734),
    (12.027032, 60.113277),
    (11.682811, 56.889751),
    (13.167529, 56.938343),
    (12.634826, 60.008259),
    (12.410872, 60.017971),
    (11.608757, 56.984451),
    (11.571531, 54.738640),
    (13.071286, 54.740818),
    (12.977318, 59.563828),
    (11.555151, 59.554596),
    (11.500016, 51.064949),
    (12.999977, 51.065945),
    (12.940996, 58.607513),
    (11.655307, 58.643467),
    (11.430095, 55.114868),
    (12.927938, 55.097919),
    (12.889035, 56.043720),
    (12.121703, 59.083885),
    (11.711528, 59.057781),
    (11.326834, 55.870907),
    (12.812164, 55.898544),
    (12.314219, 59.037182),
    (11.288222, 58.956741),
];

fn flatten(contour: &[(f64, f64)]) -> Vec<f64> {
    let mut flat = Vec::with_capacity(contour.len() * 2);
    for &(x, y) in contour {
        flat.push(x);
        flat.push(y);
    }
    flat
}

/// Reproduces the exact path that panicked on the deployed wasm
/// build: one contour, NonZero winding, Polygons output, 3-vertex
/// elements, 2-D vertices.
#[test]
fn wasm_glyph_must_not_panic_nonzero() {
    let flat = flatten(GLYPH_CONTOUR);
    let mut tess = Tessellator::new();
    tess.add_contour(2, &flat);
    let _ok = tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 3, 2, None);
    // Either succeeds (great, bug fixed) or returns false gracefully.
    // What it MUST NOT do is panic — the whole point of the
    // regression is to drive `check_for_right_splice` (and the
    // upstream sweep-region update path) over the input that
    // currently produces an `INVALID` `e_up_org`.
}

/// Same input, every winding rule, to confirm the fix isn't
/// rule-specific.  Even-odd (Odd) is what `tessellate_fill` uses by
/// default, so a future call site that swaps modes shouldn't
/// re-introduce the panic.
#[test]
fn wasm_glyph_must_not_panic_all_rules() {
    let flat = flatten(GLYPH_CONTOUR);
    for rule in [
        WindingRule::Odd,
        WindingRule::NonZero,
        WindingRule::Positive,
        WindingRule::Negative,
        WindingRule::AbsGeqTwo,
    ] {
        let mut tess = Tessellator::new();
        tess.add_contour(2, &flat);
        let _ok = tess.tessellate(rule, ElementType::Polygons, 3, 2, None);
    }
}
