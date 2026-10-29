// Copyright 2025 Lars Brubaker
// Constrained Delaunay refinement checked for what it promises: every edge it may flip ends
// locally Delaunay (judged by the exact predicate), the boundary and the triangle count are those
// of plain tessellation, and on input full of cocircular points - outlines traced on a pixel grid,
// points on one lattice circle - it finishes in a number of flips that grows with the input, not
// its square.

mod helpers;

use std::collections::HashMap;

use tess2_rust::predicates::in_circle_sign;
use tess2_rust::{ElementType, TessOption, Tessellator, WindingRule};

/// The loops of an SVG path made of `M`, `L` and `Z` commands only.
fn svg_loops(d: &str) -> Vec<Vec<[f64; 2]>> {
    let mut loops = Vec::new();
    let mut current = Vec::new();
    let mut numbers: Vec<f64> = Vec::new();
    for token in d.split(|c: char| c == ' ' || c == ',' || c.is_ascii_alphabetic()) {
        if let Ok(n) = token.parse::<f64>() {
            numbers.push(n);
        }
    }
    // Commands only separate points, so walk the text again to see where loops close.
    let mut index = 0;
    for c in d.chars() {
        match c {
            'M' | 'L' => {
                current.push([numbers[index], numbers[index + 1]]);
                index += 2;
            }
            'Z' => loops.push(std::mem::take(&mut current)),
            _ => {}
        }
    }
    loops
}

/// The traced silhouette, in clipper-style integer units (1000 per mm).
fn traced_outline() -> Vec<Vec<[f64; 2]>> {
    svg_loops(include_str!("data/silhouette_trace.svgd"))
        .into_iter()
        .map(|l| {
            l.iter()
                .map(|p| [(p[0] * 1000.0).round(), (p[1] * 1000.0).round()])
                .collect()
        })
        .collect()
}

/// Each loop shrunk slightly about its own centroid, onto the same integer grid: with the
/// originals, under the odd winding rule, a thin band like a bevel's.
fn shrunk(loops: &[Vec<[f64; 2]>], factor: f64) -> Vec<Vec<[f64; 2]>> {
    loops
        .iter()
        .map(|l| {
            let n = l.len() as f64;
            let cx = l.iter().map(|p| p[0]).sum::<f64>() / n;
            let cy = l.iter().map(|p| p[1]).sum::<f64>() / n;
            l.iter()
                .map(|p| {
                    [
                        (cx + (p[0] - cx) * factor).round(),
                        (cy + (p[1] - cy) * factor).round(),
                    ]
                })
                .collect()
        })
        .collect()
}

/// The staircase outline of the grid cells whose centres lie within `radius` of the origin.
fn pixel_disk(radius: i64) -> Vec<[f64; 2]> {
    let half_width = |row: i64| {
        let y = row as f64 + 0.5;
        ((radius * radius) as f64 - y * y).max(0.0).sqrt().round() as i64
    };
    let rows: Vec<i64> = (-radius..radius).filter(|&r| half_width(r) > 0).collect();
    let mut right = Vec::new();
    let mut left = Vec::new();
    for &row in &rows {
        let w = half_width(row);
        right.push([w as f64, row as f64]);
        right.push([w as f64, (row + 1) as f64]);
        left.push([-w as f64, row as f64]);
        left.push([-w as f64, (row + 1) as f64]);
    }
    left.reverse();
    right.extend(left);
    right
}

/// Every lattice point on the circle x² + y² = r², counter-clockwise by angle.
fn lattice_circle(r: i64) -> Vec<[f64; 2]> {
    let mut points: Vec<[f64; 2]> = Vec::new();
    for x in -r..=r {
        let y2 = r * r - x * x;
        let y = (y2 as f64).sqrt().round() as i64;
        if y * y == y2 {
            points.push([x as f64, y as f64]);
            if y != 0 {
                points.push([x as f64, -y as f64]);
            }
        }
    }
    points.sort_by(|a, b| a[1].atan2(a[0]).total_cmp(&b[1].atan2(b[0])));
    points
}

fn tessellate(loops: &[Vec<[f64; 2]>], rule: WindingRule, cdt: bool) -> Tessellator {
    let mut tess = Tessellator::new();
    tess.set_option(TessOption::ConstrainedDelaunayTriangulation, cdt);
    for l in loops {
        let flat: Vec<f64> = l.iter().flat_map(|p| *p).collect();
        tess.add_contour(2, &flat);
    }
    assert!(tess.tessellate(rule, ElementType::Polygons, 3, 2, None));
    tess
}

fn point(tess: &Tessellator, i: u32) -> [f64; 2] {
    let v = tess.vertices();
    [v[2 * i as usize], v[2 * i as usize + 1]]
}

fn key(p: [f64; 2]) -> (u64, u64) {
    (p[0].to_bits(), p[1].to_bits())
}

/// One triangle seen from an edge: its corner opposite that edge, and its corners
/// counter-clockwise.
type Side = (u32, [u32; 3]);

/// Asserts the triangulation is constrained Delaunay: across every edge that is not an input
/// edge, neither triangle's opposite corner lies strictly inside the other's circumcircle,
/// judged by the exact predicate.
fn assert_constrained_delaunay(tess: &Tessellator) {
    let elements = tess.elements();
    let flags = tess.edge_flags();
    // Undirected edge -> (opposite corner, the triangle's corners counter-clockwise).
    let mut sides: HashMap<(u32, u32), Vec<Side>> = HashMap::new();
    for (t, tri) in elements.chunks(3).enumerate() {
        let [a, b, c] = [tri[0], tri[1], tri[2]];
        let (pa, pb, pc) = (point(tess, a), point(tess, b), point(tess, c));
        let cross = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pc[0] - pa[0]) * (pb[1] - pa[1]);
        let ccw = if cross > 0.0 { [a, b, c] } else { [a, c, b] };
        for k in 0..3 {
            if flags[3 * t + k] == 1 {
                continue;
            }
            let (u, v) = (tri[k], tri[(k + 1) % 3]);
            let opposite = tri[(k + 2) % 3];
            sides
                .entry((u.min(v), u.max(v)))
                .or_default()
                .push((opposite, ccw));
        }
    }
    let mut violations = 0;
    for both in sides.values() {
        if let [(_, first), (other_corner, _)] = both.as_slice() {
            let p = |i: u32| point(tess, i);
            let [a, b, c] = *first;
            if in_circle_sign(p(a), p(b), p(c), p(*other_corner)) > 0 {
                violations += 1;
            }
        }
    }
    assert_eq!(violations, 0, "edges that are not locally Delaunay");
}

/// The output's boundary edges (those flagged as input edges), by their end coordinates.
fn boundary_edges(tess: &Tessellator) -> Vec<((u64, u64), (u64, u64))> {
    let flags = tess.edge_flags();
    let mut edges: Vec<_> = tess
        .elements()
        .chunks(3)
        .enumerate()
        .flat_map(|(t, tri)| {
            (0..3)
                .filter(move |&k| flags[3 * t + k] == 1)
                .map(move |k| (tri[k], tri[(k + 1) % 3]))
        })
        .map(|(u, v)| (key(point(tess, u)), key(point(tess, v))))
        .collect();
    edges.sort_unstable();
    edges
}

/// Asserts every input edge between distinct points is an edge of the output.
fn assert_keeps_input_edges(tess: &Tessellator, loops: &[Vec<[f64; 2]>]) {
    let mut out_edges = std::collections::HashSet::new();
    for tri in tess.elements().chunks(3) {
        for k in 0..3 {
            let (u, v) = (point(tess, tri[k]), point(tess, tri[(k + 1) % 3]));
            out_edges.insert((key(u), key(v)));
            out_edges.insert((key(v), key(u)));
        }
    }
    let mut missing = 0;
    for l in loops {
        for i in 0..l.len() {
            let (u, v) = (l[i], l[(i + 1) % l.len()]);
            if u != v && !out_edges.contains(&(key(u), key(v))) {
                missing += 1;
            }
        }
    }
    assert_eq!(missing, 0, "input edges missing from the triangulation");
}

/// Tessellates with and without the refinement and checks the refined result: a flip count
/// linear in the points, the same triangle count, the Delaunay property, and the same boundary.
/// `keeps_input_edges` is for input that neither crosses nor touches itself, whose every edge
/// must then be an edge of the output.
fn check(loops: &[Vec<[f64; 2]>], rule: WindingRule, keeps_input_edges: bool) {
    let points: usize = loops.iter().map(|l| l.len()).sum();
    let plain = tessellate(loops, rule, false);
    let refined = tessellate(loops, rule, true);
    // The sweep's triangulation of long thin pieces fans out from few corners, so a valid
    // refinement flips each point's edges a few times over; these inputs take 1 to 3 flips per
    // point. A refinement that flips cocircular edges back and forth takes thousands.
    assert!(
        refined.delaunay_flip_count() <= 4 * points,
        "{} flips for {points} points",
        refined.delaunay_flip_count()
    );
    assert_eq!(refined.element_count(), plain.element_count());
    assert_constrained_delaunay(&refined);
    // The refinement flips only internal edges: the boundary is the sweep's, edge for edge.
    assert_eq!(boundary_edges(&refined), boundary_edges(&plain));
    if keeps_input_edges {
        assert_keeps_input_edges(&refined, loops);
    }
}

#[test]
fn a_traced_outline_refines_in_few_flips_per_point() {
    // After rounding onto the grid the trace touches itself in places, where the sweep splits
    // its edges, so the boundary is checked against plain tessellation instead.
    check(&traced_outline(), WindingRule::Odd, false);
}

#[test]
fn a_thin_band_around_a_traced_outline_refines_in_few_flips_per_point() {
    let outline = traced_outline();
    let mut band = outline.clone();
    band.extend(shrunk(&outline, 0.995));
    // The shrunk copy crosses the outline here and there, where the sweep splits input edges.
    check(&band, WindingRule::Odd, false);
}

#[test]
fn a_band_between_two_pixel_staircase_disks_refines_in_few_flips_per_point() {
    let outer = pixel_disk(120);
    let mut inner = pixel_disk(117);
    inner.reverse();
    check(&[outer, inner], WindingRule::Positive, true);
}

#[test]
fn a_polygon_of_points_on_one_circle_needs_no_flips() {
    // 5² · 13² · 17² has many representations as a sum of two squares.
    let ring = lattice_circle(5 * 13 * 17);
    assert!(ring.len() > 50, "{} points", ring.len());
    let refined = tessellate(std::slice::from_ref(&ring), WindingRule::Positive, true);
    // Every triangulation of a convex polygon with all corners on one circle is Delaunay.
    assert_eq!(refined.delaunay_flip_count(), 0);
    assert_eq!(refined.element_count(), ring.len() - 2);
    assert_constrained_delaunay(&refined);
    assert_keeps_input_edges(&refined, std::slice::from_ref(&ring));
}

#[test]
fn every_sample_drawing_refines_to_a_constrained_delaunay_triangulation() {
    let samples = [
        include_str!("data/dude.dat"),
        include_str!("data/star.dat"),
        include_str!("data/spiral.dat"),
        include_str!("data/bird.dat"),
        include_str!("data/tank.dat"),
        include_str!("data/funny.dat"),
        include_str!("data/kzer_za.dat"),
        include_str!("data/dude_holes.dat"),
    ];
    for data in samples {
        let loops: Vec<Vec<[f64; 2]>> = helpers::parse_contours(data)
            .iter()
            .map(|c| c.chunks(2).map(|p| [p[0], p[1]]).collect())
            .collect();
        for rule in [WindingRule::Odd, WindingRule::Positive] {
            let plain = tessellate(&loops, rule, false);
            let refined = tessellate(&loops, rule, true);
            assert_eq!(refined.element_count(), plain.element_count());
            assert_eq!(boundary_edges(&refined), boundary_edges(&plain));
            assert_constrained_delaunay(&refined);
        }
    }
}
