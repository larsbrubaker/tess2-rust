// Copyright 2025 Lars Brubaker
// Port of agg-sharp `Tests/Agg.Tests/Other/TesselatorTests.cs` (MatterCAD's
// C# GLU tessellator), checked against `Tessellator::render_order`.
//
// The C# test feeds four instruction streams through the GLU callback
// tessellator under every winding rule, once without and once with an
// edge-flag callback, and compares every callback against a recorded token
// stream (`tests/data/glu_tesselator_streams.txt`, copied verbatim from the
// C# arrays).  With an edge-flag callback GLU emits only separate triangles,
// which is what `render_order` reproduces: those twenty runs are ported here
// with their combine coordinates (1e-6, the C# EPSILON), client indices and
// edge flags in order.  The twenty runs without an edge-flag callback pin
// GLU's triangle fan/strip grouping, which tess2 has no output for, and are
// not ported.

use tess2_rust::{ElementType, Tessellator, WindingRule};

const EPSILON: f64 = 1e-6;

struct Streams {
    inputs: Vec<Vec<String>>,
    outputs: Vec<Vec<String>>,
}

fn load_streams() -> Streams {
    let text = include_str!("data/glu_tesselator_streams.txt");
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut tokens = line.split_whitespace().map(str::to_string);
        let kind = tokens.next().unwrap();
        let index: usize = tokens.next().unwrap().parse().unwrap();
        let rest: Vec<String> = tokens.collect();
        let target = if kind == "IN" {
            &mut inputs
        } else {
            &mut outputs
        };
        assert_eq!(target.len(), index, "{kind} lines are in order");
        target.push(rest);
    }
    Streams { inputs, outputs }
}

/// C# `ParseStreamForTesselator`: contours of (x, y) points.
fn parse_contours(stream: &[String]) -> Vec<Vec<f64>> {
    let mut contours = Vec::new();
    let mut current = Vec::new();
    let mut i = 0;
    while i < stream.len() {
        match stream[i].as_str() {
            "BP" | "EP" => {}
            "BC" => current = Vec::new(),
            "V" => {
                current.push(stream[i + 1].parse::<f64>().unwrap());
                current.push(stream[i + 2].parse::<f64>().unwrap());
                i += 2;
            }
            "EC" => contours.push(std::mem::take(&mut current)),
            other => panic!("unknown instruction {other}"),
        }
        i += 1;
    }
    contours
}

/// What one edge-flag run of the C# test expects: the combined vertices'
/// coordinates in callback order, and each emitted (vertex, edge flag).
struct Expected {
    combined: Vec<(f64, f64)>,
    corners: Vec<(u32, bool)>,
}

fn parse_expected(stream: &[String]) -> Expected {
    let mut combined = Vec::new();
    let mut corners = Vec::new();
    let mut edge = false;
    let mut i = 0;
    while i < stream.len() {
        match stream[i].as_str() {
            "C" => {
                // coords3[0], coords3[1], data4[0..4], weight4[0..4]
                combined.push((
                    stream[i + 1].parse().unwrap(),
                    stream[i + 2].parse().unwrap(),
                ));
                i += 10;
            }
            "B" => {
                assert_eq!(
                    stream[i + 1],
                    "TRI",
                    "edge-flag runs emit separate triangles"
                );
                i += 1;
            }
            "F" => {
                edge = stream[i + 1] == "1";
                i += 1;
            }
            "V" => {
                corners.push((stream[i + 1].parse().unwrap(), edge));
                i += 1;
            }
            "E" => {}
            other => panic!("unknown output token {other}"),
        }
        i += 1;
    }
    Expected { combined, corners }
}

const WINDING_RULES: [WindingRule; 5] = [
    WindingRule::Odd,
    WindingRule::NonZero,
    WindingRule::Positive,
    WindingRule::Negative,
    WindingRule::AbsGeqTwo,
];

#[test]
fn matches_glu_tesselator() {
    let streams = load_streams();
    assert_eq!(streams.inputs.len(), 4);
    assert_eq!(streams.outputs.len(), 40);

    let mut runs = 0;
    for (input_index, input) in streams.inputs.iter().enumerate() {
        let contours = parse_contours(input);
        let input_count: usize = contours.iter().map(|c| c.len() / 2).sum();
        for (rule_index, &rule) in WINDING_RULES.iter().enumerate() {
            // C# CurrentOutputTest: ten runs per input, the edge-flag five last.
            let output_index = input_index * 10 + 5 + rule_index;
            let expected = parse_expected(&streams.outputs[output_index]);
            let label = format!("input {input_index}, {rule:?}, edge flag");

            let mut tess = Tessellator::new();
            for contour in &contours {
                tess.add_contour(2, contour);
            }
            assert!(
                tess.tessellate(rule, ElementType::Polygons, 3, 2, None),
                "{label}"
            );
            let order = tess.render_order().expect("triangles were requested");

            assert_eq!(
                order.vertices.len(),
                input_count + expected.combined.len(),
                "{label}: combine callbacks"
            );
            for (k, &(x, y)) in expected.combined.iter().enumerate() {
                let v = order.vertices[input_count + k];
                assert!(
                    (v[0] - x).abs() < EPSILON && (v[1] - y).abs() < EPSILON,
                    "{label}: combine {k} at ({}, {}), expected ({x}, {y})",
                    v[0],
                    v[1]
                );
            }

            let corners: Vec<(u32, bool)> =
                order.indices.iter().map(|r| (r.index, r.is_edge)).collect();
            assert_eq!(corners, expected.corners, "{label}: emitted corners");
            runs += 1;
        }
    }
    assert_eq!(runs, 20);
}
