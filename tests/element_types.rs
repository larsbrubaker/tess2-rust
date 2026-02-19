// Copyright 2025 Lars Brubaker
// Tests for ConnectedPolygons and BoundaryContours element types.

mod helpers;

use tess2_rust::{ElementType, TessOption, Tessellator, WindingRule};

fn unit_quad() -> Vec<f32> {
    vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]
}

fn polygon_with_hole() -> (Vec<f32>, Vec<f32>) {
    let outer = vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let inner = vec![3.0, 3.0, 7.0, 3.0, 7.0, 7.0, 3.0, 7.0];
    (outer, inner)
}

// --- ConnectedPolygons tests ---

#[test]
fn connected_polygons_quad_produces_neighbor_info() {
    let mut tess = Tessellator::new();
    tess.add_contour(2, &unit_quad());
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::ConnectedPolygons,
        3,
        2,
        None,
    );
    assert!(ok, "ConnectedPolygons tessellation should succeed");
    assert!(
        tess.element_count() > 0,
        "should produce at least one element"
    );

    // ConnectedPolygons: elements array has 2*poly_size entries per element.
    // First poly_size are vertex indices, next poly_size are neighbor element indices.
    let elems = tess.elements();
    let poly_size = 3;
    let expected_len = tess.element_count() * poly_size * 2;
    assert_eq!(
        elems.len(),
        expected_len,
        "elements should have 2*poly_size entries per element (got {} expected {})",
        elems.len(),
        expected_len
    );

    for i in 0..tess.element_count() {
        let base = i * poly_size * 2;
        for j in 0..poly_size {
            let idx = elems[base + j];
            if idx != u32::MAX {
                assert!(
                    (idx as usize) < tess.vertex_count(),
                    "vertex index {} out of range",
                    idx
                );
            }
        }
    }
}

#[test]
fn connected_polygons_quad_neighbors_reference_valid_elements() {
    let mut tess = Tessellator::new();
    tess.add_contour(2, &unit_quad());
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::ConnectedPolygons,
        3,
        2,
        None,
    );
    assert!(ok);

    let elems = tess.elements();
    let poly_size = 3;
    for i in 0..tess.element_count() {
        let base = i * poly_size * 2;
        for j in 0..poly_size {
            let neighbor = elems[base + poly_size + j];
            if neighbor != u32::MAX {
                assert!(
                    (neighbor as usize) < tess.element_count(),
                    "neighbor index {} out of range (element_count={})",
                    neighbor,
                    tess.element_count()
                );
            }
        }
    }
}

#[test]
fn connected_polygons_polygon_with_hole() {
    let (outer, inner) = polygon_with_hole();
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &outer);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &inner);

    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::ConnectedPolygons,
        3,
        2,
        None,
    );
    assert!(ok);
    assert!(
        tess.element_count() >= 8,
        "polygon with hole needs at least 8 triangles"
    );
}

// --- BoundaryContours tests ---

#[test]
fn boundary_contours_simple_quad() {
    let mut tess = Tessellator::new();
    tess.add_contour(2, &unit_quad());
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::BoundaryContours,
        0,
        2,
        None,
    );
    assert!(ok, "BoundaryContours tessellation should succeed");
    assert!(
        tess.element_count() > 0,
        "should produce at least one boundary contour"
    );

    // BoundaryContours: elements are [start_vertex_index, vertex_count] pairs
    let elems = tess.elements();
    assert_eq!(
        elems.len(),
        tess.element_count() * 2,
        "elements should have 2 entries per contour"
    );

    let verts = tess.vertices();
    for i in 0..tess.element_count() {
        let start = elems[i * 2] as usize;
        let count = elems[i * 2 + 1] as usize;
        assert!(
            count >= 3,
            "boundary contour should have at least 3 vertices"
        );
        assert!(
            start + count <= tess.vertex_count(),
            "contour vertices out of range: start={} count={} vertex_count={}",
            start,
            count,
            tess.vertex_count()
        );
        for j in start..(start + count) {
            let x = verts[j * 2];
            let y = verts[j * 2 + 1];
            assert!(
                x.is_finite() && y.is_finite(),
                "vertex ({}, {}) not finite",
                x,
                y
            );
        }
    }
}

#[test]
fn boundary_contours_polygon_with_hole_produces_two_contours() {
    let (outer, inner) = polygon_with_hole();
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &outer);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &inner);

    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::BoundaryContours,
        0,
        2,
        None,
    );
    assert!(ok);
    assert!(
        tess.element_count() >= 2,
        "polygon with hole should produce at least 2 boundary contours, got {}",
        tess.element_count()
    );
}

#[test]
fn boundary_contours_can_be_re_tessellated() {
    // The "combine then triangulate" pattern from libtess2 example.c:
    // 1) Tessellate to BoundaryContours
    // 2) Feed those contours back in
    // 3) Tessellate to Polygons
    let (outer, inner) = polygon_with_hole();
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &outer);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &inner);

    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::BoundaryContours,
        0,
        2,
        None,
    );
    assert!(ok);

    let boundary_verts = tess.vertices().to_vec();
    let boundary_elems = tess.elements().to_vec();
    let boundary_count = tess.element_count();

    let mut tess2 = Tessellator::new();
    for i in 0..boundary_count {
        let start = boundary_elems[i * 2] as usize;
        let count = boundary_elems[i * 2 + 1] as usize;
        let contour: Vec<f32> = (start..start + count)
            .flat_map(|j| vec![boundary_verts[j * 2], boundary_verts[j * 2 + 1]])
            .collect();
        tess2.add_contour(2, &contour);
    }

    let ok2 = tess2.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok2, "re-tessellation should succeed");
    assert!(
        tess2.element_count() > 0,
        "re-tessellation should produce triangles"
    );
    helpers::verify_valid_output(&tess2);
}

#[test]
fn boundary_contours_nested_squares() {
    let mut tess = Tessellator::new();
    tess.add_contour(2, &[-5.0f32, -5.0, 5.0, -5.0, 5.0, 5.0, -5.0, 5.0]);
    tess.add_contour(2, &[-2.0f32, -2.0, -2.0, 2.0, 2.0, 2.0, 2.0, -2.0]);

    let ok = tess.tessellate(WindingRule::Odd, ElementType::BoundaryContours, 0, 2, None);
    assert!(ok);
    assert!(
        tess.element_count() >= 2,
        "nested squares with Odd should produce >= 2 contours"
    );
}
