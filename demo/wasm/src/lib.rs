// Copyright 2025 Lars Brubaker
// WASM bindings for tess2-rust

use tess2_rust::{ElementType, TessOption, Tessellator, WindingRule};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main_js() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    // Always set in debug; in release we use the feature flag via the dep.
    console_error_panic_hook::set_once();
}

/// A stateful tessellator that can accumulate multiple contours then tessellate.
#[wasm_bindgen]
pub struct TessellatorJs {
    inner: Tessellator,
}

#[wasm_bindgen]
impl TessellatorJs {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TessellatorJs {
        TessellatorJs {
            inner: Tessellator::new(),
        }
    }

    /// Add a contour from a flat [x0,y0, x1,y1, ...] Float32Array.
    pub fn add_contour(&mut self, vertices: &[f32]) {
        self.inner.add_contour(2, vertices);
    }

    /// Set an option (0 = ConstrainedDelaunay, 1 = ReverseContours).
    pub fn set_option(&mut self, option: u32, value: bool) {
        let opt = match option {
            0 => TessOption::ConstrainedDelaunayTriangulation,
            1 => TessOption::ReverseContours,
            _ => return,
        };
        self.inner.set_option(opt, value);
    }

    /// Tessellate and return true on success.
    /// winding: 0=Odd 1=NonZero 2=Positive 3=Negative 4=AbsGeqTwo
    pub fn tessellate(&mut self, winding: u32) -> bool {
        let wr = winding_rule(winding);
        self.inner.tessellate(wr, ElementType::Polygons, 3, 2, None)
    }

    /// Tessellate with full control over element type and polygon size.
    /// element_type: 0=Polygons 1=ConnectedPolygons 2=BoundaryContours
    pub fn tessellate_full(&mut self, winding: u32, element_type: u32, poly_size: u32) -> bool {
        let wr = winding_rule(winding);
        let et = match element_type {
            0 => ElementType::Polygons,
            1 => ElementType::ConnectedPolygons,
            2 => ElementType::BoundaryContours,
            _ => ElementType::Polygons,
        };
        self.inner.tessellate(wr, et, poly_size as usize, 2, None)
    }

    /// Number of output elements (triangles, polygons, or contours depending on element type).
    pub fn element_count(&self) -> u32 {
        self.inner.element_count() as u32
    }

    pub fn vertex_count(&self) -> u32 {
        self.inner.vertex_count() as u32
    }

    /// Flat triangle vertex-index triples [i0,i1,i2, ...].
    pub fn get_elements(&self) -> Vec<u32> {
        self.inner.elements().to_vec()
    }

    /// Flat vertex positions [x0,y0, x1,y1, ...] for the output mesh.
    pub fn get_vertices(&self) -> Vec<f32> {
        self.inner.vertices().to_vec()
    }
}

fn winding_rule(winding: u32) -> WindingRule {
    match winding {
        0 => WindingRule::Odd,
        1 => WindingRule::NonZero,
        2 => WindingRule::Positive,
        3 => WindingRule::Negative,
        4 => WindingRule::AbsGeqTwo,
        _ => WindingRule::Odd,
    }
}

/// Convenience: tessellate a single closed contour of 2D vertices.
/// Returns flat [x0,y0, x1,y1, ...] vertex array (use get_elements for indices).
#[wasm_bindgen]
pub fn tessellate_polygon(vertices: &[f32], winding: u32) -> Vec<f32> {
    let mut t = TessellatorJs::new();
    t.add_contour(vertices);
    if !t.tessellate(winding) {
        return Vec::new();
    }
    t.get_vertices()
}

/// Convenience: same as tessellate_polygon but returns the element index array.
#[wasm_bindgen]
pub fn tessellate_polygon_elements(vertices: &[f32], winding: u32) -> Vec<u32> {
    let mut t = TessellatorJs::new();
    t.add_contour(vertices);
    if !t.tessellate(winding) {
        return Vec::new();
    }
    t.get_elements()
}
