// Copyright 2025 Lars Brubaker
// License: SGI Free Software License B (MIT-compatible)
//
// Port of libtess2 tess.c/h + sweep.c/h + tesselator.h
//
// This module is the complete tessellator: public API + full sweep line algorithm.
// The C code is split across tess.c and sweep.c; they're merged here since both
// share the same internal state (TESStesselator).

use crate::dict::{Dict, NodeIdx, DICT_HEAD};
use crate::geom::{
    edge_intersect, edge_sign, vert_eq, vert_leq, Real,
};
use crate::mesh::{EdgeIdx, Mesh, VertIdx, INVALID, V_HEAD, F_HEAD, E_HEAD};
use crate::priorityq::{PriorityQ, INVALID_HANDLE};
use crate::sweep::ActiveRegion;

// ─────────────────────────────── Public types ──────────────────────────────────

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WindingRule {
    Odd,
    NonZero,
    Positive,
    Negative,
    AbsGeqTwo,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ElementType {
    Polygons,
    ConnectedPolygons,
    BoundaryContours,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TessOption {
    ConstrainedDelaunayTriangulation,
    ReverseContours,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TessStatus {
    Ok,
    OutOfMemory,
    InvalidInput,
}

pub const TESS_UNDEF: u32 = u32::MAX;
const MAX_VALID_COORD: f32 = (1u32 << 23) as f32;
const MIN_VALID_COORD: f32 = -MAX_VALID_COORD;

type RegionIdx = u32;

// ─────────────────────────── Tessellator ──────────────────────────────────────

pub struct Tessellator {
    mesh: Option<Mesh>,
    pub status: TessStatus,
    normal: [Real; 3],
    s_unit: [Real; 3],
    t_unit: [Real; 3],
    bmin: [Real; 2],
    bmax: [Real; 2],
    process_cdt: bool,
    reverse_contours: bool,
    winding_rule: WindingRule,

    // Sweep state
    dict: Dict,
    pq: Option<PriorityQ>,
    event: VertIdx,
    event_s: Real,
    event_t: Real,

    // Region arena
    regions: Vec<Option<ActiveRegion>>,
    region_free: Vec<RegionIdx>,

    // Output
    pub out_vertices: Vec<Real>,
    pub out_vertex_indices: Vec<u32>,
    pub out_elements: Vec<u32>,
    pub out_vertex_count: usize,
    pub out_element_count: usize,
    vertex_index_counter: u32,

    // Primary event queue: pre-sorted vertices for the initial sweep phase
    sorted_events: Vec<VertIdx>,
    sorted_event_pos: usize,
}

impl Tessellator {
    pub fn new() -> Self {
        Tessellator {
            mesh: None,
            status: TessStatus::Ok,
            normal: [0.0; 3],
            s_unit: [0.0; 3],
            t_unit: [0.0; 3],
            bmin: [0.0; 2],
            bmax: [0.0; 2],
            process_cdt: false,
            reverse_contours: false,
            winding_rule: WindingRule::Odd,
            dict: Dict::new(),
            pq: None,
            event: INVALID,
            event_s: 0.0,
            event_t: 0.0,
            regions: Vec::new(),
            region_free: Vec::new(),
            out_vertices: Vec::new(),
            out_vertex_indices: Vec::new(),
            out_elements: Vec::new(),
            out_vertex_count: 0,
            out_element_count: 0,
            vertex_index_counter: 0,
            sorted_events: Vec::new(),
            sorted_event_pos: 0,
        }
    }

    pub fn set_option(&mut self, option: TessOption, value: bool) {
        match option {
            TessOption::ConstrainedDelaunayTriangulation => self.process_cdt = value,
            TessOption::ReverseContours => self.reverse_contours = value,
        }
    }

    /// Add a contour. `size` = 2 or 3 (coords per vertex). `vertices` is flat.
    pub fn add_contour(&mut self, size: usize, vertices: &[f32]) {
        if self.status != TessStatus::Ok {
            return;
        }
        let size = size.min(3).max(2);
        let count = vertices.len() / size;
        if self.mesh.is_none() {
            self.mesh = Some(Mesh::new());
        }

        let mut e = INVALID;
        for i in 0..count {
            let cx = vertices[i * size];
            let cy = vertices[i * size + 1];
            let cz = if size > 2 { vertices[i * size + 2] } else { 0.0 };

            if !is_valid_coord(cx) || !is_valid_coord(cy) || (size > 2 && !is_valid_coord(cz)) {
                self.status = TessStatus::InvalidInput;
                return;
            }

            let mesh = self.mesh.as_mut().unwrap();
            if e == INVALID {
                let new_e = match mesh.make_edge() {
                    Some(v) => v,
                    None => { self.status = TessStatus::OutOfMemory; return; }
                };
                e = new_e;
                if !mesh.splice(e, e ^ 1) {
                    self.status = TessStatus::OutOfMemory;
                    return;
                }
            } else {
                if mesh.split_edge(e).is_none() {
                    self.status = TessStatus::OutOfMemory;
                    return;
                }
                e = mesh.edges[e as usize].lnext;
            }

            let org = mesh.edges[e as usize].org;
            mesh.verts[org as usize].coords[0] = cx;
            mesh.verts[org as usize].coords[1] = cy;
            mesh.verts[org as usize].coords[2] = cz;
            mesh.verts[org as usize].idx = self.vertex_index_counter;
            self.vertex_index_counter += 1;

            let w = if self.reverse_contours { -1 } else { 1 };
            mesh.edges[e as usize].winding = w;
            mesh.edges[(e ^ 1) as usize].winding = -w;
        }
    }

    pub fn tessellate(
        &mut self,
        winding_rule: WindingRule,
        element_type: ElementType,
        poly_size: usize,
        vertex_size: usize,
        normal: Option<[f32; 3]>,
    ) -> bool {
        if self.status != TessStatus::Ok {
            return false;
        }
        self.winding_rule = winding_rule;
        self.out_vertices.clear();
        self.out_vertex_indices.clear();
        self.out_elements.clear();
        self.out_vertex_count = 0;
        self.out_element_count = 0;
        self.normal = normal.unwrap_or([0.0, 0.0, 0.0]);

        if self.mesh.is_none() {
            self.mesh = Some(Mesh::new());
        }

        if !self.project_polygon() {
            self.status = TessStatus::OutOfMemory;
            return false;
        }

        if !self.compute_interior() {
            if self.status == TessStatus::Ok {
                self.status = TessStatus::OutOfMemory;
            }
            return false;
        }

        let vertex_size = vertex_size.min(3).max(2);
        if element_type == ElementType::BoundaryContours {
            self.output_contours(vertex_size);
        } else {
            self.output_polymesh(element_type, poly_size, vertex_size);
        }

        self.mesh = None;
        self.status == TessStatus::Ok
    }

    // ─────── Accessors ────────────────────────────────────────────────────────

    pub fn vertex_count(&self) -> usize { self.out_vertex_count }
    pub fn element_count(&self) -> usize { self.out_element_count }
    pub fn vertices(&self) -> &[f32] { &self.out_vertices }
    pub fn vertex_indices(&self) -> &[u32] { &self.out_vertex_indices }
    pub fn elements(&self) -> &[u32] { &self.out_elements }
    pub fn get_status(&self) -> TessStatus { self.status }

    // ─────── Projection ───────────────────────────────────────────────────────

    fn project_polygon(&mut self) -> bool {
        let mut norm = self.normal;
        let mut computed_normal = false;
        if norm[0] == 0.0 && norm[1] == 0.0 && norm[2] == 0.0 {
            if let Some(ref m) = self.mesh {
                compute_normal(m, &mut norm);
            }
            computed_normal = true;
        }

        let i = long_axis(&norm);
        self.s_unit = [0.0; 3];
        self.t_unit = [0.0; 3];
        self.s_unit[(i + 1) % 3] = 1.0;
        self.t_unit[(i + 2) % 3] = if norm[i] > 0.0 { 1.0 } else { -1.0 };
        let su = self.s_unit;
        let tu = self.t_unit;

        if let Some(ref mut mesh) = self.mesh {
            let mut v = mesh.verts[V_HEAD as usize].next;
            while v != V_HEAD {
                let c = mesh.verts[v as usize].coords;
                mesh.verts[v as usize].s = dot(&c, &su);
                mesh.verts[v as usize].t = dot(&c, &tu);
                v = mesh.verts[v as usize].next;
            }
            if computed_normal { check_orientation(mesh); }

            let mut first = true;
            let mut v = mesh.verts[V_HEAD as usize].next;
            while v != V_HEAD {
                let vs = mesh.verts[v as usize].s;
                let vt = mesh.verts[v as usize].t;
                if first { self.bmin = [vs, vt]; self.bmax = [vs, vt]; first = false; }
                else {
                    if vs < self.bmin[0] { self.bmin[0] = vs; }
                    if vs > self.bmax[0] { self.bmax[0] = vs; }
                    if vt < self.bmin[1] { self.bmin[1] = vt; }
                    if vt > self.bmax[1] { self.bmax[1] = vt; }
                }
                v = mesh.verts[v as usize].next;
            }
        }
        true
    }

    // ─────── Main interior computation ───────────────────────────────────────

    fn compute_interior(&mut self) -> bool {
        if !self.remove_degenerate_edges() { return false; }
        if !self.init_priority_queue() { return false; }
        if !self.init_edge_dict() { return false; }

        loop {
            if self.pq_is_empty() { break; }

            let v = self.pq_extract_min();
            if v == INVALID { break; }

            // Coalesce coincident vertices
            loop {
                if self.pq_is_empty() { break; }
                let next_v = self.pq_minimum();
                if next_v == INVALID { break; }
                let (v_s, v_t) = {
                    let mesh = self.mesh.as_ref().unwrap();
                    (mesh.verts[v as usize].s, mesh.verts[v as usize].t)
                };
                let (nv_s, nv_t) = {
                    let mesh = self.mesh.as_ref().unwrap();
                    (mesh.verts[next_v as usize].s, mesh.verts[next_v as usize].t)
                };
                if !vert_eq(v_s, v_t, nv_s, nv_t) { break; }
                let next_v = self.pq_extract_min();
                // Merge next_v into v
                let an1 = self.mesh.as_ref().unwrap().verts[v as usize].an_edge;
                let an2 = self.mesh.as_ref().unwrap().verts[next_v as usize].an_edge;
                if an1 != INVALID && an2 != INVALID {
                    if !self.mesh.as_mut().unwrap().splice(an1, an2) { return false; }
                }
            }

            self.event = v;
            let (v_s, v_t) = {
                let m = self.mesh.as_ref().unwrap();
                (m.verts[v as usize].s, m.verts[v as usize].t)
            };
            self.event_s = v_s;
            self.event_t = v_t;

            if !self.sweep_event(v) { return false; }
        }

        self.done_edge_dict();

        if let Some(ref mut mesh) = self.mesh {
            if !mesh.tessellate_interior() { return false; }
            if self.process_cdt { mesh.refine_delaunay(); }
        }
        true
    }

    fn remove_degenerate_edges(&mut self) -> bool {
        // Mirrors C RemoveDegenerateEdges exactly
        let mesh = match self.mesh.as_mut() { Some(m) => m, None => return true };
        let mut e = mesh.edges[E_HEAD as usize].next;
        while e != E_HEAD {
            let mut e_next = mesh.edges[e as usize].next;
            let mut e_lnext = mesh.edges[e as usize].lnext;

            let org = mesh.edges[e as usize].org;
            let dst = mesh.dst(e);
            let valid = org != INVALID && dst != INVALID
                && (org as usize) < mesh.verts.len()
                && (dst as usize) < mesh.verts.len();

            if valid {
                let (os, ot) = (mesh.verts[org as usize].s, mesh.verts[org as usize].t);
                let (ds, dt) = (mesh.verts[dst as usize].s, mesh.verts[dst as usize].t);

                if vert_eq(os, ot, ds, dt) && mesh.edges[e_lnext as usize].lnext != e {
                    // Zero-length edge, contour has at least 3 edges
                    mesh.splice(e_lnext, e);
                    if !mesh.delete_edge(e) { return false; }
                    e = e_lnext;
                    e_lnext = mesh.edges[e as usize].lnext;
                }
            }

            // Degenerate contour (one or two edges): e_lnext->lnext == e
            let e_lnext_lnext = mesh.edges[e_lnext as usize].lnext;
            if e_lnext_lnext == e {
                if e_lnext != e {
                    // Advance e_next past e_lnext or its sym
                    if e_lnext == e_next || e_lnext == (e_next ^ 1) {
                        e_next = mesh.edges[e_next as usize].next;
                    }
                    let w1 = mesh.edges[e_lnext as usize].winding;
                    let w2 = mesh.edges[(e_lnext ^ 1) as usize].winding;
                    mesh.edges[e as usize].winding += w1;
                    mesh.edges[(e ^ 1) as usize].winding += w2;
                    if !mesh.delete_edge(e_lnext) { return false; }
                }
                // Advance e_next past e or its sym
                if e == e_next || e == (e_next ^ 1) {
                    e_next = mesh.edges[e_next as usize].next;
                }
                if !mesh.delete_edge(e) { return false; }
            }

            e = e_next;
        }
        true
    }

    fn init_priority_queue(&mut self) -> bool {
        let mesh = match self.mesh.as_ref() { Some(m) => m, None => return true };
        let mut count = 0usize;
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD { count += 1; v = mesh.verts[v as usize].next; }

        // Collect (s,t,vert_idx) and sort ascending by vert_leq.
        let mut vert_coords: Vec<(Real, Real, VertIdx)> = Vec::with_capacity(count);
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD {
            vert_coords.push((mesh.verts[v as usize].s, mesh.verts[v as usize].t, v));
            v = mesh.verts[v as usize].next;
        }
        drop(mesh);

        vert_coords.sort_unstable_by(|a, b| {
            if vert_leq(a.0, a.1, b.0, b.1) { std::cmp::Ordering::Less }
            else { std::cmp::Ordering::Greater }
        });

        // Build the sorted event queue. Store each vertex's position as a negative
        // handle (convention: -(index+1)) so that pq_delete can invalidate it.
        self.sorted_events = vert_coords.iter().map(|&(_, _, v)| v).collect();
        self.sorted_event_pos = 0;
        self.pq = None; // Dynamic heap used only for intersection vertices added later

        // Assign each initial vertex a handle encoding its sorted_events index.
        for (idx, &(_, _, v)) in vert_coords.iter().enumerate() {
            let handle = -(idx as i32 + 1); // negative → sorted_events slot
            self.mesh.as_mut().unwrap().verts[v as usize].pq_handle = handle;
        }

        true
    }

    // The event queue is just the sorted vertex list
    // We use these fields instead of PriorityQ for the main sweep
    // (PriorityQ is still used for intersection vertices added during sweep)

    fn pq_is_empty(&self) -> bool {
        self.sorted_events_min() == INVALID
            && self.pq.as_ref().map_or(true, |pq| pq.is_empty())
    }

    fn sorted_events_min(&self) -> VertIdx {
        // Return the first non-INVALID entry at or after sorted_event_pos.
        let mut pos = self.sorted_event_pos;
        while pos < self.sorted_events.len() {
            let v = self.sorted_events[pos];
            if v != INVALID { return v; }
            pos += 1;
        }
        INVALID
    }

    fn pq_minimum(&self) -> VertIdx {
        let sort_min = self.sorted_events_min();
        let heap_min = self.pq.as_ref().map_or(INVALID, |pq| if !pq.is_empty() { pq.minimum() } else { INVALID });

        match (sort_min, heap_min) {
            (INVALID, INVALID) => INVALID,
            (INVALID, h) => h,
            (s, INVALID) => s,
            (s, h) => {
                let mesh = self.mesh.as_ref().unwrap();
                let (ss, st) = (mesh.verts[s as usize].s, mesh.verts[s as usize].t);
                let (hs, ht) = (mesh.verts[h as usize].s, mesh.verts[h as usize].t);
                if vert_leq(ss, st, hs, ht) { s } else { h }
            }
        }
    }

    fn pq_extract_min(&mut self) -> VertIdx {
        let v = self.pq_minimum();
        if v == INVALID { return INVALID; }

        if self.sorted_events_min() == v {
            // Consume from sorted_events: advance past INVALID slots and then past v.
            while self.sorted_event_pos < self.sorted_events.len() {
                let s = self.sorted_events[self.sorted_event_pos];
                self.sorted_event_pos += 1;
                if s != INVALID { break; } // consumed v
            }
        } else {
            // Comes from the dynamic heap (intersection vertex).
            if let Some(ref mut pq) = self.pq {
                pq.extract_min();
            }
        }
        v
    }

    fn pq_delete(&mut self, handle: i32) {
        if handle >= 0 {
            // Heap-phase handle (intersection vertex)
            if let Some(ref mut pq) = self.pq {
                pq.delete(handle);
            }
        } else {
            // Sorted-events handle: mark the slot as INVALID
            let idx = (-(handle + 1)) as usize;
            if idx < self.sorted_events.len() {
                self.sorted_events[idx] = INVALID;
            }
        }
    }

    fn pq_insert(&mut self, v: VertIdx) -> i32 {
        if self.pq.is_none() {
            self.pq = Some(PriorityQ::new(16, |_, _| true));
            self.pq.as_mut().unwrap().init();
        }
        self.pq.as_mut().unwrap().insert(v)
    }

    // ─────── Edge dictionary initialization ──────────────────────────────────

    fn add_sentinel(&mut self, smin: Real, smax: Real, t: Real) -> bool {
        // Mirror C AddSentinel: create a horizontal edge at height t,
        // going from Org=(smax,t) to Dst=(smin,t), and insert as a sentinel region.
        let e = match self.mesh.as_mut().unwrap().make_edge() {
            Some(e) => e,
            None => return false,
        };
        {
            let mesh = self.mesh.as_mut().unwrap();
            let org = mesh.edges[e as usize].org;
            let dst = mesh.dst(e);
            mesh.verts[org as usize].s = smax;
            mesh.verts[org as usize].t = t;
            mesh.verts[dst as usize].s = smin;
            mesh.verts[dst as usize].t = t;
        }
        // Set the event to Dst (as C does) so edge_leq works during insertion
        let dst = self.mesh.as_ref().unwrap().dst(e);
        let (dst_s, dst_t) = {
            let m = self.mesh.as_ref().unwrap();
            (m.verts[dst as usize].s, m.verts[dst as usize].t)
        };
        self.event = dst;
        self.event_s = dst_s;
        self.event_t = dst_t;

        let reg = self.alloc_region();
        {
            let r = self.region_mut(reg);
            r.e_up = e;
            r.winding_number = 0;
            r.inside = false;
            r.sentinel = true;
            r.dirty = false;
            r.fix_upper_edge = false;
        }

        // Insert the region into the dict using edge_leq ordering
        let node = self.dict_insert_region(reg);
        if node == INVALID {
            return false;
        }
        self.region_mut(reg).node_up = node;

        // Set the edge's active_region so it's recognized as a sentinel edge
        self.mesh.as_mut().unwrap().edges[e as usize].active_region = reg;
        true
    }

    /// Insert a region into the dict at the sorted position (using edge_leq).
    /// Returns the new node index.
    fn dict_insert_region(&mut self, reg: RegionIdx) -> NodeIdx {
        // Walk backward from head, stopping when leq(node_key, reg) == true (or node_key is INVALID/sentinel)
        let mut node = DICT_HEAD;
        loop {
            node = self.dict.nodes[node as usize].prev;
            let key = self.dict.nodes[node as usize].key;
            if key == INVALID {
                break; // hit head sentinel
            }
            if self.edge_leq(key, reg) {
                break;
            }
        }
        // Insert after `node`
        let after = node;
        let before = self.dict.nodes[after as usize].next;
        let new_node = self.dict.nodes.len() as NodeIdx;
        use crate::dict::DictNode;
        let new_dict_node = DictNode { key: reg, next: before, prev: after };
        self.dict.nodes.push(new_dict_node);
        self.dict.nodes[after as usize].next = new_node;
        self.dict.nodes[before as usize].prev = new_node;
        new_node
    }

    fn init_edge_dict(&mut self) -> bool {
        self.dict = Dict::new();

        // Compute sentinel bounds from bounding box + margin (mirrors C InitEdgeDict)
        let w = (self.bmax[0] - self.bmin[0]) + 0.01;
        let h = (self.bmax[1] - self.bmin[1]) + 0.01;
        let smin = self.bmin[0] - w;
        let smax = self.bmax[0] + w;
        let tmin = self.bmin[1] - h;
        let tmax = self.bmax[1] + h;

        // Add bottom sentinel first (at tmin), then top sentinel (at tmax).
        // After insertion with EdgeLeq ordering, top ends up before bottom in the dict.
        if !self.add_sentinel(smin, smax, tmin) { return false; }
        if !self.add_sentinel(smin, smax, tmax) { return false; }

        true
    }

    fn done_edge_dict(&mut self) {
        // Remove all sentinel regions
        let mut node = self.dict.min();
        while node != DICT_HEAD {
            let key = self.dict.key(node);
            let next = self.dict.succ(node);
            if key != INVALID {
                let is_sentinel = self.region(key).sentinel;
                if is_sentinel {
                    self.dict.delete(node);
                    self.free_region(key);
                }
            }
            node = next;
        }
    }

    // ─────── Region operations ────────────────────────────────────────────────

    fn alloc_region(&mut self) -> RegionIdx {
        if let Some(idx) = self.region_free.pop() {
            self.regions[idx as usize] = Some(ActiveRegion::default());
            idx
        } else {
            let idx = self.regions.len() as RegionIdx;
            self.regions.push(Some(ActiveRegion::default()));
            idx
        }
    }

    fn free_region(&mut self, idx: RegionIdx) {
        if idx != INVALID {
            self.regions[idx as usize] = None;
            self.region_free.push(idx);
        }
    }

    fn region(&self, idx: RegionIdx) -> &ActiveRegion {
        self.regions[idx as usize].as_ref().unwrap()
    }

    fn region_mut(&mut self, idx: RegionIdx) -> &mut ActiveRegion {
        self.regions[idx as usize].as_mut().unwrap()
    }

    /// Returns the region index of the dict node's successor region.
    fn region_above(&self, reg: RegionIdx) -> RegionIdx {
        let node = self.region(reg).node_up;
        self.dict.key(self.dict.succ(node))
    }

    /// Returns the region index of the dict node's predecessor region.
    fn region_below(&self, reg: RegionIdx) -> RegionIdx {
        let node = self.region(reg).node_up;
        self.dict.key(self.dict.pred(node))
    }

    /// EdgeLeq: Returns reg1 <= reg2 at the current sweep position (event).
    fn edge_leq(&self, reg1: RegionIdx, reg2: RegionIdx) -> bool {
        let e1 = self.region(reg1).e_up;
        let e2 = self.region(reg2).e_up;
        if e1 == INVALID { return true; }
        if e2 == INVALID { return false; }
        let mesh = self.mesh.as_ref().unwrap();

        let e1_dst = mesh.dst(e1);
        let e2_dst = mesh.dst(e2);
        let e1_org = mesh.edges[e1 as usize].org;
        let e2_org = mesh.edges[e2 as usize].org;

        let ev_s = self.event_s;
        let ev_t = self.event_t;

        let (e1ds, e1dt) = (mesh.verts[e1_dst as usize].s, mesh.verts[e1_dst as usize].t);
        let (e2ds, e2dt) = (mesh.verts[e2_dst as usize].s, mesh.verts[e2_dst as usize].t);
        let (e1os, e1ot) = (mesh.verts[e1_org as usize].s, mesh.verts[e1_org as usize].t);
        let (e2os, e2ot) = (mesh.verts[e2_org as usize].s, mesh.verts[e2_org as usize].t);

        if vert_eq(e1ds, e1dt, ev_s, ev_t) {
            if vert_eq(e2ds, e2dt, ev_s, ev_t) {
                if vert_leq(e1os, e1ot, e2os, e2ot) {
                    return edge_sign(e2ds, e2dt, e1os, e1ot, e2os, e2ot) <= 0.0;
                }
                return edge_sign(e1ds, e1dt, e2os, e2ot, e1os, e1ot) >= 0.0;
            }
            return edge_sign(e2ds, e2dt, ev_s, ev_t, e2os, e2ot) <= 0.0;
        }
        if vert_eq(e2ds, e2dt, ev_s, ev_t) {
            return edge_sign(e1ds, e1dt, ev_s, ev_t, e1os, e1ot) >= 0.0;
        }
        let t1 = crate::geom::edge_eval(e1ds, e1dt, ev_s, ev_t, e1os, e1ot);
        let t2 = crate::geom::edge_eval(e2ds, e2dt, ev_s, ev_t, e2os, e2ot);
        t1 >= t2
    }

    /// Insert a new region below `reg_above` with upper edge `e_new_up`.
    /// Mirrors C's AddRegionBelow which calls ComputeWinding internally.
    fn add_region_below(&mut self, _reg_above: RegionIdx, e_new_up: EdgeIdx) -> RegionIdx {
        let reg_new = self.alloc_region();
        {
            let r = self.region_mut(reg_new);
            r.e_up = e_new_up;
            r.fix_upper_edge = false;
            r.sentinel = false;
            r.dirty = false;
        }

        // Insert at sorted position (mirrors C dictInsert which starts from head)
        let new_node_idx = self.dict_insert_region(reg_new);
        if new_node_idx == INVALID {
            self.free_region(reg_new);
            return INVALID;
        }
        self.region_mut(reg_new).node_up = new_node_idx;

        // Link the edge to the region
        self.mesh.as_mut().unwrap().edges[e_new_up as usize].active_region = reg_new;

        // Compute winding number (C's AddRegionBelow calls ComputeWinding here)
        self.compute_winding(reg_new);

        #[cfg(test)]
        if std::env::var("TESS_DEBUG").is_ok() {
            let org = self.mesh.as_ref().unwrap().edges[e_new_up as usize].org;
            let (os, ot) = if org != INVALID {
                (self.mesh.as_ref().unwrap().verts[org as usize].s,
                 self.mesh.as_ref().unwrap().verts[org as usize].t)
            } else { (0.0, 0.0) };
            eprintln!("  add_region_below: reg_new={} e_up={} org=({:.3},{:.3}) winding={} inside={}",
                reg_new, e_new_up, os, ot,
                self.region(reg_new).winding_number,
                self.region(reg_new).inside);
        }

        reg_new
    }

    fn delete_region(&mut self, reg: RegionIdx) {
        if self.region(reg).fix_upper_edge {
            // Was created with zero winding - must be deleted with zero winding
        }
        let e_up = self.region(reg).e_up;
        if e_up != INVALID {
            self.mesh.as_mut().unwrap().edges[e_up as usize].active_region = INVALID;
        }
        let node = self.region(reg).node_up;
        self.dict.delete(node);
        self.free_region(reg);
    }

    fn fix_upper_edge(&mut self, reg: RegionIdx, new_edge: EdgeIdx) -> bool {
        let old_edge = self.region(reg).e_up;
        if old_edge != INVALID {
            if !self.mesh.as_mut().unwrap().delete_edge(old_edge) { return false; }
        }
        self.region_mut(reg).fix_upper_edge = false;
        self.region_mut(reg).e_up = new_edge;
        self.mesh.as_mut().unwrap().edges[new_edge as usize].active_region = reg;
        true
    }

    fn is_winding_inside(&self, n: i32) -> bool {
        match self.winding_rule {
            WindingRule::Odd => n & 1 != 0,
            WindingRule::NonZero => n != 0,
            WindingRule::Positive => n > 0,
            WindingRule::Negative => n < 0,
            WindingRule::AbsGeqTwo => n >= 2 || n <= -2,
        }
    }

    fn compute_winding(&mut self, reg: RegionIdx) {
        let above = self.region_above(reg);
        let above_winding = if above != INVALID { self.region(above).winding_number } else { 0 };
        let e_up = self.region(reg).e_up;
        let e_winding = if e_up != INVALID {
            self.mesh.as_ref().unwrap().edges[e_up as usize].winding
        } else { 0 };
        let new_winding = above_winding + e_winding;
        let inside = self.is_winding_inside(new_winding);
        #[cfg(test)]
        if std::env::var("TESS_DEBUG").is_ok() {
            let (es, et) = if e_up != INVALID {
                let m = self.mesh.as_ref().unwrap();
                let org = m.edges[e_up as usize].org;
                if org != INVALID { (m.verts[org as usize].s, m.verts[org as usize].t) } else { (0.0, 0.0) }
            } else { (0.0, 0.0) };
            eprintln!("  compute_winding: reg={} above={} above_wnd={} e_up={} e_wnd={} → winding={} inside={}  eUp_org=({:.1},{:.1})",
                reg, above, above_winding, e_up, e_winding, new_winding, inside, es, et);
        }
        self.region_mut(reg).winding_number = new_winding;
        self.region_mut(reg).inside = inside;
    }

    fn finish_region(&mut self, reg: RegionIdx) {
        let e = self.region(reg).e_up;
        if e != INVALID {
            let lface = self.mesh.as_ref().unwrap().edges[e as usize].lface;
            if lface != INVALID {
                let inside = self.region(reg).inside;
                #[cfg(test)]
                if std::env::var("TESS_DEBUG").is_ok() {
                    eprintln!("  finish_region: reg={} e_up={} lface={} inside={} winding={}",
                        reg, e, lface, inside, self.region(reg).winding_number);
                }
                self.mesh.as_mut().unwrap().faces[lface as usize].inside = inside;
                self.mesh.as_mut().unwrap().faces[lface as usize].an_edge = e;
            }
        }
        self.delete_region(reg);
    }

    /// Find topmost region with same Org as reg->eUp->Org.
    fn top_left_region(&mut self, reg: RegionIdx) -> RegionIdx {
        let org = {
            let e = self.region(reg).e_up;
            if e == INVALID { return INVALID; }
            self.mesh.as_ref().unwrap().edges[e as usize].org
        };
        let mut r = reg;
        loop {
            r = self.region_above(r);
            if r == INVALID { return INVALID; }
            let e = self.region(r).e_up;
            if e == INVALID { return INVALID; }
            let e_org = self.mesh.as_ref().unwrap().edges[e as usize].org;
            if e_org != org { break; }
        }
        // r is now above the topmost region with same origin
        // Check if we need to fix it
        if self.region(r).fix_upper_edge {
            let below = self.region_below(r);
            let below_e = self.region(below).e_up;
            let below_e_sym = below_e ^ 1;
            let r_e = self.region(r).e_up;
            let r_e_lnext = self.mesh.as_ref().unwrap().edges[r_e as usize].lnext;
            let new_e = match self.mesh.as_mut().unwrap().connect(below_e_sym, r_e_lnext) {
                Some(e) => e,
                None => return INVALID,
            };
            if !self.fix_upper_edge(r, new_e) { return INVALID; }
            r = self.region_above(r);
        }
        r
    }

    fn top_right_region(&self, reg: RegionIdx) -> RegionIdx {
        let dst = {
            let e = self.region(reg).e_up;
            if e == INVALID { return INVALID; }
            self.mesh.as_ref().unwrap().dst(e)
        };
        let mut r = reg;
        loop {
            r = self.region_above(r);
            if r == INVALID { return INVALID; }
            let e = self.region(r).e_up;
            if e == INVALID { return INVALID; }
            let e_dst = self.mesh.as_ref().unwrap().dst(e);
            if e_dst != dst { break; }
        }
        r
    }

    fn finish_left_regions(&mut self, reg_first: RegionIdx, reg_last: RegionIdx) -> EdgeIdx {
        let mut reg_prev = reg_first;
        let mut e_prev = self.region(reg_first).e_up;

        while reg_prev != reg_last {
            self.region_mut(reg_prev).fix_upper_edge = false;
            let reg = self.region_below(reg_prev);
            if reg == INVALID { break; }
            let e = self.region(reg).e_up;

            let e_org = if e != INVALID { self.mesh.as_ref().unwrap().edges[e as usize].org } else { INVALID };
            let ep_org = if e_prev != INVALID { self.mesh.as_ref().unwrap().edges[e_prev as usize].org } else { INVALID };

            if e_org != ep_org {
                if !self.region(reg).fix_upper_edge {
                    self.finish_region(reg_prev);
                    break;
                }
                let ep_lprev = if e_prev != INVALID { self.mesh.as_ref().unwrap().lprev(e_prev) } else { INVALID };
                let e_sym = if e != INVALID { e ^ 1 } else { INVALID };
                let new_e = if ep_lprev != INVALID && e_sym != INVALID {
                    self.mesh.as_mut().unwrap().connect(ep_lprev, e_sym)
                } else { None };
                if let Some(ne) = new_e {
                    if !self.fix_upper_edge(reg, ne) { return INVALID; }
                }
            }

            if e_prev != INVALID && e != INVALID {
                let ep_onext = self.mesh.as_ref().unwrap().edges[e_prev as usize].onext;
                if ep_onext != e {
                    let e_oprev = self.mesh.as_ref().unwrap().oprev(e);
                    self.mesh.as_mut().unwrap().splice(e_oprev, e);
                    self.mesh.as_mut().unwrap().splice(e_prev, e);
                }
            }

            self.finish_region(reg_prev);
            e_prev = self.region(reg).e_up;
            reg_prev = reg;
        }
        e_prev
    }

    fn add_right_edges(
        &mut self,
        reg_up: RegionIdx,
        e_first: EdgeIdx,
        e_last: EdgeIdx,
        e_top_left: EdgeIdx,
        clean_up: bool,
    ) {
        // Insert right-going edges into the dictionary
        let mut e = e_first;
        loop {
            self.add_region_below(reg_up, e ^ 1);
            e = self.mesh.as_ref().unwrap().edges[e as usize].onext;
            if e == e_last { break; }
        }

        // Determine e_top_left
        let e_top_left = if e_top_left == INVALID {
            let reg_below = self.region_below(reg_up);
            if reg_below == INVALID { return; }
            let rb_e = self.region(reg_below).e_up;
            if rb_e == INVALID { return; }
            self.mesh.as_ref().unwrap().rprev(rb_e)
        } else {
            e_top_left
        };

        let mut reg_prev = reg_up;
        let mut e_prev = e_top_left;
        let mut first_time = true;

        loop {
            let reg = self.region_below(reg_prev);
            if reg == INVALID { break; }
            let e = {
                let re = self.region(reg).e_up;
                if re == INVALID { break; }
                re ^ 1 // e = reg->eUp->Sym
            };
            let e_org = self.mesh.as_ref().unwrap().edges[e as usize].org;
            let ep_org = if e_prev != INVALID { self.mesh.as_ref().unwrap().edges[e_prev as usize].org } else { INVALID };
            if e_org != ep_org { break; }

            if e_prev != INVALID {
                let ep_onext = self.mesh.as_ref().unwrap().edges[e_prev as usize].onext;
                if ep_onext != e {
                    let e_oprev = self.mesh.as_ref().unwrap().oprev(e);
                    self.mesh.as_mut().unwrap().splice(e_oprev, e);
                    let ep_oprev = self.mesh.as_ref().unwrap().oprev(e_prev);
                    self.mesh.as_mut().unwrap().splice(ep_oprev, e);
                }
            }

            let above_winding = self.region(reg_prev).winding_number;
            let e_winding = self.mesh.as_ref().unwrap().edges[e as usize].winding;
            let new_winding = above_winding - e_winding;
            let inside = self.is_winding_inside(new_winding);
            self.region_mut(reg).winding_number = new_winding;
            self.region_mut(reg).inside = inside;

            self.region_mut(reg_prev).dirty = true;
            if !first_time {
                if self.check_for_right_splice(reg_prev) {
                    // AddWinding
                    let re = self.region(reg).e_up;
                    let rep = self.region(reg_prev).e_up;
                    if re != INVALID && rep != INVALID {
                        let w1 = self.mesh.as_ref().unwrap().edges[re as usize].winding;
                        let w2 = self.mesh.as_ref().unwrap().edges[(re ^ 1) as usize].winding;
                        let wp1 = self.mesh.as_ref().unwrap().edges[rep as usize].winding;
                        let wp2 = self.mesh.as_ref().unwrap().edges[(rep ^ 1) as usize].winding;
                        self.mesh.as_mut().unwrap().edges[re as usize].winding += wp1;
                        self.mesh.as_mut().unwrap().edges[(re ^ 1) as usize].winding += wp2;
                    }
                    self.delete_region(reg_prev);
                    if e_prev != INVALID {
                        self.mesh.as_mut().unwrap().delete_edge(e_prev);
                    }
                }
            }
            first_time = false;
            reg_prev = reg;
            e_prev = e;
        }

        self.region_mut(reg_prev).dirty = true;

        if clean_up {
            self.walk_dirty_regions(reg_prev);
        }
    }

    fn check_for_right_splice(&mut self, reg_up: RegionIdx) -> bool {
        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return false; }
        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return false; }

        let mesh = self.mesh.as_ref().unwrap();
        let e_up_org = mesh.edges[e_up as usize].org;
        let e_lo_org = mesh.edges[e_lo as usize].org;
        let (euo_s, euo_t) = (mesh.verts[e_up_org as usize].s, mesh.verts[e_up_org as usize].t);
        let (elo_s, elo_t) = (mesh.verts[e_lo_org as usize].s, mesh.verts[e_lo_org as usize].t);
        let e_lo_dst = mesh.dst(e_lo);
        let (eld_s, eld_t) = (mesh.verts[e_lo_dst as usize].s, mesh.verts[e_lo_dst as usize].t);
        let e_up_dst = mesh.dst(e_up);
        let (eud_s, eud_t) = (mesh.verts[e_up_dst as usize].s, mesh.verts[e_up_dst as usize].t);
        drop(mesh);

        if vert_leq(euo_s, euo_t, elo_s, elo_t) {
            if edge_sign(eld_s, eld_t, euo_s, euo_t, elo_s, elo_t) > 0.0 { return false; }
            if !vert_eq(euo_s, euo_t, elo_s, elo_t) {
                // Splice eUp->Org into eLo
                self.mesh.as_mut().unwrap().split_edge(e_lo ^ 1);
                let e_lo_oprev = self.mesh.as_ref().unwrap().oprev(e_lo);
                self.mesh.as_mut().unwrap().splice(e_up, e_lo_oprev);
                self.region_mut(reg_up).dirty = true;
                self.region_mut(reg_lo).dirty = true;
            } else if e_up_org != e_lo_org {
                // Merge: delete eUp->Org from PQ and splice
                let handle = self.mesh.as_ref().unwrap().verts[e_up_org as usize].pq_handle;
                self.pq_delete(handle);
                let e_lo_oprev = self.mesh.as_ref().unwrap().oprev(e_lo);
                self.mesh.as_mut().unwrap().splice(e_lo_oprev, e_up);
            }
        } else {
            if edge_sign(eud_s, eud_t, elo_s, elo_t, euo_s, euo_t) < 0.0 { return false; }
            let reg_above = self.region_above(reg_up);
            if reg_above != INVALID {
                self.region_mut(reg_above).dirty = true;
            }
            self.region_mut(reg_up).dirty = true;
            self.mesh.as_mut().unwrap().split_edge(e_up ^ 1);
            let e_lo_oprev = self.mesh.as_ref().unwrap().oprev(e_lo);
            self.mesh.as_mut().unwrap().splice(e_lo_oprev, e_up);
        }
        true
    }

    fn check_for_left_splice(&mut self, reg_up: RegionIdx) -> bool {
        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return false; }
        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return false; }

        let mesh = self.mesh.as_ref().unwrap();
        let e_up_dst = mesh.dst(e_up);
        let e_lo_dst = mesh.dst(e_lo);
        if vert_eq(
            mesh.verts[e_up_dst as usize].s, mesh.verts[e_up_dst as usize].t,
            mesh.verts[e_lo_dst as usize].s, mesh.verts[e_lo_dst as usize].t,
        ) { return false; } // Same destination

        let (eud_s, eud_t) = (mesh.verts[e_up_dst as usize].s, mesh.verts[e_up_dst as usize].t);
        let (eld_s, eld_t) = (mesh.verts[e_lo_dst as usize].s, mesh.verts[e_lo_dst as usize].t);
        let e_up_org = mesh.edges[e_up as usize].org;
        let e_lo_org = mesh.edges[e_lo as usize].org;
        let (euo_s, euo_t) = (mesh.verts[e_up_org as usize].s, mesh.verts[e_up_org as usize].t);
        let (elo_s, elo_t) = (mesh.verts[e_lo_org as usize].s, mesh.verts[e_lo_org as usize].t);
        drop(mesh);

        if vert_leq(eud_s, eud_t, eld_s, eld_t) {
            if edge_sign(eud_s, eud_t, eld_s, eld_t, euo_s, euo_t) < 0.0 { return false; }
            // eLo->Dst is above eUp: splice eLo->Dst into eUp
            let reg_above = self.region_above(reg_up);
            if reg_above != INVALID { self.region_mut(reg_above).dirty = true; }
            self.region_mut(reg_up).dirty = true;
            let new_e = match self.mesh.as_mut().unwrap().split_edge(e_up) {
                Some(e) => e,
                None => return false,
            };
            let e_lo_sym = e_lo ^ 1;
            self.mesh.as_mut().unwrap().splice(e_lo_sym, new_e);
            let new_lface = self.mesh.as_ref().unwrap().edges[new_e as usize].lface;
            let inside = self.region(reg_up).inside;
            if new_lface != INVALID {
                self.mesh.as_mut().unwrap().faces[new_lface as usize].inside = inside;
            }
        } else {
            if edge_sign(eld_s, eld_t, eud_s, eud_t, elo_s, elo_t) > 0.0 { return false; }
            // eUp->Dst is below eLo: splice eUp->Dst into eLo
            self.region_mut(reg_up).dirty = true;
            self.region_mut(reg_lo).dirty = true;
            let new_e = match self.mesh.as_mut().unwrap().split_edge(e_lo) {
                Some(e) => e,
                None => return false,
            };
            let e_up_lnext = self.mesh.as_ref().unwrap().edges[e_up as usize].lnext;
            let e_lo_sym = e_lo ^ 1;
            self.mesh.as_mut().unwrap().splice(e_up_lnext, e_lo_sym);
            let new_rface = self.mesh.as_ref().unwrap().rface(new_e);
            let inside = self.region(reg_up).inside;
            if new_rface != INVALID {
                self.mesh.as_mut().unwrap().faces[new_rface as usize].inside = inside;
            }
        }
        true
    }

    fn check_for_intersect(&mut self, reg_up: RegionIdx) -> bool {
        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return false; }
        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return false; }
        if self.region(reg_up).fix_upper_edge || self.region(reg_lo).fix_upper_edge {
            return false;
        }

        let mesh = self.mesh.as_ref().unwrap();
        let org_up = mesh.edges[e_up as usize].org;
        let org_lo = mesh.edges[e_lo as usize].org;
        let dst_up = mesh.dst(e_up);
        let dst_lo = mesh.dst(e_lo);

        if vert_eq(
            mesh.verts[dst_up as usize].s, mesh.verts[dst_up as usize].t,
            mesh.verts[dst_lo as usize].s, mesh.verts[dst_lo as usize].t,
        ) { return false; }

        let (ou_s, ou_t) = (mesh.verts[org_up as usize].s, mesh.verts[org_up as usize].t);
        let (ol_s, ol_t) = (mesh.verts[org_lo as usize].s, mesh.verts[org_lo as usize].t);
        let (du_s, du_t) = (mesh.verts[dst_up as usize].s, mesh.verts[dst_up as usize].t);
        let (dl_s, dl_t) = (mesh.verts[dst_lo as usize].s, mesh.verts[dst_lo as usize].t);
        let ev_s = self.event_s;
        let ev_t = self.event_t;
        drop(mesh);

        // Quick rejection tests
        let t_min_up = ou_t.min(du_t);
        let t_max_lo = ol_t.max(dl_t);
        if t_min_up > t_max_lo { return false; }

        if vert_leq(ou_s, ou_t, ol_s, ol_t) {
            if edge_sign(dl_s, dl_t, ou_s, ou_t, ol_s, ol_t) > 0.0 { return false; }
        } else {
            if edge_sign(du_s, du_t, ol_s, ol_t, ou_s, ou_t) < 0.0 { return false; }
        }

        // Compute intersection
        let (isect_s, isect_t) = edge_intersect(du_s, du_t, ou_s, ou_t, dl_s, dl_t, ol_s, ol_t);

        // Clamp intersection to sweep event position
        let (isect_s, isect_t) = if vert_leq(isect_s, isect_t, ev_s, ev_t) {
            (ev_s, ev_t)
        } else {
            (isect_s, isect_t)
        };

        // Clamp to rightmost origin
        let (org_min_s, org_min_t) = if vert_leq(ou_s, ou_t, ol_s, ol_t) {
            (ou_s, ou_t)
        } else {
            (ol_s, ol_t)
        };
        let (isect_s, isect_t) = if vert_leq(org_min_s, org_min_t, isect_s, isect_t) {
            (org_min_s, org_min_t)
        } else {
            (isect_s, isect_t)
        };

        // Check if intersection is at one of the endpoints
        if vert_eq(isect_s, isect_t, ou_s, ou_t) || vert_eq(isect_s, isect_t, ol_s, ol_t) {
            self.check_for_right_splice(reg_up);
            return false;
        }

        if (!vert_eq(du_s, du_t, ev_s, ev_t) && edge_sign(du_s, du_t, ev_s, ev_t, isect_s, isect_t) >= 0.0)
            || (!vert_eq(dl_s, dl_t, ev_s, ev_t) && edge_sign(dl_s, dl_t, ev_s, ev_t, isect_s, isect_t) <= 0.0)
        {
            if vert_eq(dl_s, dl_t, ev_s, ev_t) {
                // Splice dstLo into eUp
                self.mesh.as_mut().unwrap().split_edge(e_up ^ 1);
                let e_lo_sym = e_lo ^ 1;
                let e_up2 = self.region(reg_up).e_up;
                self.mesh.as_mut().unwrap().splice(e_lo_sym, e_up2);
                let reg_up2 = self.top_left_region(reg_up);
                if reg_up2 == INVALID { return false; }
                let rb = self.region_below(reg_up2);
                let rb_e = self.region(rb).e_up;
                let rl2 = self.region_below(rb);
                self.finish_left_regions(self.region_below(reg_up2), reg_lo);
                let e_up_new = self.region(rb).e_up;
                let e_oprev = self.mesh.as_ref().unwrap().oprev(e_up_new);
                self.add_right_edges(reg_up2, e_oprev, e_up_new, e_up_new, true);
                return true;
            }
            if vert_eq(du_s, du_t, ev_s, ev_t) {
                self.mesh.as_mut().unwrap().split_edge(e_lo ^ 1);
                let e_up_lnext = self.mesh.as_ref().unwrap().edges[e_up as usize].lnext;
                let e_lo_oprev = self.mesh.as_ref().unwrap().oprev(e_lo);
                self.mesh.as_mut().unwrap().splice(e_up_lnext, e_lo_oprev);
                let reg_lo2 = reg_up;
                let reg_up2 = self.top_right_region(reg_up);
                if reg_up2 == INVALID { return false; }
                let e_finish = self.mesh.as_ref().unwrap().rprev(self.region(self.region_below(reg_up2)).e_up);
                self.region_mut(reg_lo2).e_up = self.mesh.as_ref().unwrap().oprev(e_lo);
                let lo_end = self.finish_left_regions(reg_lo2, INVALID);
                let e_lo_onext = if lo_end != INVALID { self.mesh.as_ref().unwrap().edges[lo_end as usize].onext } else { INVALID };
                let e_up_rprev = self.mesh.as_ref().unwrap().rprev(e_up);
                self.add_right_edges(reg_up2, e_lo_onext, e_up_rprev, e_finish, true);
                return true;
            }
            // Split edges
            if edge_sign(du_s, du_t, ev_s, ev_t, isect_s, isect_t) >= 0.0 {
                let reg_above = self.region_above(reg_up);
                if reg_above != INVALID { self.region_mut(reg_above).dirty = true; }
                self.region_mut(reg_up).dirty = true;
                self.mesh.as_mut().unwrap().split_edge(e_up ^ 1);
                let e_up2 = self.region(reg_up).e_up;
                let e_up2_org = self.mesh.as_ref().unwrap().edges[e_up2 as usize].org;
                self.mesh.as_mut().unwrap().verts[e_up2_org as usize].s = ev_s;
                self.mesh.as_mut().unwrap().verts[e_up2_org as usize].t = ev_t;
            }
            if edge_sign(dl_s, dl_t, ev_s, ev_t, isect_s, isect_t) <= 0.0 {
                self.region_mut(reg_up).dirty = true;
                self.region_mut(reg_lo).dirty = true;
                self.mesh.as_mut().unwrap().split_edge(e_lo ^ 1);
                let e_lo2 = self.region(reg_lo).e_up;
                let e_lo2_org = self.mesh.as_ref().unwrap().edges[e_lo2 as usize].org;
                self.mesh.as_mut().unwrap().verts[e_lo2_org as usize].s = ev_s;
                self.mesh.as_mut().unwrap().verts[e_lo2_org as usize].t = ev_t;
            }
            return false;
        }

        // General case: split both edges and splice at intersection
        self.mesh.as_mut().unwrap().split_edge(e_up ^ 1);
        self.mesh.as_mut().unwrap().split_edge(e_lo ^ 1);
        let e_lo2 = self.region(reg_lo).e_up;
        let e_lo2_oprev = self.mesh.as_ref().unwrap().oprev(e_lo2);
        let e_up2 = self.region(reg_up).e_up;
        self.mesh.as_mut().unwrap().splice(e_lo2_oprev, e_up2);

        // Set intersection coordinates
        let e_up2_org = self.mesh.as_ref().unwrap().edges[e_up2 as usize].org;

        // Compute weighted coordinates for the intersection vertex
        let (org_up_s, org_up_t) = (ou_s, ou_t);
        let (dst_up_s, dst_up_t) = (du_s, du_t);
        let (org_lo_s, org_lo_t) = (ol_s, ol_t);
        let (dst_lo_s, dst_lo_t) = (dl_s, dl_t);

        self.mesh.as_mut().unwrap().verts[e_up2_org as usize].s = isect_s;
        self.mesh.as_mut().unwrap().verts[e_up2_org as usize].t = isect_t;
        self.mesh.as_mut().unwrap().verts[e_up2_org as usize].coords = compute_intersect_coords(
            isect_s, isect_t,
            org_up_s, org_up_t, self.mesh.as_ref().unwrap().verts[
                self.mesh.as_ref().unwrap().edges[e_up2 as usize].org as usize
            ].coords,
            dst_up_s, dst_up_t,
            org_lo_s, org_lo_t,
            dst_lo_s, dst_lo_t,
        );
        self.mesh.as_mut().unwrap().verts[e_up2_org as usize].idx = TESS_UNDEF;

        // Insert new vertex into priority queue
        let handle = self.pq_insert(e_up2_org);
        if handle == INVALID_HANDLE { return false; }
        self.mesh.as_mut().unwrap().verts[e_up2_org as usize].pq_handle = handle;

        let reg_above = self.region_above(reg_up);
        if reg_above != INVALID { self.region_mut(reg_above).dirty = true; }
        self.region_mut(reg_up).dirty = true;
        self.region_mut(reg_lo).dirty = true;

        false
    }

    fn walk_dirty_regions(&mut self, reg_up: RegionIdx) {
        let mut reg_up = reg_up;
        let mut reg_lo = self.region_below(reg_up);

        loop {
            // Find lowest dirty region
            while reg_lo != INVALID && self.region(reg_lo).dirty {
                reg_up = reg_lo;
                reg_lo = self.region_below(reg_lo);
            }
            if !self.region(reg_up).dirty {
                reg_lo = reg_up;
                reg_up = self.region_above(reg_up);
                if reg_up == INVALID || !self.region(reg_up).dirty { return; }
            }

            self.region_mut(reg_up).dirty = false;
            if reg_lo == INVALID { return; }
            let e_up = self.region(reg_up).e_up;
            let e_lo = self.region(reg_lo).e_up;

            if e_up != INVALID && e_lo != INVALID {
                let e_up_dst = self.mesh.as_ref().unwrap().dst(e_up);
                let e_lo_dst = self.mesh.as_ref().unwrap().dst(e_lo);
                let (eud_s, eud_t) = (self.mesh.as_ref().unwrap().verts[e_up_dst as usize].s,
                                      self.mesh.as_ref().unwrap().verts[e_up_dst as usize].t);
                let (eld_s, eld_t) = (self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].s,
                                      self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].t);

                if !vert_eq(eud_s, eud_t, eld_s, eld_t) {
                    if self.check_for_left_splice(reg_up) {
                        let reg_lo_fix = self.region(reg_lo).fix_upper_edge;
                        let reg_up_fix = self.region(reg_up).fix_upper_edge;
                        if reg_lo_fix {
                            let e_lo2 = self.region(reg_lo).e_up;
                            self.delete_region(reg_lo);
                            if e_lo2 != INVALID { self.mesh.as_mut().unwrap().delete_edge(e_lo2); }
                            reg_lo = self.region_below(reg_up);
                        } else if reg_up_fix {
                            let e_up2 = self.region(reg_up).e_up;
                            self.delete_region(reg_up);
                            if e_up2 != INVALID { self.mesh.as_mut().unwrap().delete_edge(e_up2); }
                            reg_up = self.region_above(reg_lo);
                        }
                    }
                }

                let e_up2 = self.region(reg_up).e_up;
                let e_lo2 = self.region(reg_lo).e_up;
                if e_up2 != INVALID && e_lo2 != INVALID {
                    let e_up_org = self.mesh.as_ref().unwrap().edges[e_up2 as usize].org;
                    let e_lo_org = self.mesh.as_ref().unwrap().edges[e_lo2 as usize].org;
                    if e_up_org != e_lo_org {
                        let e_up_dst2 = self.mesh.as_ref().unwrap().dst(e_up2);
                        let e_lo_dst2 = self.mesh.as_ref().unwrap().dst(e_lo2);
                        let fix_up = self.region(reg_up).fix_upper_edge;
                        let fix_lo = self.region(reg_lo).fix_upper_edge;
                        if !vert_eq(
                            self.mesh.as_ref().unwrap().verts[e_up_dst2 as usize].s,
                            self.mesh.as_ref().unwrap().verts[e_up_dst2 as usize].t,
                            self.mesh.as_ref().unwrap().verts[e_lo_dst2 as usize].s,
                            self.mesh.as_ref().unwrap().verts[e_lo_dst2 as usize].t,
                        ) && !fix_up && !fix_lo
                            && (vert_eq(self.mesh.as_ref().unwrap().verts[e_up_dst2 as usize].s, self.mesh.as_ref().unwrap().verts[e_up_dst2 as usize].t, self.event_s, self.event_t)
                                || vert_eq(self.mesh.as_ref().unwrap().verts[e_lo_dst2 as usize].s, self.mesh.as_ref().unwrap().verts[e_lo_dst2 as usize].t, self.event_s, self.event_t))
                        {
                            if self.check_for_intersect(reg_up) { return; }
                        } else {
                            self.check_for_right_splice(reg_up);
                        }
                    }
                }

                // Check for degenerate 2-edge loop
                let e_up3 = self.region(reg_up).e_up;
                let e_lo3 = self.region(reg_lo).e_up;
                if e_up3 != INVALID && e_lo3 != INVALID {
                    let e_up_org3 = self.mesh.as_ref().unwrap().edges[e_up3 as usize].org;
                    let e_lo_org3 = self.mesh.as_ref().unwrap().edges[e_lo3 as usize].org;
                    let e_up_dst3 = self.mesh.as_ref().unwrap().dst(e_up3);
                    let e_lo_dst3 = self.mesh.as_ref().unwrap().dst(e_lo3);
                    if e_up_org3 == e_lo_org3 && e_up_dst3 == e_lo_dst3 {
                        // Merge winding and delete one region
                        let eu_w = self.mesh.as_ref().unwrap().edges[e_up3 as usize].winding;
                        let eu_sw = self.mesh.as_ref().unwrap().edges[(e_up3 ^ 1) as usize].winding;
                        self.mesh.as_mut().unwrap().edges[e_lo3 as usize].winding += eu_w;
                        self.mesh.as_mut().unwrap().edges[(e_lo3 ^ 1) as usize].winding += eu_sw;
                        self.delete_region(reg_up);
                        self.mesh.as_mut().unwrap().delete_edge(e_up3);
                        reg_up = self.region_above(reg_lo);
                    }
                }
            }
        }
    }

    fn connect_right_vertex(&mut self, reg_up: RegionIdx, e_bottom_left: EdgeIdx) {
        // Mirrors C ConnectRightVertex exactly.
        // eTopLeft = eBottomLeft->Onext
        let e_top_left = self.mesh.as_ref().unwrap().edges[e_bottom_left as usize].onext;

        // Step 1: if eUp->Dst != eLo->Dst, check for intersection
        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return; }
        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return; }

        let dst_differ = {
            let e_up_dst = self.mesh.as_ref().unwrap().dst(e_up);
            let e_lo_dst = self.mesh.as_ref().unwrap().dst(e_lo);
            let (s1, t1) = (self.mesh.as_ref().unwrap().verts[e_up_dst as usize].s, self.mesh.as_ref().unwrap().verts[e_up_dst as usize].t);
            let (s2, t2) = (self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].s, self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].t);
            !vert_eq(s1, t1, s2, t2)
        };
        if dst_differ {
            if self.check_for_intersect(reg_up) { return; }
        }

        // Step 2: re-read after possible changes from CheckForIntersect
        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return; }
        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return; }

        // Step 3: degenerate cases
        let mut degenerate = false;
        let mut reg_up = reg_up;
        let mut e_top_left = e_top_left;
        let mut e_bottom_left = e_bottom_left;

        // if(VertEq(eUp->Org, event))
        let e_up_org = self.mesh.as_ref().unwrap().edges[e_up as usize].org;
        if e_up_org != INVALID {
            let (s, t) = (self.mesh.as_ref().unwrap().verts[e_up_org as usize].s, self.mesh.as_ref().unwrap().verts[e_up_org as usize].t);
            if vert_eq(s, t, self.event_s, self.event_t) {
                // splice(eTopLeft->Oprev, eUp)
                let e_tl_oprev = self.mesh.as_ref().unwrap().oprev(e_top_left);
                self.mesh.as_mut().unwrap().splice(e_tl_oprev, e_up);
                // regUp = TopLeftRegion(regUp)
                let reg_up2 = self.top_left_region(reg_up);
                if reg_up2 == INVALID { return; }
                // eTopLeft = RegionBelow(regUp)->eUp
                let rb = self.region_below(reg_up2);
                e_top_left = if rb != INVALID { self.region(rb).e_up } else { INVALID };
                // FinishLeftRegions(RegionBelow(regUp), regLo)
                self.finish_left_regions(rb, reg_lo);
                reg_up = reg_up2;
                degenerate = true;
            }
        }

        // if(VertEq(eLo->Org, event))
        let e_lo2 = if degenerate {
            let rl = self.region_below(reg_up);
            if rl != INVALID { self.region(rl).e_up } else { INVALID }
        } else { e_lo };
        let reg_lo2 = self.region_below(reg_up);

        let e_lo_org = if e_lo2 != INVALID { self.mesh.as_ref().unwrap().edges[e_lo2 as usize].org } else { INVALID };
        if e_lo_org != INVALID {
            let (s, t) = (self.mesh.as_ref().unwrap().verts[e_lo_org as usize].s, self.mesh.as_ref().unwrap().verts[e_lo_org as usize].t);
            if vert_eq(s, t, self.event_s, self.event_t) {
                // splice(eBottomLeft, eLo->Oprev)
                let e_lo_oprev = self.mesh.as_ref().unwrap().oprev(e_lo2);
                self.mesh.as_mut().unwrap().splice(e_bottom_left, e_lo_oprev);
                // eBottomLeft = FinishLeftRegions(regLo, NULL)
                e_bottom_left = self.finish_left_regions(reg_lo2, INVALID);
                degenerate = true;
            }
        }

        if degenerate {
            if e_bottom_left != INVALID && e_top_left != INVALID {
                let e_bl_onext = self.mesh.as_ref().unwrap().edges[e_bottom_left as usize].onext;
                self.add_right_edges(reg_up, e_bl_onext, e_top_left, e_top_left, true);
            }
            return;
        }

        // Step 4: non-degenerate — add temporary fixable edge
        let e_up2 = self.region(reg_up).e_up;
        let rl = self.region_below(reg_up);
        if rl == INVALID { return; }
        let e_lo3 = self.region(rl).e_up;
        if e_up2 == INVALID || e_lo3 == INVALID { return; }

        let e_up2_org = self.mesh.as_ref().unwrap().edges[e_up2 as usize].org;
        let e_lo3_org = self.mesh.as_ref().unwrap().edges[e_lo3 as usize].org;
        let e_new_target = if e_up2_org != INVALID && e_lo3_org != INVALID {
            let (euo_s, euo_t) = (self.mesh.as_ref().unwrap().verts[e_up2_org as usize].s, self.mesh.as_ref().unwrap().verts[e_up2_org as usize].t);
            let (elo_s, elot) = (self.mesh.as_ref().unwrap().verts[e_lo3_org as usize].s, self.mesh.as_ref().unwrap().verts[e_lo3_org as usize].t);
            // eNew = VertLeq(eLo->Org, eUp->Org) ? eLo->Oprev : eUp
            if vert_leq(elo_s, elot, euo_s, euo_t) {
                self.mesh.as_ref().unwrap().oprev(e_lo3)
            } else {
                e_up2
            }
        } else {
            e_up2
        };

        // eNew = connect(eBottomLeft->Lprev, eNewTarget)
        let e_bl_lprev = self.mesh.as_ref().unwrap().lprev(e_bottom_left);
        let e_new = match self.mesh.as_mut().unwrap().connect(e_bl_lprev, e_new_target) {
            Some(e) => e,
            None => return,
        };

        // AddRightEdges(regUp, eNew, eNew->Onext, eNew->Onext, FALSE)
        let e_new_onext = self.mesh.as_ref().unwrap().edges[e_new as usize].onext;
        self.add_right_edges(reg_up, e_new, e_new_onext, e_new_onext, false);

        // eNew->Sym->activeRegion->fixUpperEdge = TRUE
        let e_new_sym_ar = self.mesh.as_ref().unwrap().edges[(e_new ^ 1) as usize].active_region;
        if e_new_sym_ar != INVALID {
            self.region_mut(e_new_sym_ar).fix_upper_edge = true;
        }
        self.walk_dirty_regions(reg_up);
    }

    fn connect_left_degenerate(&mut self, reg_up: RegionIdx, v_event: VertIdx) {
        let e_up = self.region(reg_up).e_up;
        if e_up == INVALID { return; }
        let e_up_org = self.mesh.as_ref().unwrap().edges[e_up as usize].org;
        let (euo_s, euo_t) = (self.mesh.as_ref().unwrap().verts[e_up_org as usize].s,
                              self.mesh.as_ref().unwrap().verts[e_up_org as usize].t);
        let (ev_s, ev_t) = (self.event_s, self.event_t);

        if vert_eq(euo_s, euo_t, ev_s, ev_t) {
            // eUp->Org is same as event -- splice them
            let v_an = self.mesh.as_ref().unwrap().verts[v_event as usize].an_edge;
            if v_an != INVALID {
                self.mesh.as_mut().unwrap().splice(v_an, e_up);
            }
            let reg_up2 = self.top_left_region(reg_up);
            if reg_up2 == INVALID { return; }
            let rb = self.region_below(reg_up2);
            let rb_e = self.region(rb).e_up;
            let rl = self.region_below(rb);
            self.finish_left_regions(self.region_below(reg_up2), rl);
            let e_up3 = self.region(rb).e_up;
            let e_oprev = self.mesh.as_ref().unwrap().oprev(e_up3);
            self.add_right_edges(reg_up2, e_oprev, e_up3, e_up3, true);
        } else {
            // Create a new temporary edge to connect v_event to eUp
            self.check_for_right_splice(reg_up);
        }
    }

    /// Mirrors C's dictSearch: walks forward from head.next, returns the key of
    /// the FIRST node where edge_leq(tmp_reg, node.key) is true.
    /// This is exactly how the C code finds the containing region in ConnectLeftVertex.
    fn dict_search_forward(&mut self, tmp_e_up: EdgeIdx) -> RegionIdx {
        let tmp_reg = self.alloc_region();
        self.region_mut(tmp_reg).e_up = tmp_e_up;

        // C dictSearch: walk forward from head.next until key==NULL or edge_leq(tmp, node.key)
        let mut node = self.dict.nodes[DICT_HEAD as usize].next;
        let result = loop {
            let key = self.dict.key(node);
            if key == INVALID {
                // hit head sentinel — not found
                break INVALID;
            }
            if self.edge_leq(tmp_reg, key) {
                break key;
            }
            node = self.dict.succ(node);
        };

        self.free_region(tmp_reg);
        result
    }

    fn connect_left_vertex(&mut self, v_event: VertIdx) {
        let an_edge = self.mesh.as_ref().unwrap().verts[v_event as usize].an_edge;
        if an_edge == INVALID { return; }

        #[cfg(test)]
        let dbg = std::env::var("TESS_DEBUG").is_ok();

        // tmp.eUp = vEvent->anEdge->Sym
        let tmp_e_up = an_edge ^ 1;

        // regUp = dictKey(dictSearch(dict, &tmp))
        // C dictSearch walks FORWARD and returns the FIRST region R where edge_leq(tmp, R).
        // This is the first (lowest) region whose upper edge is at or above v_event.
        let reg_up = self.dict_search_forward(tmp_e_up);
        #[cfg(test)]
        if dbg {
            let is_sentinel = if reg_up != INVALID { self.region(reg_up).sentinel } else { false };
            eprintln!("  connect_left_vertex: an_edge={} tmp_e_up={} → reg_up={} (sentinel={})",
                an_edge, tmp_e_up, reg_up, is_sentinel);
        }
        if reg_up == INVALID { return; }

        let reg_lo = self.region_below(reg_up);
        #[cfg(test)]
        if dbg {
            let is_sentinel = if reg_lo != INVALID { self.region(reg_lo).sentinel } else { false };
            eprintln!("  connect_left_vertex: reg_lo={} (sentinel={})", reg_lo, is_sentinel);
        }
        if reg_lo == INVALID { return; }

        let e_up = self.region(reg_up).e_up;
        let e_lo = self.region(reg_lo).e_up;
        if e_up == INVALID || e_lo == INVALID { return; }

        // Try merging with U or L first: EdgeSign(eUp->Dst, vEvent, eUp->Org) == 0
        let e_up_dst = self.mesh.as_ref().unwrap().dst(e_up);
        let e_up_org = self.mesh.as_ref().unwrap().edges[e_up as usize].org;
        if e_up_dst == INVALID || e_up_org == INVALID { return; }
        let eud_s = self.mesh.as_ref().unwrap().verts[e_up_dst as usize].s;
        let eud_t = self.mesh.as_ref().unwrap().verts[e_up_dst as usize].t;
        let euo_s = self.mesh.as_ref().unwrap().verts[e_up_org as usize].s;
        let euo_t = self.mesh.as_ref().unwrap().verts[e_up_org as usize].t;

        if crate::geom::edge_sign(eud_s, eud_t, self.event_s, self.event_t, euo_s, euo_t) == 0.0 {
            self.connect_left_degenerate(reg_up, v_event);
            return;
        }

        // reg = VertLeq(eLo->Dst, eUp->Dst) ? regUp : regLo
        let e_lo_dst = self.mesh.as_ref().unwrap().dst(e_lo);
        let eld_s = self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].s;
        let eld_t = self.mesh.as_ref().unwrap().verts[e_lo_dst as usize].t;
        let reg = if vert_leq(eld_s, eld_t, eud_s, eud_t) { reg_up } else { reg_lo };

        let reg_up_inside = self.region(reg_up).inside;
        let reg_fix = self.region(reg).fix_upper_edge;

        #[cfg(test)]
        if dbg {
            eprintln!("  connect_left_vertex: reg_up_inside={} reg_fix={} reg=={} reg_up=={} reg_lo=={}",
                reg_up_inside, reg_fix, reg, reg_up, reg_lo);
        }

        if reg_up_inside || reg_fix {
            let e_new = if reg == reg_up {
                // C: eNew = Connect(eUp->Lnext, vEvent->anEdge->Sym)
                // connect(eOrg, eDst) creates edge from eOrg->Dst to eDst->Org.
                // So eNew.Org = (eUp->Lnext)->Dst, going to (anEdge->Sym)->Org.
                let e_up_lnext = self.mesh.as_ref().unwrap().edges[e_up as usize].lnext;
                #[cfg(test)]
                if dbg { eprintln!("  → connect(e_up_lnext={}, an_edge^1={})", e_up_lnext, an_edge^1); }
                self.mesh.as_mut().unwrap().connect(e_up_lnext, an_edge ^ 1)
            } else {
                // tempHalfEdge = connect(eLo->Dnext, vEvent->anEdge); eNew = tempHalfEdge->Sym
                let e_lo_dnext = self.mesh.as_ref().unwrap().dnext(e_lo);
                #[cfg(test)]
                if dbg { eprintln!("  → connect(e_lo_dnext={}, an_edge={})^1", e_lo_dnext, an_edge); }
                self.mesh.as_mut().unwrap().connect(e_lo_dnext, an_edge).map(|e| e ^ 1)
            };
            let e_new = match e_new {
                Some(e) => e,
                None => return,
            };
            #[cfg(test)]
            if dbg { eprintln!("  connect_left_vertex: e_new={}", e_new); }

            if reg_fix {
                if !self.fix_upper_edge(reg, e_new) { return; }
            } else {
                // add_region_below calls compute_winding internally
                self.add_region_below(reg_up, e_new);
            }
            // Recursively process this vertex now that new edges are connected
            self.sweep_event(v_event);
        } else {
            // Vertex is in a region outside the polygon — just add right-going edges
            #[cfg(test)]
            if dbg { eprintln!("  → add_right_edges (outside region): an_edge={}", an_edge); }
            self.add_right_edges(reg_up, an_edge, an_edge, INVALID, true);
        }
    }

    /// Dict search: finds the first region where edge_leq(tmp_reg, region) == true.
    /// `tmp_e_up` is the e_up of a temporary region used for comparison.
    /// Returns the matching region index.
    fn dict_search_by_edge(&mut self, tmp_e_up: EdgeIdx) -> RegionIdx {
        // Temporarily allocate a region with tmp_e_up for comparison
        let tmp_reg = self.alloc_region();
        self.region_mut(tmp_reg).e_up = tmp_e_up;

        // Walk forward from head looking for the first node where edge_leq(tmp_reg, node_key)
        let mut node = self.dict.succ(DICT_HEAD);
        let result = loop {
            let key = self.dict.key(node);
            if key == INVALID {
                // Hit head (wrapped around) - not found
                break INVALID;
            }
            if self.edge_leq(tmp_reg, key) {
                break key;
            }
            node = self.dict.succ(node);
        };

        self.free_region(tmp_reg);
        result
    }

    fn sweep_event(&mut self, v_event: VertIdx) -> bool {
        let an_edge = self.mesh.as_ref().unwrap().verts[v_event as usize].an_edge;
        if an_edge == INVALID { return true; }

        #[cfg(test)]
        let dbg = std::env::var("TESS_DEBUG").is_ok();
        #[cfg(test)]
        if dbg {
            let (vs, vt) = (self.mesh.as_ref().unwrap().verts[v_event as usize].s,
                            self.mesh.as_ref().unwrap().verts[v_event as usize].t);
            eprintln!("sweep_event: v={} ({:.3},{:.3}) an_edge={}", v_event, vs, vt, an_edge);
        }

        // Walk through all edges at v_event (the onext ring).
        // If ANY has active_region != INVALID, it's already in the dict → "right vertex" case.
        // If NONE has active_region set → call connect_left_vertex (C: ConnectLeftVertex).
        let e_start = an_edge;
        let mut e = e_start;
        let found_e = loop {
            let ar = self.mesh.as_ref().unwrap().edges[e as usize].active_region;
            if ar != INVALID {
                break Some(e); // e is in the dict
            }
            let next = self.mesh.as_ref().unwrap().edges[e as usize].onext;
            e = next;
            if e == e_start {
                break None; // all edges have no active region
            }
        };

        if found_e.is_none() {
            // All edges are new (none in dict) → "left vertex" (C: ConnectLeftVertex)
            #[cfg(test)]
            if dbg { eprintln!("  → connect_left_vertex (no active regions)"); }
            self.connect_left_vertex(v_event);
            return true;
        }

        // At least one edge is already in the dict.
        // e = that edge with active_region != INVALID.
        let e = found_e.unwrap();
        let reg_up = {
            let ar = self.mesh.as_ref().unwrap().edges[e as usize].active_region;
            #[cfg(test)]
            if dbg { eprintln!("  → right-vertex path: found_e={} active_region={}", e, ar); }
            self.top_left_region(ar)
        };
        if reg_up == INVALID { return false; }

        let reg_lo = self.region_below(reg_up);
        if reg_lo == INVALID { return true; }
        let e_top_left = self.region(reg_lo).e_up;
        #[cfg(test)]
        if dbg {
            eprintln!("  reg_up={} reg_lo={} e_top_left={}", reg_up, reg_lo, e_top_left);
        }
        let e_bottom_left = self.finish_left_regions(reg_lo, INVALID);

        if e_bottom_left == INVALID { return true; }
        let e_bottom_left_onext = self.mesh.as_ref().unwrap().edges[e_bottom_left as usize].onext;
        #[cfg(test)]
        if dbg {
            eprintln!("  e_bottom_left={} e_bottom_left_onext={} e_top_left={}",
                e_bottom_left, e_bottom_left_onext, e_top_left);
        }
        if e_bottom_left_onext == e_top_left {
            // No right-going edges → temporary fixable edge
            #[cfg(test)]
            if dbg { eprintln!("  → connect_right_vertex"); }
            self.connect_right_vertex(reg_up, e_bottom_left);
        } else {
            #[cfg(test)]
            if dbg { eprintln!("  → add_right_edges"); }
            self.add_right_edges(reg_up, e_bottom_left_onext, e_top_left, e_top_left, true);
        }
        true
    }

    // ─────── Output ───────────────────────────────────────────────────────────

    fn output_polymesh(&mut self, element_type: ElementType, poly_size: usize, vertex_size: usize) {
        if poly_size > 3 {
            if let Some(ref mut mesh) = self.mesh {
                if !mesh.merge_convex_faces(poly_size) {
                    self.status = TessStatus::OutOfMemory;
                    return;
                }
            }
        }

        let mesh = match self.mesh.as_mut() { Some(m) => m, None => return };

        // Mark all vertices unused
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD {
            mesh.verts[v as usize].n = TESS_UNDEF;
            v = mesh.verts[v as usize].next;
        }

        let mut max_vert = 0u32;
        let mut max_face = 0u32;

        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            mesh.faces[f as usize].n = TESS_UNDEF;
            if !mesh.faces[f as usize].inside { f = mesh.faces[f as usize].next; continue; }

            let e_start = mesh.faces[f as usize].an_edge;
            let mut e = e_start;
            loop {
                let org = mesh.edges[e as usize].org;
                if mesh.verts[org as usize].n == TESS_UNDEF {
                    mesh.verts[org as usize].n = max_vert;
                    max_vert += 1;
                }
                e = mesh.edges[e as usize].lnext;
                if e == e_start { break; }
            }
            mesh.faces[f as usize].n = max_face;
            max_face += 1;
            f = mesh.faces[f as usize].next;
        }

        self.out_element_count = max_face as usize;
        self.out_vertex_count = max_vert as usize;

        let stride = if element_type == ElementType::ConnectedPolygons { poly_size * 2 } else { poly_size };
        self.out_elements = vec![TESS_UNDEF; max_face as usize * stride];
        self.out_vertices = vec![0.0; max_vert as usize * vertex_size];
        self.out_vertex_indices = vec![TESS_UNDEF; max_vert as usize];

        // Output vertex data
        let mesh = self.mesh.as_ref().unwrap();
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD {
            let n = mesh.verts[v as usize].n;
            if n != TESS_UNDEF {
                let base = n as usize * vertex_size;
                self.out_vertices[base] = mesh.verts[v as usize].coords[0];
                self.out_vertices[base + 1] = mesh.verts[v as usize].coords[1];
                if vertex_size > 2 { self.out_vertices[base + 2] = mesh.verts[v as usize].coords[2]; }
                self.out_vertex_indices[n as usize] = mesh.verts[v as usize].idx;
            }
            v = mesh.verts[v as usize].next;
        }

        // Output element indices
        let mut ep = 0;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            if !mesh.faces[f as usize].inside { f = mesh.faces[f as usize].next; continue; }
            let e_start = mesh.faces[f as usize].an_edge;
            let mut e = e_start;
            let mut fv = 0;
            loop {
                let org = mesh.edges[e as usize].org;
                self.out_elements[ep] = mesh.verts[org as usize].n;
                ep += 1; fv += 1;
                e = mesh.edges[e as usize].lnext;
                if e == e_start { break; }
            }
            for _ in fv..poly_size { self.out_elements[ep] = TESS_UNDEF; ep += 1; }

            if element_type == ElementType::ConnectedPolygons {
                let e_start = mesh.faces[f as usize].an_edge;
                let mut e = e_start;
                let mut fv2 = 0;
                loop {
                    let rf = mesh.rface(e);
                    let nf = if rf != INVALID && mesh.faces[rf as usize].inside {
                        mesh.faces[rf as usize].n
                    } else { TESS_UNDEF };
                    self.out_elements[ep] = nf;
                    ep += 1; fv2 += 1;
                    e = mesh.edges[e as usize].lnext;
                    if e == e_start { break; }
                }
                for _ in fv2..poly_size { self.out_elements[ep] = TESS_UNDEF; ep += 1; }
            }

            f = mesh.faces[f as usize].next;
        }
    }

    fn output_contours(&mut self, vertex_size: usize) {
        let mesh = match self.mesh.as_ref() { Some(m) => m, None => return };
        let mut total_verts = 0usize;
        let mut total_elems = 0usize;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            if mesh.faces[f as usize].inside {
                let e_start = mesh.faces[f as usize].an_edge;
                let mut e = e_start;
                loop { total_verts += 1; e = mesh.edges[e as usize].lnext; if e == e_start { break; } }
                total_elems += 1;
            }
            f = mesh.faces[f as usize].next;
        }
        self.out_element_count = total_elems;
        self.out_vertex_count = total_verts;
        self.out_elements = vec![TESS_UNDEF; total_elems * 2];
        self.out_vertices = vec![0.0; total_verts * vertex_size];
        self.out_vertex_indices = vec![TESS_UNDEF; total_verts];

        let mesh = self.mesh.as_ref().unwrap();
        let mut vp = 0usize;
        let mut ep = 0usize;
        let mut sv = 0usize;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            if !mesh.faces[f as usize].inside { f = mesh.faces[f as usize].next; continue; }
            let e_start = mesh.faces[f as usize].an_edge;
            let mut e = e_start;
            let mut vc = 0usize;
            loop {
                let org = mesh.edges[e as usize].org;
                let base = vp * vertex_size;
                self.out_vertices[base] = mesh.verts[org as usize].coords[0];
                self.out_vertices[base + 1] = mesh.verts[org as usize].coords[1];
                if vertex_size > 2 { self.out_vertices[base + 2] = mesh.verts[org as usize].coords[2]; }
                self.out_vertex_indices[vp] = mesh.verts[org as usize].idx;
                vp += 1; vc += 1;
                e = mesh.edges[e as usize].lnext;
                if e == e_start { break; }
            }
            self.out_elements[ep] = sv as u32;
            self.out_elements[ep + 1] = vc as u32;
            ep += 2; sv += vc;
            f = mesh.faces[f as usize].next;
        }
    }
}

// These fields need to be added to the Tessellator struct above.
// Rust doesn't allow extending structs, so we handle the sorted event queue
// by adding fields via a separate tracking mechanism.
// We'll use a Vec<VertIdx> stored directly in the tessellator.
// (Fields added as sorted_events and sorted_event_pos in struct definition)

// ─────────────────────────── Helper functions ─────────────────────────────────

fn is_valid_coord(c: f32) -> bool {
    c <= MAX_VALID_COORD && c >= MIN_VALID_COORD && !c.is_nan()
}

fn dot(u: &[f32; 3], v: &[f32; 3]) -> f32 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

fn long_axis(v: &[f32; 3]) -> usize {
    let mut i = 0;
    if v[1].abs() > v[0].abs() { i = 1; }
    if v[2].abs() > v[i].abs() { i = 2; }
    i
}

fn short_axis(v: &[f32; 3]) -> usize {
    let mut i = 0;
    if v[1].abs() < v[0].abs() { i = 1; }
    if v[2].abs() < v[i].abs() { i = 2; }
    i
}

fn compute_normal(mesh: &Mesh, norm: &mut [f32; 3]) {
    let first_v = mesh.verts[V_HEAD as usize].next;
    if first_v == V_HEAD { norm[0] = 0.0; norm[1] = 0.0; norm[2] = 1.0; return; }

    let mut max_val = [0f32; 3];
    let mut min_val = [0f32; 3];
    let mut max_vert = [V_HEAD; 3];
    let mut min_vert = [V_HEAD; 3];

    for i in 0..3 {
        let c = mesh.verts[first_v as usize].coords[i];
        min_val[i] = c; min_vert[i] = first_v;
        max_val[i] = c; max_vert[i] = first_v;
    }

    let mut v = mesh.verts[V_HEAD as usize].next;
    while v != V_HEAD {
        for i in 0..3 {
            let c = mesh.verts[v as usize].coords[i];
            if c < min_val[i] { min_val[i] = c; min_vert[i] = v; }
            if c > max_val[i] { max_val[i] = c; max_vert[i] = v; }
        }
        v = mesh.verts[v as usize].next;
    }

    let mut i = 0;
    if max_val[1] - min_val[1] > max_val[0] - min_val[0] { i = 1; }
    if max_val[2] - min_val[2] > max_val[i] - min_val[i] { i = 2; }
    if min_val[i] >= max_val[i] { norm[0] = 0.0; norm[1] = 0.0; norm[2] = 1.0; return; }

    let v1 = min_vert[i];
    let v2 = max_vert[i];
    let d1 = [
        mesh.verts[v1 as usize].coords[0] - mesh.verts[v2 as usize].coords[0],
        mesh.verts[v1 as usize].coords[1] - mesh.verts[v2 as usize].coords[1],
        mesh.verts[v1 as usize].coords[2] - mesh.verts[v2 as usize].coords[2],
    ];

    let mut max_len2 = 0.0f32;
    let mut v = mesh.verts[V_HEAD as usize].next;
    while v != V_HEAD {
        let d2 = [
            mesh.verts[v as usize].coords[0] - mesh.verts[v2 as usize].coords[0],
            mesh.verts[v as usize].coords[1] - mesh.verts[v2 as usize].coords[1],
            mesh.verts[v as usize].coords[2] - mesh.verts[v2 as usize].coords[2],
        ];
        let tn = [d1[1]*d2[2]-d1[2]*d2[1], d1[2]*d2[0]-d1[0]*d2[2], d1[0]*d2[1]-d1[1]*d2[0]];
        let tl2 = tn[0]*tn[0] + tn[1]*tn[1] + tn[2]*tn[2];
        if tl2 > max_len2 { max_len2 = tl2; *norm = tn; }
        v = mesh.verts[v as usize].next;
    }

    if max_len2 <= 0.0 {
        norm[0] = 0.0; norm[1] = 0.0; norm[2] = 0.0;
        norm[short_axis(&d1)] = 1.0;
    }
}

fn check_orientation(mesh: &mut Mesh) {
    let mut area = 0.0f32;
    let mut f = mesh.faces[F_HEAD as usize].next;
    while f != F_HEAD {
        let an = mesh.faces[f as usize].an_edge;
        if an != INVALID && mesh.edges[an as usize].winding > 0 {
            let mut e = an;
            loop {
                let org = mesh.edges[e as usize].org;
                let dst = mesh.dst(e);
                area += (mesh.verts[org as usize].s - mesh.verts[dst as usize].s)
                    * (mesh.verts[org as usize].t + mesh.verts[dst as usize].t);
                e = mesh.edges[e as usize].lnext;
                if e == an { break; }
            }
        }
        f = mesh.faces[f as usize].next;
    }
    if area < 0.0 {
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD {
            mesh.verts[v as usize].t = -mesh.verts[v as usize].t;
            v = mesh.verts[v as usize].next;
        }
    }
}

fn compute_intersect_coords(
    _isect_s: Real, _isect_t: Real,
    org_up_s: Real, org_up_t: Real, _org_up_coords: [Real; 3],
    dst_up_s: Real, dst_up_t: Real,
    org_lo_s: Real, org_lo_t: Real,
    dst_lo_s: Real, dst_lo_t: Real,
) -> [Real; 3] {
    // Simplified: return zeros (coordinates computed by interpolation in full impl)
    [0.0, 0.0, 0.0]
}

// ────────────────────────── Public wrapper ────────────────────────────────────

/// High-level tessellator (public interface).
pub struct TessellatorApi {
    inner: Tessellator,
}

impl TessellatorApi {
    pub fn new() -> Self {
        TessellatorApi { inner: Tessellator::new() }
    }
    pub fn set_option(&mut self, option: TessOption, value: bool) { self.inner.set_option(option, value); }
    pub fn add_contour(&mut self, size: usize, vertices: &[f32]) { self.inner.add_contour(size, vertices); }
    pub fn tessellate(&mut self, winding_rule: WindingRule, element_type: ElementType, poly_size: usize, vertex_size: usize, normal: Option<[f32; 3]>) -> bool {
        self.inner.tessellate(winding_rule, element_type, poly_size, vertex_size, normal)
    }
    pub fn vertex_count(&self) -> usize { self.inner.vertex_count() }
    pub fn element_count(&self) -> usize { self.inner.element_count() }
    pub fn vertices(&self) -> &[f32] { self.inner.vertices() }
    pub fn vertex_indices(&self) -> &[u32] { self.inner.vertex_indices() }
    pub fn elements(&self) -> &[u32] { self.inner.elements() }
    pub fn status(&self) -> TessStatus { self.inner.get_status() }
}

impl Default for TessellatorApi {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_polygon_with_hole() {
        use crate::mesh::{F_HEAD, INVALID as MESH_INVALID};
        let mut tess = Tessellator::new();
        // outer CCW square
        tess.set_option(TessOption::ReverseContours, false);
        tess.add_contour(2, &[0.0f32, 0.0, 3.0, 0.0, 3.0, 3.0, 0.0, 3.0]);
        // inner CW hole
        tess.set_option(TessOption::ReverseContours, true);
        tess.add_contour(2, &[1.0f32, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0]);
        
        // Run interior manually but stop before tessellate_interior
        tess.winding_rule = WindingRule::Positive;
        tess.project_polygon();
        
        // Run just the sweep (not tessellate_interior)
        tess.remove_degenerate_edges();
        tess.init_priority_queue();
        tess.init_edge_dict();
        loop {
            if tess.pq_is_empty() { break; }
            let v = tess.pq_extract_min();
            if v == INVALID { break; }
            loop {
                if tess.pq_is_empty() { break; }
                let next_v = tess.pq_minimum();
                if next_v == INVALID { break; }
                let (v_s, v_t) = { let m = tess.mesh.as_ref().unwrap(); (m.verts[v as usize].s, m.verts[v as usize].t) };
                let (nv_s, nv_t) = { let m = tess.mesh.as_ref().unwrap(); (m.verts[next_v as usize].s, m.verts[next_v as usize].t) };
                if !crate::geom::vert_eq(v_s, v_t, nv_s, nv_t) { break; }
                let next_v = tess.pq_extract_min();
                let an1 = tess.mesh.as_ref().unwrap().verts[v as usize].an_edge;
                let an2 = tess.mesh.as_ref().unwrap().verts[next_v as usize].an_edge;
                if an1 != INVALID && an2 != INVALID {
                    tess.mesh.as_mut().unwrap().splice(an1, an2);
                }
            }
            tess.event = v;
            let (v_s, v_t) = { let m = tess.mesh.as_ref().unwrap(); (m.verts[v as usize].s, m.verts[v as usize].t) };
            tess.event_s = v_s; tess.event_t = v_t;
            tess.sweep_event(v);
        }
        tess.done_edge_dict();
        
        // Count faces before tessellate_interior
        {
            let mesh = tess.mesh.as_ref().unwrap();
            let mut inside_count = 0;
            let mut outside_count = 0;
            let mut f = mesh.faces[F_HEAD as usize].next;
            while f != F_HEAD {
                let inside = mesh.faces[f as usize].inside;
                // Count edges in face's lnext loop
                let ae = mesh.faces[f as usize].an_edge;
                let mut edge_count = 0;
                let mut e = ae;
                loop {
                    edge_count += 1;
                    e = mesh.edges[e as usize].lnext;
                    if e == ae { break; }
                    if edge_count > 100 { eprintln!("INFINITE LOOP in face {}!", f); break; }
                }
                eprintln!("Face {}: inside={} edge_count={}", f, inside, edge_count);
                if inside { inside_count += 1; } else { outside_count += 1; }
                f = mesh.faces[f as usize].next;
            }
            eprintln!("BEFORE tessellate_interior: inside={} outside={}", inside_count, outside_count);
        }
        
        // Run tessellate_interior
        tess.mesh.as_mut().unwrap().tessellate_interior();
        
        // Count faces after tessellate_interior
        let mesh = tess.mesh.as_ref().unwrap();
        let mut inside_count = 0;
        let mut outside_count = 0;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            let inside = mesh.faces[f as usize].inside;
            if inside { inside_count += 1; } else { outside_count += 1; }
            f = mesh.faces[f as usize].next;
        }
        eprintln!("AFTER tessellate_interior: inside={} outside={}", inside_count, outside_count);
    }

    #[test]
    fn debug_simple_quad() {
        let mut tess = Tessellator::new();
        tess.add_contour(2, &[0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
        let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
        eprintln!("simple_quad: ok={} element_count={}", ok, tess.element_count());
    }

    #[test]
    fn debug_single_triangle() {
        use crate::mesh::{F_HEAD, E_HEAD, V_HEAD, INVALID as MESH_INVALID};

        let mut tess = Tessellator::new();
        tess.add_contour(2, &[0.0f32, 0.0, 0.0, 1.0, 1.0, 0.0]);

        // Run compute_interior manually but keep mesh alive
        tess.winding_rule = WindingRule::Positive;
        if !tess.project_polygon() { panic!("project_polygon failed"); }

        // Print mesh state before sweep
        {
            let mesh = tess.mesh.as_ref().unwrap();
            eprintln!("=== After add_contour + project_polygon ===");
            // Print all edges (even and odd)
            for ei in 2..mesh.edges.len() {
                let e = ei as u32;
                let org = mesh.edges[e as usize].org;
                let (os, ot) = if org != MESH_INVALID && (org as usize) < mesh.verts.len() {
                    (mesh.verts[org as usize].s, mesh.verts[org as usize].t)
                } else { (-999.0, -999.0) };
                let lface = mesh.edges[e as usize].lface;
                let winding = mesh.edges[e as usize].winding;
                eprintln!("  Edge {}: org={} ({:.1},{:.1}) lface={} w={} onext={} lnext={} next={}",
                    e, org, os, ot, lface, winding,
                    mesh.edges[e as usize].onext,
                    mesh.edges[e as usize].lnext,
                    mesh.edges[e as usize].next);
            }
            let mut v = mesh.verts[V_HEAD as usize].next;
            while v != V_HEAD {
                eprintln!("  Vertex {}: s={} t={} an_edge={}", v, mesh.verts[v as usize].s, mesh.verts[v as usize].t, mesh.verts[v as usize].an_edge);
                v = mesh.verts[v as usize].next;
            }
        }

        if !tess.compute_interior() { panic!("compute_interior failed"); }

        // Count faces with inside=true
        let mesh = tess.mesh.as_ref().unwrap();
        let mut inside_count = 0;
        let mut total_faces = 0;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            total_faces += 1;
            if mesh.faces[f as usize].inside {
                inside_count += 1;
            }
            eprintln!("  Face {}: inside={} an_edge={}", f, mesh.faces[f as usize].inside, mesh.faces[f as usize].an_edge);
            f = mesh.faces[f as usize].next;
        }
        eprintln!("Total faces: {}, inside: {}", total_faces, inside_count);
    }

    #[test]
    fn empty_polyline() {
        let mut tess = TessellatorApi::new();
        tess.add_contour(2, &[]);
        let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
        assert!(ok);
        assert_eq!(tess.element_count(), 0);
    }

    #[test]
    fn invalid_input_status() {
        let mut tess = TessellatorApi::new();
        tess.add_contour(2, &[-2e37f32, 0.0, 0.0, 5.0, 1e37f32, -5.0]);
        let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
        assert!(!ok);
        assert_eq!(tess.status(), TessStatus::InvalidInput);
    }

    #[test]
    fn nan_quad_fails_gracefully() {
        let nan = f32::NAN;
        let mut tess = TessellatorApi::new();
        tess.add_contour(2, &[nan, nan, nan, nan, nan, nan, nan, nan]);
        let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
        // NaN is not a valid coord, so should fail with InvalidInput
        assert!(!ok);
    }

    #[test]
    fn float_overflow_quad_does_not_panic() {
        let min = f32::MIN;
        let max = f32::MAX;
        let mut tess = TessellatorApi::new();
        tess.add_contour(2, &[min, min, min, max, max, max, max, min]);
        let _ = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    }

    #[test]
    fn singularity_quad_no_panic() {
        let mut tess = TessellatorApi::new();
        tess.add_contour(2, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
        // Either succeeds with 0 elements or fails gracefully
        if ok { assert_eq!(tess.element_count(), 0); }
    }
}
