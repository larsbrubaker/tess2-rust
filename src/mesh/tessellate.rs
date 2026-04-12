// Monotone-region and interior tessellation for Mesh.
// Split from mod.rs to keep that file under the 967-line limit.

use super::{Mesh, F_HEAD};

impl Mesh {
    /// Tessellate a single monotone region (face).
    /// The face must be a CCW-oriented simple polygon.
    pub fn tessellate_mono_region(&mut self, face: super::FaceIdx) -> bool {
        use crate::geom::{edge_sign, vert_leq};

        let mut up = self.faces[face as usize].an_edge;
        if self.edges[up as usize].lnext == up
            || self.edges[self.edges[up as usize].lnext as usize].lnext == up
        {
            return false; // degenerate face (< 3 edges) — skip instead of panic
        }

        // Find the edge whose origin vertex is rightmost (largest s).
        // VertLeq(Dst, Org) means Dst <= Org (going left = bad), so we want
        // to find an edge where the Org is the rightmost.
        let max_ring_iters = self.edges.len() + 2;
        let mut ring_iter = 0usize;
        loop {
            let up_dst = self.dst(up);
            let up_org = self.edges[up as usize].org;
            if !vert_leq(
                self.verts[up_dst as usize].s,
                self.verts[up_dst as usize].t,
                self.verts[up_org as usize].s,
                self.verts[up_org as usize].t,
            ) {
                break;
            }
            up = self.lprev(up);
            ring_iter += 1;
            if ring_iter > max_ring_iters {
                return false; // degenerate face — all vertices co-sorted
            }
        }
        ring_iter = 0;
        loop {
            let up_org = self.edges[up as usize].org;
            let up_dst = self.dst(up);
            if !vert_leq(
                self.verts[up_org as usize].s,
                self.verts[up_org as usize].t,
                self.verts[up_dst as usize].s,
                self.verts[up_dst as usize].t,
            ) {
                break;
            }
            up = self.edges[up as usize].lnext;
            ring_iter += 1;
            if ring_iter > max_ring_iters {
                return false; // degenerate face — all vertices co-sorted
            }
        }

        let mut lo = self.lprev(up);

        let max_tess_iters = self.edges.len() * 2 + 4;
        let mut outer_iter = 0usize;
        while self.edges[up as usize].lnext != lo {
            outer_iter += 1;
            if outer_iter > max_tess_iters {
                return false; // degenerate region — guard against infinite triangulation
            }
            let up_dst = self.dst(up);
            let lo_org = self.edges[lo as usize].org;
            if vert_leq(
                self.verts[up_dst as usize].s,
                self.verts[up_dst as usize].t,
                self.verts[lo_org as usize].s,
                self.verts[lo_org as usize].t,
            ) {
                // up->Dst is on the left; make triangles from lo->Org
                let mut inner_iter = 0usize;
                while self.edges[lo as usize].lnext != up {
                    inner_iter += 1;
                    if inner_iter > max_tess_iters {
                        return false;
                    }
                    let lo_lnext = self.edges[lo as usize].lnext;
                    let lo_lnext_dst = self.dst(lo_lnext);
                    let lo_org2 = self.edges[lo as usize].org;
                    let lo_dst = self.dst(lo);
                    let goes_left = self.edge_goes_left(lo_lnext);
                    let sign_val = edge_sign(
                        self.verts[lo_org2 as usize].s,
                        self.verts[lo_org2 as usize].t,
                        self.verts[lo_dst as usize].s,
                        self.verts[lo_dst as usize].t,
                        self.verts[lo_lnext_dst as usize].s,
                        self.verts[lo_lnext_dst as usize].t,
                    );
                    if !goes_left && sign_val > 0.0 {
                        break;
                    }
                    let temp = match self.connect(lo_lnext, lo) {
                        Some(e) => e,
                        None => return false,
                    };
                    lo = temp ^ 1;
                }
                lo = self.lprev(lo);
            } else {
                // lo->Org is on the left; make CCW triangles from up->Dst
                let mut inner_iter = 0usize;
                while self.edges[lo as usize].lnext != up {
                    inner_iter += 1;
                    if inner_iter > max_tess_iters {
                        return false;
                    }
                    let up_lprev = self.lprev(up);
                    let up_lprev_org = self.edges[up_lprev as usize].org;
                    let up_dst2 = self.dst(up);
                    let up_org2 = self.edges[up as usize].org;
                    let goes_right = self.edge_goes_right(up_lprev);
                    let sign_val = edge_sign(
                        self.verts[up_dst2 as usize].s,
                        self.verts[up_dst2 as usize].t,
                        self.verts[up_org2 as usize].s,
                        self.verts[up_org2 as usize].t,
                        self.verts[up_lprev_org as usize].s,
                        self.verts[up_lprev_org as usize].t,
                    );
                    if !goes_right && sign_val < 0.0 {
                        break;
                    }
                    let temp = match self.connect(up, up_lprev) {
                        Some(e) => e,
                        None => return false,
                    };
                    up = temp ^ 1;
                }
                up = self.edges[up as usize].lnext;
            }
        }

        // Tessellate the remaining fan from the leftmost vertex.
        if self.edges[lo as usize].lnext == up {
            return false; // degenerate — no fan to tessellate
        }
        let mut fan_iter = 0usize;
        while self.edges[self.edges[lo as usize].lnext as usize].lnext != up {
            fan_iter += 1;
            if fan_iter > max_tess_iters {
                return false;
            }
            let lo_lnext = self.edges[lo as usize].lnext;
            let temp = match self.connect(lo_lnext, lo) {
                Some(e) => e,
                None => return false,
            };
            lo = temp ^ 1;
        }

        true
    }

    /// Tessellate all interior monotone regions.
    pub fn tessellate_interior(&mut self) -> bool {
        let mut f = self.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            let next = self.faces[f as usize].next;
            if self.faces[f as usize].inside {
                if !self.tessellate_mono_region(f) {
                    // Mark as outside so the output extraction skips this face.
                    // Leaving it inside=true would cause degenerate triangles with
                    // wrong vertices to be emitted (the untriangulated polygon edges
                    // get read as triangle vertices during output).
                    self.faces[f as usize].inside = false;
                }
            }
            f = next;
        }
        true
    }
}
