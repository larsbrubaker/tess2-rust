// Copyright 2025 Lars Brubaker
// Tests for Constrained Delaunay Triangulation (CDT) option.

mod helpers;

use tess2_rust::{ElementType, TessOption, Tessellator, WindingRule};

fn tessellate_with_cdt(vertices: &[f32], cdt: bool) -> Tessellator {
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ConstrainedDelaunayTriangulation, cdt);
    tess.add_contour(2, vertices);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok, "tessellation failed (cdt={})", cdt);
    tess
}

#[test]
fn cdt_quad_produces_valid_output() {
    let quad = &[0.0f32, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let tess = tessellate_with_cdt(quad, true);
    assert_eq!(
        tess.element_count(),
        2,
        "quad should produce 2 triangles with CDT"
    );
    helpers::verify_valid_output(&tess);
}

#[test]
fn cdt_triangle_produces_one_triangle() {
    let tri = &[0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0];
    let tess = tessellate_with_cdt(tri, true);
    assert_eq!(tess.element_count(), 1);
    helpers::verify_valid_output(&tess);
}

#[test]
fn cdt_preserves_area() {
    let quad = &[0.0f32, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let tess_normal = tessellate_with_cdt(quad, false);
    let tess_cdt = tessellate_with_cdt(quad, true);

    let area_normal = helpers::total_tessellation_area(&tess_normal);
    let area_cdt = helpers::total_tessellation_area(&tess_cdt);

    assert!(
        (area_normal - area_cdt).abs() < 0.01,
        "CDT and normal should produce same area: {} vs {}",
        area_normal,
        area_cdt
    );
}

#[test]
fn cdt_complex_polygon() {
    let data = include_str!("data/dude.dat");
    let contours = helpers::parse_contours(data);

    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ConstrainedDelaunayTriangulation, true);
    for contour in &contours {
        tess.add_contour(2, contour);
    }
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok, "CDT tessellation of dude should succeed");
    helpers::verify_valid_output(&tess);
    assert!(tess.element_count() > 0, "CDT should produce triangles");

    let area = helpers::total_tessellation_area(&tess);
    assert!(area > 0.0, "CDT dude should have non-zero area");
}

#[test]
fn cdt_star() {
    let data = include_str!("data/star.dat");
    let contours = helpers::parse_contours(data);

    let mut tess_normal = Tessellator::new();
    for contour in &contours {
        tess_normal.add_contour(2, contour);
    }
    let ok = tess_normal.tessellate(WindingRule::Odd, ElementType::Polygons, 3, 2, None);
    assert!(ok);

    let mut tess_cdt = Tessellator::new();
    tess_cdt.set_option(TessOption::ConstrainedDelaunayTriangulation, true);
    for contour in &contours {
        tess_cdt.add_contour(2, contour);
    }
    let ok = tess_cdt.tessellate(WindingRule::Odd, ElementType::Polygons, 3, 2, None);
    assert!(ok);

    helpers::verify_valid_output(&tess_cdt);

    let area_normal = helpers::total_tessellation_area(&tess_normal);
    let area_cdt = helpers::total_tessellation_area(&tess_cdt);
    assert!(
        (area_normal - area_cdt).abs() < 1.0,
        "CDT and normal star areas should be similar: {} vs {}",
        area_normal,
        area_cdt
    );
}

#[test]
fn cdt_polygon_with_hole() {
    // Tessellate with CDT - outer square with inner hole
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ConstrainedDelaunayTriangulation, true);
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &[0.0f32, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0]);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &[3.0f32, 3.0, 7.0, 3.0, 7.0, 7.0, 3.0, 7.0]);

    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok, "CDT polygon with hole should succeed");
    helpers::verify_valid_output(&tess);
    assert!(tess.element_count() > 0, "CDT should produce triangles");

    let area = helpers::total_tessellation_area(&tess);
    assert!(
        area > 0.0,
        "CDT polygon with hole should have positive area"
    );
}

#[test]
fn cdt_toggle_does_not_crash() {
    // Toggle CDT on/off between tessellations (reuse check).
    let quad = &[0.0f32, 0.0, 5.0, 0.0, 5.0, 5.0, 0.0, 5.0];

    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ConstrainedDelaunayTriangulation, true);
    tess.add_contour(2, quad);
    let _ = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);

    let mut tess2 = Tessellator::new();
    tess2.set_option(TessOption::ConstrainedDelaunayTriangulation, false);
    tess2.add_contour(2, quad);
    let ok = tess2.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok);
}
