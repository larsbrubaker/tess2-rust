// Copyright 2025 Lars Brubaker
// Tests verifying output correctness: area preservation, valid indices, no degenerates.

mod helpers;

use tess2_rust::{ElementType, TessOption, Tessellator, WindingRule};

fn tessellate_simple(vertices: &[f32]) -> Tessellator {
    let mut tess = Tessellator::new();
    tess.add_contour(2, vertices);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok, "tessellation failed");
    tess
}

// --- Area preservation tests ---

#[test]
fn area_unit_square() {
    let tess = tessellate_simple(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    let area = helpers::total_tessellation_area(&tess);
    assert!(
        (area - 1.0).abs() < 0.001,
        "unit square area should be 1.0, got {}",
        area
    );
}

#[test]
fn area_unit_triangle() {
    let tess = tessellate_simple(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    let area = helpers::total_tessellation_area(&tess);
    assert!(
        (area - 0.5).abs() < 0.001,
        "unit triangle area should be 0.5, got {}",
        area
    );
}

#[test]
fn area_rectangle_2x3() {
    let tess = tessellate_simple(&[0.0, 0.0, 2.0, 0.0, 2.0, 3.0, 0.0, 3.0]);
    let area = helpers::total_tessellation_area(&tess);
    assert!(
        (area - 6.0).abs() < 0.01,
        "2x3 rectangle area should be 6.0, got {}",
        area
    );
}

#[test]
fn area_polygon_with_hole() {
    // Use the same pattern as the C++ test: 3x3 outer, 1x1 inner hole
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &[0.0f32, 0.0, 3.0, 0.0, 3.0, 3.0, 0.0, 3.0]);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &[1.0f32, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0]);

    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok);
    assert_eq!(tess.element_count(), 8, "should produce 8 triangles");
    helpers::verify_valid_output(&tess);

    let area = helpers::total_tessellation_area(&tess);
    assert!(
        area > 0.0,
        "polygon with hole should have non-zero area, got {}",
        area
    );
}

#[test]
fn area_regular_hexagon() {
    use std::f32::consts::PI;
    let mut hex = Vec::new();
    for i in 0..6 {
        let angle = PI / 3.0 * i as f32;
        hex.push(angle.cos());
        hex.push(angle.sin());
    }
    let tess = tessellate_simple(&hex);
    let area = helpers::total_tessellation_area(&tess);
    // Regular hexagon with circumradius 1 has area = 3*sqrt(3)/2 ≈ 2.598
    let expected = 3.0 * 3.0f32.sqrt() / 2.0;
    assert!(
        (area - expected).abs() < 0.01,
        "hexagon area should be ~{}, got {}",
        expected,
        area
    );
}

// --- Valid indices tests ---

#[test]
fn valid_indices_quad() {
    let tess = tessellate_simple(&[0.0, 0.0, 5.0, 0.0, 5.0, 5.0, 0.0, 5.0]);
    helpers::verify_valid_output(&tess);
}

#[test]
fn valid_indices_complex_polygon() {
    let data = include_str!("data/dude.dat");
    let contours = helpers::parse_contours(data);
    let tess = helpers::tessellate_contours(&contours, WindingRule::Positive);
    helpers::verify_valid_output(&tess);
}

#[test]
fn valid_indices_star() {
    let data = include_str!("data/star.dat");
    let contours = helpers::parse_contours(data);
    for &rule in &[
        WindingRule::Odd,
        WindingRule::NonZero,
        WindingRule::Positive,
    ] {
        let tess = helpers::tessellate_contours(&contours, rule);
        helpers::verify_valid_output(&tess);
    }
}

// --- No degenerate triangles for well-formed input ---

#[test]
fn no_degenerate_triangles_square() {
    let tess = tessellate_simple(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0]);
    helpers::verify_no_degenerate_triangles(&tess, 1e-6);
}

#[test]
fn no_degenerate_triangles_pentagon() {
    use std::f32::consts::PI;
    let mut pent = Vec::new();
    for i in 0..5 {
        let angle = 2.0 * PI * i as f32 / 5.0 - PI / 2.0;
        pent.push(100.0 * angle.cos());
        pent.push(100.0 * angle.sin());
    }
    let tess = tessellate_simple(&pent);
    helpers::verify_no_degenerate_triangles(&tess, 1e-3);
}

// --- Vertex bounds ---

#[test]
fn output_vertices_within_input_bounds() {
    let input = &[10.0f32, 20.0, 50.0, 20.0, 50.0, 80.0, 10.0, 80.0];
    let tess = tessellate_simple(input);
    let verts = tess.vertices();

    for i in 0..tess.vertex_count() {
        let x = verts[i * 2];
        let y = verts[i * 2 + 1];
        assert!(
            x >= 10.0 - 0.001 && x <= 50.0 + 0.001,
            "x={} out of input bounds [10, 50]",
            x
        );
        assert!(
            y >= 20.0 - 0.001 && y <= 80.0 + 0.001,
            "y={} out of input bounds [20, 80]",
            y
        );
    }
}

#[test]
fn output_vertices_within_bounds_star() {
    let data = include_str!("data/star.dat");
    let contours = helpers::parse_contours(data);
    let tess = helpers::tessellate_contours(&contours, WindingRule::Odd);
    let verts = tess.vertices();

    for i in 0..tess.vertex_count() {
        let x = verts[i * 2];
        let y = verts[i * 2 + 1];
        assert!(x.is_finite(), "x not finite");
        assert!(y.is_finite(), "y not finite");
        // Star vertices range: x in [231, 469], y in [75, 301]
        assert!(x >= 230.0 && x <= 470.0, "x={} out of star bounds", x);
        assert!(y >= 74.0 && y <= 302.0, "y={} out of star bounds", y);
    }
}

// --- Vertex indices mapping ---

#[test]
fn vertex_indices_mapping() {
    let tess = tessellate_simple(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    let indices = tess.vertex_indices();
    assert_eq!(
        indices.len(),
        tess.vertex_count(),
        "vertex_indices length should match vertex_count"
    );

    for &idx in indices {
        assert!(
            idx == u32::MAX || (idx as usize) < 100,
            "vertex index {} seems out of range",
            idx
        );
    }
}

// --- Multiple contours area ---

#[test]
fn area_two_separate_triangles() {
    let mut tess = Tessellator::new();
    // Two non-overlapping triangles
    tess.add_contour(2, &[0.0f32, 0.0, 1.0, 0.0, 0.5, 1.0]);
    tess.add_contour(2, &[5.0f32, 5.0, 6.0, 5.0, 5.5, 6.0]);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok);

    let area = helpers::total_tessellation_area(&tess);
    assert!(
        (area - 1.0).abs() < 0.01,
        "two triangles each area 0.5 should total 1.0, got {}",
        area
    );
    helpers::verify_valid_output(&tess);
}

// --- Consistent element count ---

#[test]
fn element_count_matches_elements_length() {
    let tess = tessellate_simple(&[0.0, 0.0, 3.0, 0.0, 3.0, 3.0, 0.0, 3.0]);
    let elems = tess.elements();
    assert_eq!(
        elems.len(),
        tess.element_count() * 3,
        "elements length should be element_count * 3 for triangles"
    );
}
