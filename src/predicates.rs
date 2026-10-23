// Copyright 2025 Lars Brubaker
// Exact geometric predicates for the Delaunay refinement (`mesh/delaunay.rs`).
//
// The in-circle test decides which diagonal of a quad a Delaunay triangulation keeps. Evaluated
// in plain f64, its result is rounded: for four points on (or very near) one circle the rounded
// value can say "flip" for both diagonals, and the refinement then flips the same edge back and
// forth until its iteration cap. Points on a common circle are everywhere in real input - any
// outline traced on a pixel grid, scaled onto an integer grid, is full of them. This module
// returns the exact sign of the in-circle determinant for the f64 inputs it is given.
//
// It evaluates the determinant in f64 first and trusts that value when its magnitude exceeds a
// bound on the rounding error, which is nearly always. Only when the rounded value is too close
// to zero does it recompute the determinant exactly with floating-point expansions: sums of f64
// terms whose exact total is the value, built with error-free addition and multiplication. The
// error bound and the expansion arithmetic follow J. R. Shewchuk, "Adaptive Precision
// Floating-Point Arithmetic and Fast Robust Geometric Predicates" (1997). No fused multiply-add
// is used: every step relies on separately rounded products and sums.

use crate::geom::Real;

/// Half an ulp of 1.0: the relative rounding error of one f64 operation.
const EPSILON: Real = f64::EPSILON * 0.5;

/// Splits an f64 into two halves of 26 bits each so their products are exact.
const SPLITTER: Real = 134_217_729.0; // 2^27 + 1

/// Relative error bound of the f64 in-circle determinant below. When the rounded determinant
/// exceeds this times the sum of the absolute values of its terms, its sign is right.
const IN_CIRCLE_ERROR_BOUND: Real = (10.0 + 96.0 * EPSILON) * EPSILON;

/// The sign of the in-circle determinant of `d` against the circle through `a`, `b`, `c`:
/// `1` when `d` lies strictly inside that circle and `a`, `b`, `c` run counter-clockwise
/// (strictly outside when they run clockwise), `-1` for the opposite, and `0` exactly when the
/// four points lie on one circle (or `a`, `b`, `c` are collinear with `d` on their line).
///
/// The sign is exact for any finite inputs whose products neither overflow nor underflow.
pub fn in_circle_sign(a: [Real; 2], b: [Real; 2], c: [Real; 2], d: [Real; 2]) -> i32 {
    let adx = a[0] - d[0];
    let ady = a[1] - d[1];
    let bdx = b[0] - d[0];
    let bdy = b[1] - d[1];
    let cdx = c[0] - d[0];
    let cdy = c[1] - d[1];

    let bdx_cdy = bdx * cdy;
    let cdx_bdy = cdx * bdy;
    let a_lift = adx * adx + ady * ady;

    let cdx_ady = cdx * ady;
    let adx_cdy = adx * cdy;
    let b_lift = bdx * bdx + bdy * bdy;

    let adx_bdy = adx * bdy;
    let bdx_ady = bdx * ady;
    let c_lift = cdx * cdx + cdy * cdy;

    let det =
        a_lift * (bdx_cdy - cdx_bdy) + b_lift * (cdx_ady - adx_cdy) + c_lift * (adx_bdy - bdx_ady);
    let permanent = (bdx_cdy.abs() + cdx_bdy.abs()) * a_lift
        + (cdx_ady.abs() + adx_cdy.abs()) * b_lift
        + (adx_bdy.abs() + bdx_ady.abs()) * c_lift;
    let bound = IN_CIRCLE_ERROR_BOUND * permanent;
    if det > bound {
        return 1;
    }
    if -det > bound {
        return -1;
    }
    in_circle_exact_sign(a, b, c, d)
}

/// The in-circle determinant computed exactly as an expansion; returns its sign.
fn in_circle_exact_sign(a: [Real; 2], b: [Real; 2], c: [Real; 2], d: [Real; 2]) -> i32 {
    // Each coordinate difference is exactly the sum of two f64s.
    let adx = two_diff(a[0], d[0]);
    let ady = two_diff(a[1], d[1]);
    let bdx = two_diff(b[0], d[0]);
    let bdy = two_diff(b[1], d[1]);
    let cdx = two_diff(c[0], d[0]);
    let cdy = two_diff(c[1], d[1]);

    let lift = |dx: &[Real], dy: &[Real]| expansion_sum(&product(dx, dx), &product(dy, dy));
    let cross = |px: &[Real], py: &[Real], qx: &[Real], qy: &[Real]| {
        expansion_sum(&product(px, qy), &negated(&product(qx, py)))
    };

    let a_term = product(&lift(&adx, &ady), &cross(&bdx, &bdy, &cdx, &cdy));
    let b_term = product(&lift(&bdx, &bdy), &cross(&cdx, &cdy, &adx, &ady));
    let c_term = product(&lift(&cdx, &cdy), &cross(&adx, &ady, &bdx, &bdy));
    let det = expansion_sum(&expansion_sum(&a_term, &b_term), &c_term);

    // An expansion's terms do not overlap and grow in magnitude, so its largest term has the
    // sign of the whole sum. Zero terms are never stored, so an empty expansion is zero.
    match det.last() {
        Some(&t) if t > 0.0 => 1,
        Some(&t) if t < 0.0 => -1,
        _ => 0,
    }
}

/// `a + b` as `(rounded sum, rounding error)`: the two add up to the exact sum.
fn two_sum(a: Real, b: Real) -> (Real, Real) {
    let x = a + b;
    let b_virtual = x - a;
    let a_virtual = x - b_virtual;
    let b_round = b - b_virtual;
    let a_round = a - a_virtual;
    (x, a_round + b_round)
}

/// `a - b` as an expansion (smallest term first, zeros left out).
fn two_diff(a: Real, b: Real) -> Vec<Real> {
    let x = a - b;
    let b_virtual = a - x;
    let a_virtual = x + b_virtual;
    let b_round = b_virtual - b;
    let a_round = a - a_virtual;
    without_zeros(&[a_round + b_round, x])
}

/// `a` split into a high and a low half, each with at most 26 significant bits.
fn split(a: Real) -> (Real, Real) {
    let c = SPLITTER * a;
    let a_big = c - a;
    let hi = c - a_big;
    (hi, a - hi)
}

/// `a * b` as `(rounded product, rounding error)`: the two add up to the exact product.
fn two_product(a: Real, b: Real) -> (Real, Real) {
    let x = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let err1 = x - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    (x, a_lo * b_lo - err3)
}

fn without_zeros(terms: &[Real]) -> Vec<Real> {
    terms.iter().copied().filter(|&t| t != 0.0).collect()
}

fn negated(e: &[Real]) -> Vec<Real> {
    e.iter().map(|&t| -t).collect()
}

/// The exact sum of two expansions, as an expansion with zero terms left out. The terms of both
/// are merged in order of magnitude and accumulated with error-free additions.
fn expansion_sum(e: &[Real], f: &[Real]) -> Vec<Real> {
    let mut merged = Vec::with_capacity(e.len() + f.len());
    let (mut i, mut j) = (0, 0);
    while i < e.len() && j < f.len() {
        // Ties take from `e` first; either order gives the same exact sum.
        if f[j].abs() > e[i].abs() {
            merged.push(e[i]);
            i += 1;
        } else {
            merged.push(f[j]);
            j += 1;
        }
    }
    merged.extend_from_slice(&e[i..]);
    merged.extend_from_slice(&f[j..]);

    let mut out = Vec::with_capacity(merged.len());
    let Some((&first, rest)) = merged.split_first() else {
        return out;
    };
    let mut q = first;
    for &g in rest {
        let (sum, err) = two_sum(q, g);
        if err != 0.0 {
            out.push(err);
        }
        q = sum;
    }
    if q != 0.0 {
        out.push(q);
    }
    out
}

/// The exact product of an expansion and one f64, as an expansion with zero terms left out.
fn scale_expansion(e: &[Real], b: Real) -> Vec<Real> {
    let mut out = Vec::with_capacity(2 * e.len());
    let Some((&first, rest)) = e.split_first() else {
        return out;
    };
    let (mut q, err) = two_product(first, b);
    if err != 0.0 {
        out.push(err);
    }
    for &t in rest {
        let (p_hi, p_lo) = two_product(t, b);
        let (sum, err) = two_sum(q, p_lo);
        if err != 0.0 {
            out.push(err);
        }
        let (next, err) = two_sum(p_hi, sum);
        if err != 0.0 {
            out.push(err);
        }
        q = next;
    }
    if q != 0.0 {
        out.push(q);
    }
    out
}

/// The exact product of two expansions.
fn product(e: &[Real], f: &[Real]) -> Vec<Real> {
    f.iter().fold(Vec::new(), |acc, &t| {
        expansion_sum(&acc, &scale_expansion(e, t))
    })
}

#[cfg(test)]
mod tests;
