// Copyright 2025 Lars Brubaker
// Tests for poly_size > 3 (quad output, hex output, etc.).

mod helpers;

use tess2_rust::{ElementType, Tessellator, WindingRule};

fn tessellate_with_poly_size(vertices: &[f32], poly_size: usize) -> Tessellator {
    let mut tess = Tessellator::new();
    tess.add_contour(2, vertices);
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        poly_size,
        2,
        None,
    );
    assert!(ok, "tessellation failed with poly_size={}", poly_size);
    tess
}

#[test]
fn poly_size_3_triangle() {
    let tess = tessellate_with_poly_size(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0], 3);
    assert_eq!(
        tess.element_count(),
        1,
        "triangle with poly_size=3 should give 1 element"
    );
}

#[test]
fn poly_size_4_quad() {
    let quad = &[0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let tess = tessellate_with_poly_size(quad, 4);
    assert!(
        tess.element_count() >= 1,
        "quad with poly_size=4 should produce >= 1 element"
    );

    let elems = tess.elements();
    // Each element has poly_size indices, unused slots are TESS_UNDEF (u32::MAX)
    assert_eq!(elems.len(), tess.element_count() * 4);

    for i in 0..tess.element_count() {
        let base = i * 4;
        let mut valid_verts = 0;
        for j in 0..4 {
            let idx = elems[base + j];
            if idx != u32::MAX {
                assert!(
                    (idx as usize) < tess.vertex_count(),
                    "element vertex index {} out of range",
                    idx
                );
                valid_verts += 1;
            }
        }
        assert!(
            valid_verts >= 3,
            "polygon should have at least 3 valid vertices"
        );
    }
}

#[test]
fn poly_size_4_pentagon() {
    use std::f32::consts::PI;
    let mut pent = Vec::new();
    for i in 0..5 {
        let angle = 2.0 * PI * i as f32 / 5.0 - PI / 2.0;
        pent.push(angle.cos());
        pent.push(angle.sin());
    }
    let tess = tessellate_with_poly_size(&pent, 4);
    assert!(tess.element_count() >= 1);
    helpers::verify_valid_output(&tess);
}

#[test]
fn poly_size_6_hexagon() {
    use std::f32::consts::PI;
    let mut hex = Vec::new();
    for i in 0..6 {
        let angle = PI / 3.0 * i as f32;
        hex.push(10.0 * angle.cos());
        hex.push(10.0 * angle.sin());
    }
    let tess = tessellate_with_poly_size(&hex, 6);
    assert!(tess.element_count() >= 1);

    let elems = tess.elements();
    assert_eq!(elems.len(), tess.element_count() * 6);
    helpers::verify_valid_output(&tess);
}

#[test]
fn poly_size_4_complex_shape() {
    let data = include_str!("data/dude.dat");
    let contours = helpers::parse_contours(data);

    let mut tess = Tessellator::new();
    for contour in &contours {
        tess.add_contour(2, contour);
    }
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 4, 2, None);
    assert!(ok, "dude with poly_size=4 should succeed");
    assert!(tess.element_count() > 0);
    helpers::verify_valid_output(&tess);
}

#[test]
fn poly_size_3_and_4_same_area() {
    let quad = &[0.0f32, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];

    let tess3 = tessellate_with_poly_size(quad, 3);
    let tess4 = tessellate_with_poly_size(quad, 4);

    let area3 = helpers::total_tessellation_area(&tess3);

    // For poly_size=4, compute area manually since elements may have 4 vertices
    let verts = tess4.vertices();
    let elems = tess4.elements();
    let mut area4 = 0.0f32;
    for i in 0..tess4.element_count() {
        let base = i * 4;
        let mut poly_verts: Vec<(f32, f32)> = Vec::new();
        for j in 0..4 {
            let idx = elems[base + j];
            if idx != u32::MAX {
                poly_verts.push((verts[idx as usize * 2], verts[idx as usize * 2 + 1]));
            }
        }
        // Fan triangulate the polygon and sum areas
        if poly_verts.len() >= 3 {
            for k in 1..poly_verts.len() - 1 {
                area4 += helpers::triangle_area(
                    poly_verts[0].0,
                    poly_verts[0].1,
                    poly_verts[k].0,
                    poly_verts[k].1,
                    poly_verts[k + 1].0,
                    poly_verts[k + 1].1,
                )
                .abs();
            }
        }
    }

    assert!(
        (area3 - area4).abs() < 0.1,
        "poly_size=3 area ({}) should match poly_size=4 area ({})",
        area3,
        area4
    );
}

#[test]
fn poly_size_16_quad() {
    // Large poly_size on a simple quad - should still work
    let quad = &[0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let tess = tessellate_with_poly_size(quad, 16);
    assert!(tess.element_count() >= 1);
    helpers::verify_valid_output(&tess);
}
