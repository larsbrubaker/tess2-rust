// Copyright 2025 Lars Brubaker
// License: SGI Free Software License B (MIT-compatible)
//
// GLU render-order output: the triangles in the order agg-sharp's callback
// tessellator (`Tesselate.Tesselator`, a C# port of the SGI GLU tessellator)
// emits them when an edge-flag callback is set, numbered as its client
// indices.
//
// With an edge-flag callback, GLU's `RenderMesh` never builds fans or strips:
// `RenderMaximumFaceGroup` sends every interior face to `RenderTriangle`,
// which *prepends* it to the lonely-triangle trail (`Face.AddToTrail`), and
// `RenderLonelyTriangles` then walks that trail.  So the faces come out in
// the reverse of the mesh's face-list order, each starting at the face's
// `anEdge` origin and walking `Lnext`.  `output_polymesh` walks the same face
// list forward from the same `anEdge`, so the render order is its triangles
// reversed.
//
// Client indices: every input vertex keeps the index it was added with;
// every combine callback (`SpliceMergeVertices` for coincident vertices,
// `GetIntersectData` for edge crossings) gives the surviving vertex a new
// index, one past the last one handed out, as agg-sharp's `CachedTesselator`
// does by appending the combined point to its vertex cache.  The combine
// sites in `mod.rs`, `connect.rs` and `dirty_regions.rs` call
// [`Tessellator::record_combine`] in the order the sweep reaches them.

use crate::geom::Real;
use crate::mesh::VertIdx;

use super::{ElementType, Tessellator, TESS_UNDEF};

/// One emitted triangle corner (agg-sharp `CachedTesselator.RenderIndices`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RenderIndex {
    /// Index into [`RenderOrder::vertices`] (GLU's client index).
    pub index: u32,
    /// The edge from this corner to the next corner of the same triangle is
    /// on the polygon's outline (the edge flag in force when GLU emitted the
    /// corner).
    pub is_edge: bool,
}

/// The last tessellation as GLU's callback interface reports it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderOrder {
    /// Every vertex passed to `add_contour`, in order, followed by one entry
    /// per combine callback, in the order the sweep made them (GLU passes
    /// each combined vertex's own coordinates).  Combined entries can repeat
    /// an input position: coincident vertices are combined, not dropped.
    pub vertices: Vec<[Real; 3]>,
    /// Three corners per triangle, in GLU's render order.
    pub indices: Vec<RenderIndex>,
}

impl Tessellator {
    /// Records a GLU combine callback on the surviving vertex `v`: it gets a
    /// new client index, and its current coordinates join the client vertex
    /// list.
    pub(super) fn record_combine(&mut self, v: VertIdx) {
        let coords = match self.mesh.as_ref() {
            Some(mesh) => mesh.verts[v as usize].coords,
            None => return,
        };
        let client = self.vertex_index_counter + self.combined_coords.len() as u32;
        self.combined_coords.push(coords);
        self.client_override.insert(v, client);
    }

    /// The last tessellation's triangles in the order agg-sharp's GLU port
    /// emits them through its callbacks with an edge-flag callback set (as
    /// MatterCAD's `CachedTesselator` does), with GLU's client vertex
    /// numbering.  The triangles, their corners and edge flags are those of
    /// [`Tessellator::elements`]; only the order and the numbering differ.
    ///
    /// Returns `None` unless the last [`Tessellator::tessellate`] succeeded
    /// with [`ElementType::Polygons`] and `poly_size == 3`.  GLU has no
    /// Delaunay refinement, so leave
    /// [`super::TessOption::ConstrainedDelaunayTriangulation`] off when the
    /// order must match GLU's.
    pub fn render_order(&self) -> Option<RenderOrder> {
        if self.out_element_type != Some(ElementType::Polygons) || self.out_poly_size != 3 {
            return None;
        }

        let mut vertices = self.input_coords.clone();
        vertices.extend_from_slice(&self.combined_coords);

        let mut indices = Vec::with_capacity(self.out_elements.len());
        for (triangle, corners) in self.out_elements.chunks_exact(3).enumerate().rev() {
            for (k, &n) in corners.iter().enumerate() {
                if n == TESS_UNDEF {
                    continue;
                }
                indices.push(RenderIndex {
                    index: self.out_vertex_clients[n as usize],
                    is_edge: self.out_edge_flags.get(triangle * 3 + k).copied() == Some(1),
                });
            }
        }

        Some(RenderOrder { vertices, indices })
    }
}
