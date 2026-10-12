// Copyright 2025 Lars Brubaker
// `Tessellator::render_order`: triangles in the order agg-sharp's GLU port
// (`Tesselate.Tesselator`) emits them through its callbacks with an
// edge-flag callback set, numbered by GLU client index.
//
// The sequences below are pinned on small shapes under NonZero winding (the
// rule MatterCAD's `CachedTesselator` uses).  They are cross-checked against
// recorded C# output by `tesselator_tests.rs` (agg-sharp's own GLU streams,
// with combine callbacks) and by `conformance_vs_csharp.rs` (the lion,
// corner for corner).  The default output pins guard that the render-order
// bookkeeping leaves `elements()`, `vertex_indices()` and `edge_flags()`
// exactly as they were.

use tess2_rust::{ElementType, RenderIndex, Tessellator, WindingRule};

const SQUARE: &[f64] = &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
const CONCAVE_L: &[f64] = &[0.0, 0.0, 2.0, 0.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 0.0, 2.0];
const OUTER: &[f64] = &[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
const HOLE: &[f64] = &[3.0, 3.0, 3.0, 7.0, 7.0, 7.0, 7.0, 3.0];

fn tessellate(contours: &[&[f64]]) -> Tessellator {
    let mut tess = Tessellator::new();
    for contour in contours {
        tess.add_contour(2, contour);
    }
    assert!(tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 3, 2, None));
    tess
}

/// The render order's triangles, three (client index, edge flag) corners each.
fn triangles(tess: &Tessellator) -> Vec<[(u32, bool); 3]> {
    let order = tess.render_order().expect("triangles were requested");
    order
        .indices
        .chunks_exact(3)
        .map(|c| [0, 1, 2].map(|k| (c[k].index, c[k].is_edge)))
        .collect()
}

fn input_vertices(contours: &[&[f64]]) -> Vec<[f64; 3]> {
    contours
        .iter()
        .flat_map(|c| c.chunks_exact(2).map(|p| [p[0], p[1], 0.0]))
        .collect()
}

#[test]
fn rust_only_render_order_square() {
    let tess = tessellate(&[SQUARE]);
    let order = tess.render_order().unwrap();
    assert_eq!(order.vertices, input_vertices(&[SQUARE]));
    let (f, t) = (false, true);
    assert_eq!(
        triangles(&tess),
        vec![[(1, f), (3, t), (0, t)], [(3, f), (1, t), (2, t)]]
    );
}

#[test]
fn rust_only_render_order_concave_l() {
    let tess = tessellate(&[CONCAVE_L]);
    assert_eq!(
        tess.render_order().unwrap().vertices,
        input_vertices(&[CONCAVE_L])
    );
    let (f, t) = (false, true);
    assert_eq!(
        triangles(&tess),
        vec![
            [(1, f), (5, t), (0, t)],
            [(5, f), (1, f), (3, f)],
            [(3, f), (1, t), (2, t)],
            [(4, t), (5, f), (3, t)],
        ]
    );
}

#[test]
fn rust_only_render_order_polygon_with_hole() {
    let tess = tessellate(&[OUTER, HOLE]);
    assert_eq!(
        tess.render_order().unwrap().vertices,
        input_vertices(&[OUTER, HOLE])
    );
    let (f, t) = (false, true);
    assert_eq!(
        triangles(&tess),
        vec![
            [(0, f), (4, f), (3, t)],
            [(4, f), (0, t), (1, f)],
            [(4, f), (1, f), (7, t)],
            [(7, f), (1, f), (6, t)],
            [(3, f), (5, f), (2, t)],
            [(5, f), (3, f), (4, t)],
            [(2, f), (5, t), (6, f)],
            [(2, f), (6, f), (1, t)],
        ]
    );
}

/// The render order holds the default output's triangles, last first, each
/// on the same starting corner, numbered by input index.
#[test]
fn rust_only_render_order_is_the_default_triangles_reversed() {
    for contours in [&[SQUARE][..], &[CONCAVE_L][..], &[OUTER, HOLE][..]] {
        let tess = tessellate(contours);
        let order = tess.render_order().unwrap();
        let elements = tess.elements();
        let flags = tess.edge_flags();
        let ids = tess.vertex_indices();
        let triangles = elements.len() / 3;
        assert_eq!(order.indices.len(), elements.len());
        for (k, corner) in order.indices.iter().enumerate() {
            let source = (triangles - 1 - k / 3) * 3 + k % 3;
            let expected = RenderIndex {
                index: ids[elements[source] as usize],
                is_edge: flags[source] == 1,
            };
            assert_eq!(*corner, expected, "corner {k}");
        }
    }
}

/// Combined vertices get client indices after the inputs: two squares that
/// cross make four crossings, appended in sweep order.
#[test]
fn rust_only_render_order_numbers_crossings_after_the_inputs() {
    let a: &[f64] = &[0.0, 0.0, 4.0, 0.0, 4.0, 4.0, 0.0, 4.0];
    let b: &[f64] = &[2.0, -1.0, 3.0, -1.0, 3.0, 5.0, 2.0, 5.0];
    let tess = tessellate(&[a, b]);
    let order = tess.render_order().unwrap();
    assert_eq!(order.vertices.len(), 12);
    assert_eq!(&order.vertices[..8], &input_vertices(&[a, b])[..]);
    for v in &order.vertices[8..] {
        // The crossing is interpolated, so allow rounding.
        let near = |a: f64, b: f64| (a - b).abs() < 1e-12;
        assert!(near(v[0], 2.0) || near(v[0], 3.0), "crossing at {v:?}");
        assert!(near(v[1], 0.0) || near(v[1], 4.0), "crossing at {v:?}");
    }
    // Every crossing is used, and every corner names a known vertex.
    for crossing in 8..12 {
        assert!(order.indices.iter().any(|r| r.index == crossing));
    }
    assert!(order
        .indices
        .iter()
        .all(|r| (r.index as usize) < order.vertices.len()));
}

/// Coincident input vertices are combined, not dropped: GLU's
/// `SpliceMergeVertices` appends the kept vertex again under a new index.
#[test]
fn rust_only_render_order_numbers_merged_duplicates_after_the_inputs() {
    // The closing point repeats the first.
    let closed: &[f64] = &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0];
    let tess = tessellate(&[closed]);
    let order = tess.render_order().unwrap();
    assert_eq!(order.vertices.len(), 6);
    assert_eq!(order.vertices[5], [0.0, 0.0, 0.0]);
    assert!(order.indices.iter().any(|r| r.index == 5));
    assert!(order.indices.iter().all(|r| r.index != 0 && r.index != 4));
}

/// Coincident vertices of different contours are combined when the sweep
/// reaches them (GLU `SweepEvent` loop's `SpliceMergeVertices`): the shared
/// corner comes back as client vertex 8 and the triangles use it in place of
/// both inputs.
#[test]
fn rust_only_render_order_numbers_a_shared_corner_after_the_inputs() {
    let first: &[f64] = &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let second: &[f64] = &[1.0, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0];
    let tess = tessellate(&[first, second]);
    let order = tess.render_order().unwrap();
    let mut expected = input_vertices(&[first, second]);
    expected.push([1.0, 1.0, 0.0]);
    assert_eq!(order.vertices, expected);
    let (f, t) = (false, true);
    assert_eq!(
        triangles(&tess),
        vec![
            [(5, f), (7, t), (8, t)],
            [(7, f), (5, t), (6, t)],
            [(1, f), (3, t), (0, t)],
            [(3, f), (1, t), (8, t)],
        ]
    );
}

/// A vertex the sweep finds on an edge's origin is merged by
/// `CheckForRightSplice` (GLU `SpliceMergeVertices(eLo->Oprev, eUp)`), which
/// is a combine callback even when nothing is left to fill.
#[test]
fn rust_only_render_order_records_a_right_splice_merge() {
    let degenerate: &[f64] = &[2.0, 1.0, 3.0, 3.0, 2.0, 1.0, 2.0, 3.0];
    let tess = tessellate(&[degenerate]);
    let order = tess.render_order().unwrap();
    let mut expected = input_vertices(&[degenerate]);
    expected.push([2.0, 1.0, 0.0]);
    assert_eq!(order.vertices, expected);
    assert!(order.indices.is_empty());
}

#[test]
fn rust_only_render_order_needs_a_triangle_tessellation() {
    let mut tess = Tessellator::new();
    assert!(tess.render_order().is_none(), "nothing tessellated yet");
    tess.add_contour(2, SQUARE);
    assert!(tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 4, 2, None));
    assert!(tess.render_order().is_none(), "polygons, not triangles");

    for element_type in [
        ElementType::ConnectedPolygons,
        ElementType::BoundaryContours,
    ] {
        let mut tess = Tessellator::new();
        tess.add_contour(2, SQUARE);
        assert!(tess.tessellate(WindingRule::NonZero, element_type, 3, 2, None));
        assert!(tess.render_order().is_none(), "{element_type:?}");
    }

    let mut tess = Tessellator::new();
    tess.add_contour(2, &[f64::NAN, 0.0, 1.0, 0.0, 1.0, 1.0]);
    assert!(!tess.tessellate(WindingRule::NonZero, ElementType::Polygons, 3, 2, None));
    assert!(tess.render_order().is_none(), "failed tessellation");
}

/// The default output, pinned as tess2-rust produced it before render-order
/// bookkeeping existed.
#[test]
fn rust_only_default_output_is_unchanged() {
    let square = tessellate(&[SQUARE]);
    assert_eq!(square.elements(), &[0, 1, 2, 1, 0, 3]);
    assert_eq!(square.vertex_indices(), &[3, 1, 2, 0]);
    assert_eq!(square.edge_flags(), &[0, 1, 1, 0, 1, 1]);

    let l = tessellate(&[CONCAVE_L]);
    assert_eq!(l.elements(), &[0, 1, 2, 2, 3, 4, 1, 3, 2, 3, 1, 5]);
    assert_eq!(l.vertex_indices(), &[4, 5, 3, 1, 2, 0]);
    assert_eq!(l.edge_flags(), &[1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 1]);

    let hole = tessellate(&[OUTER, HOLE]);
    assert_eq!(
        hole.elements(),
        &[0, 1, 2, 0, 3, 1, 3, 4, 5, 4, 3, 0, 6, 2, 1, 5, 2, 6, 5, 7, 2, 7, 5, 4]
    );
    assert_eq!(hole.vertex_indices(), &[2, 6, 1, 5, 3, 4, 7, 0]);
    assert_eq!(
        hole.edge_flags(),
        &[0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1]
    );
    let vertices: Vec<f64> = hole.vertices().to_vec();
    assert_eq!(
        vertices,
        vec![10.0, 10.0, 7.0, 7.0, 10.0, 0.0, 3.0, 7.0, 0.0, 10.0, 3.0, 3.0, 7.0, 3.0, 0.0, 0.0]
    );
}
