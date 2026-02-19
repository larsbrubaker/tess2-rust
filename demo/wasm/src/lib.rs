// Copyright 2025 Lars Brubaker
// WASM bindings for tess2-rust

use wasm_bindgen::prelude::*;
use tess2_rust::{Tessellator, WindingRule, ElementType};

/// Tessellate a flat array of 2D vertices (x0,y0, x1,y1, ...) and return
/// the triangle element indices as a flat Uint32Array.
#[wasm_bindgen]
pub fn tessellate_polygon(vertices: &[f32], winding: u32) -> Vec<u32> {
    let wr = match winding {
        0 => WindingRule::Odd,
        1 => WindingRule::NonZero,
        2 => WindingRule::Positive,
        3 => WindingRule::Negative,
        4 => WindingRule::AbsGeqTwo,
        _ => WindingRule::Odd,
    };

    let mut tess = Tessellator::new();
    tess.add_contour(2, vertices);
    if !tess.tessellate(wr, ElementType::Polygons, 3, 2, None) {
        return Vec::new();
    }
    tess.elements().to_vec()
}

/// Return the tessellated vertex positions as a Float32Array.
#[wasm_bindgen]
pub fn tessellate_polygon_vertices(vertices: &[f32], winding: u32) -> Vec<f32> {
    let wr = match winding {
        0 => WindingRule::Odd,
        1 => WindingRule::NonZero,
        2 => WindingRule::Positive,
        3 => WindingRule::Negative,
        4 => WindingRule::AbsGeqTwo,
        _ => WindingRule::Odd,
    };

    let mut tess = Tessellator::new();
    tess.add_contour(2, vertices);
    if !tess.tessellate(wr, ElementType::Polygons, 3, 2, None) {
        return Vec::new();
    }
    tess.vertices().to_vec()
}
