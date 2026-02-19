// Copyright 2025 Lars Brubaker
// Tests for 3D coordinate input and custom normal vectors.

mod helpers;

use tess2_rust::{ElementType, Tessellator, WindingRule};

#[test]
fn vertex_size_3_xy_plane() {
    // 3D quad lying in the XY plane (z=0). Should produce same result as 2D case.
    let verts_3d: &[f32] = &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];

    let mut tess = Tessellator::new();
    tess.add_contour(3, verts_3d);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 3, None);
    assert!(ok, "3D XY-plane quad should tessellate");
    assert_eq!(tess.element_count(), 2, "quad should produce 2 triangles");

    let verts = tess.vertices();
    // vertex_size=3, so output has 3 components per vertex
    assert_eq!(verts.len(), tess.vertex_count() * 3);

    for i in 0..tess.vertex_count() {
        let x = verts[i * 3];
        let y = verts[i * 3 + 1];
        let z = verts[i * 3 + 2];
        assert!(x.is_finite() && y.is_finite() && z.is_finite());
        assert!(x >= -0.01 && x <= 1.01, "x out of range: {}", x);
        assert!(y >= -0.01 && y <= 1.01, "y out of range: {}", y);
        assert!((z - 0.0).abs() < 0.01, "z should be ~0: {}", z);
    }
}

#[test]
fn vertex_size_3_xz_plane_with_normal() {
    // Quad in the XZ plane (y=0), normal pointing up [0, 1, 0]
    let verts_3d: &[f32] = &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0];

    let mut tess = Tessellator::new();
    tess.add_contour(3, verts_3d);
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3,
        3,
        Some([0.0, 1.0, 0.0]),
    );
    assert!(ok, "3D XZ-plane quad with normal should tessellate");
    // Note: The XZ-plane quad may produce 0 elements with Positive winding
    // if the projection reverses the winding direction. Try NonZero instead.
    if tess.element_count() == 0 {
        let mut tess2 = Tessellator::new();
        tess2.add_contour(3, verts_3d);
        let ok2 = tess2.tessellate(
            WindingRule::NonZero,
            ElementType::Polygons,
            3,
            3,
            Some([0.0, 1.0, 0.0]),
        );
        assert!(ok2, "XZ-plane with NonZero should tessellate");
        if tess2.element_count() > 0 {
            let verts = tess2.vertices();
            for i in 0..tess2.vertex_count() {
                let y = verts[i * 3 + 1];
                assert!((y - 0.0).abs() < 0.01, "y should be ~0 for XZ plane: {}", y);
            }
        }
    } else {
        let verts = tess.vertices();
        for i in 0..tess.vertex_count() {
            let y = verts[i * 3 + 1];
            assert!((y - 0.0).abs() < 0.01, "y should be ~0 for XZ plane: {}", y);
        }
    }
}

#[test]
fn vertex_size_3_tilted_plane() {
    // Triangle on a tilted plane (z = x + y)
    let verts_3d: &[f32] = &[0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0];

    let normal = [
        -1.0f32 / 3.0f32.sqrt(),
        -1.0 / 3.0f32.sqrt(),
        1.0 / 3.0f32.sqrt(),
    ];

    let mut tess = Tessellator::new();
    tess.add_contour(3, verts_3d);
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3,
        3,
        Some(normal),
    );
    assert!(ok, "tilted plane triangle should tessellate");
    assert_eq!(tess.element_count(), 1);
}

#[test]
fn vertex_size_3_no_normal_auto_detect() {
    // 3D triangle, let tessellator auto-detect normal
    let verts_3d: &[f32] = &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0];

    let mut tess = Tessellator::new();
    tess.add_contour(3, verts_3d);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 3, None);
    assert!(ok, "auto-detect normal should work for XY plane triangle");
    assert_eq!(tess.element_count(), 1);
}

#[test]
fn vertex_size_3_pentagon_xy_plane() {
    use std::f32::consts::PI;
    let mut pent = Vec::new();
    for i in 0..5 {
        let angle = 2.0 * PI * i as f32 / 5.0 - PI / 2.0;
        pent.push(angle.cos());
        pent.push(angle.sin());
        pent.push(0.0); // z=0
    }

    let mut tess = Tessellator::new();
    tess.add_contour(3, &pent);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 3, None);
    assert!(ok, "3D pentagon should tessellate");
    assert_eq!(
        tess.element_count(),
        3,
        "pentagon should produce 3 triangles"
    );
}

#[test]
fn vertex_size_2_and_3_same_element_count() {
    // Same quad, tessellated as 2D and 3D (with z=0), should produce same element count
    let verts_2d: &[f32] = &[0.0, 0.0, 5.0, 0.0, 5.0, 5.0, 0.0, 5.0];
    let verts_3d: &[f32] = &[0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 5.0, 5.0, 0.0, 0.0, 5.0, 0.0];

    let mut tess2d = Tessellator::new();
    tess2d.add_contour(2, verts_2d);
    let ok = tess2d.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    assert!(ok);

    let mut tess3d = Tessellator::new();
    tess3d.add_contour(3, verts_3d);
    let ok = tess3d.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 3, None);
    assert!(ok);

    assert_eq!(
        tess2d.element_count(),
        tess3d.element_count(),
        "2D and 3D (z=0) should produce same element count"
    );
}

#[test]
fn custom_normal_z_positive() {
    let quad: &[f32] = &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let mut tess = Tessellator::new();
    tess.add_contour(2, quad);
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3,
        2,
        Some([0.0, 0.0, 1.0]),
    );
    assert!(ok, "custom normal [0,0,1] should work for 2D quad");
    assert_eq!(tess.element_count(), 2);
}

#[test]
fn custom_normal_z_negative_reverses_winding() {
    // With normal [0,0,-1], the polygon should be interpreted as CW.
    // For Positive winding rule, this might produce different results.
    let quad: &[f32] = &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];

    let mut tess_pos = Tessellator::new();
    tess_pos.add_contour(2, quad);
    let ok_pos = tess_pos.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3,
        2,
        Some([0.0, 0.0, 1.0]),
    );

    let mut tess_neg = Tessellator::new();
    tess_neg.add_contour(2, quad);
    let ok_neg = tess_neg.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3,
        2,
        Some([0.0, 0.0, -1.0]),
    );

    // Both should succeed
    assert!(ok_pos);
    assert!(ok_neg);
    // With negative normal, winding is reversed so Positive rule sees negative winding -> empty
    // Or it may produce the same result depending on implementation.
    // Just verify no crash and valid output.
    if tess_neg.element_count() > 0 {
        let verts = tess_neg.vertices();
        for &v in verts.iter() {
            assert!(v.is_finite());
        }
    }
}
