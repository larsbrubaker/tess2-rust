// Copyright 2025 Lars Brubaker
// Shared test utilities for tess2-rust tests.

#![allow(dead_code)]

use tess2_rust::{ElementType, Tessellator, WindingRule};

/// Parse tess2.js `.dat` format: one vertex per line as `x y` or `x, y`.
/// Blank lines separate contours. Returns a Vec of contours, each a flat f32 array.
pub fn parse_contours(data: &str) -> Vec<Vec<f32>> {
    let mut contours: Vec<Vec<f32>> = Vec::new();
    let mut current: Vec<f32> = Vec::new();

    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                contours.push(std::mem::take(&mut current));
            }
            continue;
        }
        let floats: Vec<f32> = trimmed
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        current.extend(floats);
    }
    if !current.is_empty() {
        contours.push(current);
    }
    contours
}

/// Signed area of a triangle given 3 vertices (2D).
pub fn triangle_area(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    0.5 * ((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0))
}

/// Compute total absolute area of all output triangles from a tessellation.
/// Assumes element type is Polygons with poly_size=3.
pub fn total_tessellation_area(tess: &Tessellator) -> f32 {
    let verts = tess.vertices();
    let elems = tess.elements();
    let mut total = 0.0f32;
    for tri in elems.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let x0 = verts[i0 * 2];
        let y0 = verts[i0 * 2 + 1];
        let x1 = verts[i1 * 2];
        let y1 = verts[i1 * 2 + 1];
        let x2 = verts[i2 * 2];
        let y2 = verts[i2 * 2 + 1];
        total += triangle_area(x0, y0, x1, y1, x2, y2).abs();
    }
    total
}

/// Compute total signed area of all output triangles from a tessellation.
pub fn total_tessellation_signed_area(tess: &Tessellator) -> f32 {
    let verts = tess.vertices();
    let elems = tess.elements();
    let mut total = 0.0f32;
    for tri in elems.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let x0 = verts[i0 * 2];
        let y0 = verts[i0 * 2 + 1];
        let x1 = verts[i1 * 2];
        let y1 = verts[i1 * 2 + 1];
        let x2 = verts[i2 * 2];
        let y2 = verts[i2 * 2 + 1];
        total += triangle_area(x0, y0, x1, y1, x2, y2);
    }
    total
}

/// Verify that all tessellation output is valid: indices in range, vertices finite, etc.
pub fn verify_valid_output(tess: &Tessellator) {
    let verts = tess.vertices();
    let elems = tess.elements();
    let vert_count = tess.vertex_count();

    for (i, &v) in verts.iter().enumerate() {
        assert!(
            v.is_finite(),
            "vertex component [{}] = {} is not finite",
            i,
            v
        );
    }

    for (i, &idx) in elems.iter().enumerate() {
        if idx == u32::MAX {
            continue; // TESS_UNDEF sentinel for unused polygon slots
        }
        assert!(
            (idx as usize) < vert_count,
            "element[{}] = {} out of range (vertex_count={})",
            i,
            idx,
            vert_count
        );
    }
}

/// Verify no degenerate (zero-area) triangles in output.
/// Uses a small epsilon for floating-point tolerance.
pub fn verify_no_degenerate_triangles(tess: &Tessellator, epsilon: f32) {
    let verts = tess.vertices();
    let elems = tess.elements();
    for (i, tri) in elems.chunks(3).enumerate() {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let area = triangle_area(
            verts[i0 * 2],
            verts[i0 * 2 + 1],
            verts[i1 * 2],
            verts[i1 * 2 + 1],
            verts[i2 * 2],
            verts[i2 * 2 + 1],
        )
        .abs();
        assert!(
            area > epsilon,
            "triangle {} is degenerate (area={}, vertices=({},{}) ({},{}) ({},{}))",
            i,
            area,
            verts[i0 * 2],
            verts[i0 * 2 + 1],
            verts[i1 * 2],
            verts[i1 * 2 + 1],
            verts[i2 * 2],
            verts[i2 * 2 + 1],
        );
    }
}

/// Helper: tessellate contours with the given winding rule as triangles (poly_size=3, 2D).
pub fn tessellate_contours(contours: &[Vec<f32>], winding_rule: WindingRule) -> Tessellator {
    let mut tess = Tessellator::new();
    for contour in contours {
        tess.add_contour(2, contour);
    }
    let ok = tess.tessellate(winding_rule, ElementType::Polygons, 3, 2, None);
    assert!(
        ok,
        "tessellation failed for winding rule {:?}",
        winding_rule
    );
    tess
}

/// Compute the signed area of a simple polygon given as flat [x0,y0,x1,y1,...].
pub fn polygon_signed_area(verts: &[f32]) -> f32 {
    let n = verts.len() / 2;
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        area += verts[i * 2] * verts[j * 2 + 1];
        area -= verts[j * 2] * verts[i * 2 + 1];
    }
    area * 0.5
}
