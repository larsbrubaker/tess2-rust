// Copyright 2025 Lars Brubaker
// Unit tests for the tessellator internals.

use super::*;

#[test]
fn debug_polygon_with_hole() {
    use crate::mesh::{F_HEAD, INVALID as MESH_INVALID};
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ReverseContours, false);
    tess.add_contour(2, &[0.0f32, 0.0, 3.0, 0.0, 3.0, 3.0, 0.0, 3.0]);
    tess.set_option(TessOption::ReverseContours, true);
    tess.add_contour(2, &[1.0f32, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0]);

    tess.winding_rule = WindingRule::Positive;
    tess.project_polygon();

    tess.remove_degenerate_edges();
    tess.init_priority_queue();
    tess.init_edge_dict();
    loop {
        if tess.pq_is_empty() {
            break;
        }
        let v = tess.pq_extract_min();
        if v == INVALID {
            break;
        }
        loop {
            if tess.pq_is_empty() {
                break;
            }
            let next_v = tess.pq_minimum();
            if next_v == INVALID {
                break;
            }
            let (v_s, v_t) = {
                let m = tess.mesh.as_ref().unwrap();
                (m.verts[v as usize].s, m.verts[v as usize].t)
            };
            let (nv_s, nv_t) = {
                let m = tess.mesh.as_ref().unwrap();
                (m.verts[next_v as usize].s, m.verts[next_v as usize].t)
            };
            if !crate::geom::vert_eq(v_s, v_t, nv_s, nv_t) {
                break;
            }
            let next_v = tess.pq_extract_min();
            let an1 = tess.mesh.as_ref().unwrap().verts[v as usize].an_edge;
            let an2 = tess.mesh.as_ref().unwrap().verts[next_v as usize].an_edge;
            if an1 != INVALID && an2 != INVALID {
                tess.mesh.as_mut().unwrap().splice(an1, an2);
            }
        }
        tess.event = v;
        let (v_s, v_t) = {
            let m = tess.mesh.as_ref().unwrap();
            (m.verts[v as usize].s, m.verts[v as usize].t)
        };
        tess.event_s = v_s;
        tess.event_t = v_t;
        tess.sweep_event(v);
    }
    tess.done_edge_dict();

    {
        let mesh = tess.mesh.as_ref().unwrap();
        let mut inside_count = 0;
        let mut outside_count = 0;
        let mut f = mesh.faces[F_HEAD as usize].next;
        while f != F_HEAD {
            let inside = mesh.faces[f as usize].inside;
            let ae = mesh.faces[f as usize].an_edge;
            let mut edge_count = 0;
            let mut e = ae;
            loop {
                edge_count += 1;
                e = mesh.edges[e as usize].lnext;
                if e == ae {
                    break;
                }
                if edge_count > 100 {
                    eprintln!("INFINITE LOOP in face {}!", f);
                    break;
                }
            }
            eprintln!("Face {}: inside={} edge_count={}", f, inside, edge_count);
            if inside {
                inside_count += 1;
            } else {
                outside_count += 1;
            }
            f = mesh.faces[f as usize].next;
        }
        eprintln!(
            "BEFORE tessellate_interior: inside={} outside={}",
            inside_count, outside_count
        );
    }

    tess.mesh.as_mut().unwrap().tessellate_interior();

    let mesh = tess.mesh.as_ref().unwrap();
    let mut inside_count = 0;
    let mut outside_count = 0;
    let mut f = mesh.faces[F_HEAD as usize].next;
    while f != F_HEAD {
        let inside = mesh.faces[f as usize].inside;
        if inside {
            inside_count += 1;
        } else {
            outside_count += 1;
        }
        f = mesh.faces[f as usize].next;
    }
    eprintln!(
        "AFTER tessellate_interior: inside={} outside={}",
        inside_count, outside_count
    );
}

#[test]
fn debug_simple_quad() {
    let mut tess = Tessellator::new();
    tess.add_contour(2, &[0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    let ok = tess.tessellate(WindingRule::Positive, ElementType::Polygons, 3, 2, None);
    eprintln!(
        "simple_quad: ok={} element_count={}",
        ok,
        tess.element_count()
    );
}

#[test]
fn debug_single_triangle() {
    use crate::mesh::{E_HEAD, F_HEAD, INVALID as MESH_INVALID, V_HEAD};

    let mut tess = Tessellator::new();
    tess.add_contour(2, &[0.0f32, 0.0, 0.0, 1.0, 1.0, 0.0]);

    tess.winding_rule = WindingRule::Positive;
    if !tess.project_polygon() {
        panic!("project_polygon failed");
    }

    {
        let mesh = tess.mesh.as_ref().unwrap();
        eprintln!("=== After add_contour + project_polygon ===");
        for ei in 2..mesh.edges.len() {
            let e = ei as u32;
            let org = mesh.edges[e as usize].org;
            let (os, ot) = if org != MESH_INVALID && (org as usize) < mesh.verts.len() {
                (mesh.verts[org as usize].s, mesh.verts[org as usize].t)
            } else {
                (-999.0, -999.0)
            };
            let lface = mesh.edges[e as usize].lface;
            let winding = mesh.edges[e as usize].winding;
            eprintln!(
                "  Edge {}: org={} ({:.1},{:.1}) lface={} w={} onext={} lnext={} next={}",
                e, org, os, ot, lface, winding,
                mesh.edges[e as usize].onext,
                mesh.edges[e as usize].lnext,
                mesh.edges[e as usize].next
            );
        }
        let mut v = mesh.verts[V_HEAD as usize].next;
        while v != V_HEAD {
            eprintln!(
                "  Vertex {}: s={} t={} an_edge={}",
                v,
                mesh.verts[v as usize].s,
                mesh.verts[v as usize].t,
                mesh.verts[v as usize].an_edge
            );
            v = mesh.verts[v as usize].next;
        }
    }

    if !tess.compute_interior() {
        panic!("compute_interior failed");
    }

    let mesh = tess.mesh.as_ref().unwrap();
    let mut inside_count = 0;
    let mut total_faces = 0;
    let mut f = mesh.faces[F_HEAD as usize].next;
    while f != F_HEAD {
        total_faces += 1;
        if mesh.faces[f as usize].inside {
            inside_count += 1;
        }
        eprintln!(
            "  Face {}: inside={} an_edge={}",
            f, mesh.faces[f as usize].inside, mesh.faces[f as usize].an_edge
        );
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
    if ok {
        assert_eq!(tess.element_count(), 0);
    }
}
