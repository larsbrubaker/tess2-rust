// Copyright 2025 Lars Brubaker
// Tests for the exact in-circle predicate (`predicates.rs`) against integer arithmetic.

use super::in_circle_sign;

/// The in-circle determinant of integer points, computed exactly in i128.
fn integer_in_circle(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> i32 {
    let (adx, ady) = ((a[0] - d[0]) as i128, (a[1] - d[1]) as i128);
    let (bdx, bdy) = ((b[0] - d[0]) as i128, (b[1] - d[1]) as i128);
    let (cdx, cdy) = ((c[0] - d[0]) as i128, (c[1] - d[1]) as i128);
    let det = (adx * adx + ady * ady) * (bdx * cdy - cdx * bdy)
        + (bdx * bdx + bdy * bdy) * (cdx * ady - adx * cdy)
        + (cdx * cdx + cdy * cdy) * (adx * bdy - bdx * ady);
    det.signum() as i32
}

fn as_f64(p: [i64; 2], scale: f64) -> [f64; 2] {
    [p[0] as f64 * scale, p[1] as f64 * scale]
}

/// A small deterministic generator, so a failure always reproduces.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn coord(&mut self, offset: i64, spread: u64) -> i64 {
        offset + (self.next() % spread) as i64
    }
}

#[test]
fn points_on_one_circle_give_zero_even_far_from_the_origin() {
    // 3-4-5 points on a circle of radius 5, moved far away where the f64 formula rounds.
    let circle = [[5, 0], [3, 4], [-3, 4], [0, -5], [-4, -3]];
    for offset in [0i64, 1_000, 123_457, 10_000_001, 3_000_000_017] {
        let p = |i: usize| [circle[i][0] + offset, circle[i][1] - offset];
        for (i, j, k, l) in [(0, 1, 2, 3), (1, 2, 3, 4), (4, 0, 1, 2), (3, 2, 1, 0)] {
            assert_eq!(
                in_circle_sign(
                    as_f64(p(i), 1.0),
                    as_f64(p(j), 1.0),
                    as_f64(p(k), 1.0),
                    as_f64(p(l), 1.0)
                ),
                0,
                "offset {offset}, points {i} {j} {k} {l}"
            );
        }
    }
}

#[test]
fn the_sign_matches_exact_integer_arithmetic() {
    let mut rng = Lcg(0x5eed);
    // Small spreads around big offsets make most quads nearly cocircular, which is where the
    // rounded determinant cannot be trusted and the exact path decides.
    for (offset, spread) in [
        (0i64, 7u64),
        (100_000, 5),
        (40_000_000, 9),
        (-7_000_000, 1_000),
    ] {
        for _ in 0..20_000 {
            let mut p = || [rng.coord(offset, spread), rng.coord(-offset, spread)];
            let (a, b, c, d) = (p(), p(), p(), p());
            let expected = integer_in_circle(a, b, c, d);
            // Scaling by a power of two keeps every value exact, so fractional inputs are
            // covered with the same ground truth.
            for scale in [1.0, 0.0009765625, 1024.0] {
                assert_eq!(
                    in_circle_sign(
                        as_f64(a, scale),
                        as_f64(b, scale),
                        as_f64(c, scale),
                        as_f64(d, scale)
                    ),
                    expected,
                    "{a:?} {b:?} {c:?} {d:?} scale {scale}"
                );
            }
        }
    }
}

#[test]
fn a_point_inside_a_counter_clockwise_circle_is_positive() {
    let (a, b, c) = ([0.0, 0.0], [2.0, 0.0], [0.0, 2.0]);
    assert_eq!(in_circle_sign(a, b, c, [0.5, 0.5]), 1);
    assert_eq!(in_circle_sign(a, b, c, [3.0, 3.0]), -1);
    assert_eq!(in_circle_sign(a, c, b, [0.5, 0.5]), -1);
}
