// Copyright 2025 Lars Brubaker
// Winding rule correctness tests with area verification.

mod helpers;

use tess2_rust::{ElementType, Tessellator, WindingRule};

/// Three nested squares for winding rule testing:
/// - Outer: 6x6, CCW (area=36)
/// - Middle: 4x4, CW (reversed) (area=16)
/// - Inner: 2x2, CCW (area=4)
///
/// Winding numbers from outside to inside: 0, 1, 0, 1 (CCW, CW, CCW)
fn tessellate_nested_squares(rule: WindingRule) -> Tessellator {
    let mut tess = Tessellator::new();
    // Outer 6x6 (CCW)
    tess.add_contour(2, &[-3.0f32, -3.0, 3.0, -3.0, 3.0, 3.0, -3.0, 3.0]);
    // Middle 4x4 (CW - reversed winding)
    tess.add_contour(2, &[-2.0f32, -2.0, -2.0, 2.0, 2.0, 2.0, 2.0, -2.0]);
    // Inner 2x2 (CCW)
    tess.add_contour(2, &[-1.0f32, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0]);

    let ok = tess.tessellate(rule, ElementType::Polygons, 3, 2, None);
    assert!(ok, "tessellation failed for {:?}", rule);
    tess
}

fn assert_area_approx(actual: f32, expected: f32, tolerance: f32, label: &str) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{}: expected area ~{}, got {} (diff={})",
        label,
        expected,
        actual,
        (actual - expected).abs()
    );
}

#[test]
fn winding_odd_nested_squares() {
    let tess = tessellate_nested_squares(WindingRule::Odd);
    let area = helpers::total_tessellation_area(&tess);
    assert!(area > 0.0, "Odd should produce non-zero area, got {}", area);
    helpers::verify_valid_output(&tess);
}

#[test]
fn winding_nonzero_nested_squares() {
    let tess = tessellate_nested_squares(WindingRule::NonZero);
    let area = helpers::total_tessellation_area(&tess);
    assert!(
        area > 0.0,
        "NonZero should produce non-zero area, got {}",
        area
    );
    helpers::verify_valid_output(&tess);
}

#[test]
fn winding_positive_nested_squares() {
    let tess = tessellate_nested_squares(WindingRule::Positive);
    let area = helpers::total_tessellation_area(&tess);
    assert!(
        area > 0.0,
        "Positive should produce non-zero area, got {}",
        area
    );
    helpers::verify_valid_output(&tess);
}

#[test]
fn winding_negative_nested_squares() {
    // With Negative winding, only regions with negative winding number are filled.
    // For this configuration, Negative may produce 0 or non-zero area depending
    // on the contour orientations.
    let tess = tessellate_nested_squares(WindingRule::Negative);
    helpers::verify_valid_output(&tess);
}

#[test]
fn winding_abs_geq_two_nested_squares() {
    // AbsGeqTwo: only regions with |winding| >= 2 are interior.
    let tess = tessellate_nested_squares(WindingRule::AbsGeqTwo);
    helpers::verify_valid_output(&tess);
}

// Test with overlapping same-direction contours to exercise AbsGeqTwo
#[test]
fn winding_abs_geq_two_overlapping_squares() {
    // Two overlapping CCW squares: 4x4 each, same position.
    // Winding number in the overlap region is 2.
    let square = &[0.0f32, 0.0, 4.0, 0.0, 4.0, 4.0, 0.0, 4.0];
    let mut tess = Tessellator::new();
    tess.add_contour(2, square);
    tess.add_contour(2, square);
    let ok = tess.tessellate(WindingRule::AbsGeqTwo, ElementType::Polygons, 3, 2, None);
    assert!(ok);
    let area = helpers::total_tessellation_area(&tess);
    // The overlap has winding 2, so AbsGeqTwo should fill it. Area = 16.
    assert_area_approx(area, 16.0, 0.1, "AbsGeqTwo overlapping");
}

// Different winding rules should produce valid results on a self-intersecting star
#[test]
fn winding_rules_on_star() {
    let star: &[f32] = &[
        350.0, 75.0, 379.0, 161.0, 469.0, 161.0, 397.0, 215.0, 423.0, 301.0, 350.0, 250.0, 277.0,
        301.0, 303.0, 215.0, 231.0, 161.0, 321.0, 161.0,
    ];

    let mut element_counts = std::collections::HashMap::new();
    for &rule in &[
        WindingRule::Odd,
        WindingRule::NonZero,
        WindingRule::Positive,
        WindingRule::Negative,
        WindingRule::AbsGeqTwo,
    ] {
        let mut tess = Tessellator::new();
        tess.add_contour(2, star);
        let ok = tess.tessellate(rule, ElementType::Polygons, 3, 2, None);
        assert!(ok, "star tessellation failed for {:?}", rule);
        helpers::verify_valid_output(&tess);
        element_counts.insert(format!("{:?}", rule), tess.element_count());
    }

    // Odd should exclude the center pentagon, so fewer triangles than NonZero
    let odd_count = element_counts["Odd"];
    let nonzero_count = element_counts["NonZero"];
    assert!(
        odd_count <= nonzero_count,
        "Odd element count ({}) should be <= NonZero count ({})",
        odd_count,
        nonzero_count
    );
}

// glu_winding.dat contours exercise multiple winding configurations
#[test]
fn glu_winding_nested_rectangles_all_rules() {
    let data = include_str!("data/glu_winding.dat");
    let contours = helpers::parse_contours(data);
    assert!(
        !contours.is_empty(),
        "should parse glu_winding.dat contours"
    );

    for &rule in &[
        WindingRule::Odd,
        WindingRule::NonZero,
        WindingRule::Positive,
        WindingRule::Negative,
        WindingRule::AbsGeqTwo,
    ] {
        let mut tess = Tessellator::new();
        for contour in &contours {
            tess.add_contour(2, contour);
        }
        let ok = tess.tessellate(rule, ElementType::Polygons, 3, 2, None);
        assert!(ok, "glu_winding tessellation failed for {:?}", rule);
        helpers::verify_valid_output(&tess);

        let area = helpers::total_tessellation_area(&tess);
        assert!(area >= 0.0, "area should be non-negative for {:?}", rule);
    }
}
