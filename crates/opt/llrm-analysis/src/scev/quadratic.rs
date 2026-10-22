//! Quadratic recurrences: the least `n` at which `{L,+,M,+,N}` is zero or
//! leaves a range. Translated from `ScalarEvolution.cpp` at d1106deb71cc
//! (`GetQuadraticEquation`, `SolveQuadraticAddRecExact`,
//! `SolveQuadraticAddRecRange`, `TruncIfPossible`, `MinOptional`) and
//! `APInt.cpp` (`SolveQuadraticEquationWrap`).
//!
//! # How it works
//! The accumulated value after `n` iterations is `L + nM + n(n-1)/2 N`, so
//! `Acc = 0` is `N n^2 + (2M-N) n + 2L = 0`. The equation is solved over the
//! integers, with the wrap of the range width as the shifted parabola
//! `q(x) = kR`, `R = 2^RangeWidth` ([`solve_quadratic_equation_wrap`]).
//!
//! DEVIATE Q1 (no number yet): LLVM computes in `APInt`s of three times the
//! coefficient width; here in `i128`, so a coefficient width above 41 bits
//! (3 x (width + 1) > 127) has no solution found (`None`, as when LLVM's finds
//! none). Exit: arbitrary-width `Ap`. Owner w-5. Review 2026-11-27.
//!
//! # Example
//! `{0,+,1,+,2}` is `n*n`: zero at `n = 0`. `{-49,+,1,+,2}` (`n*n - 49`) is
//! zero at `n = 7`: the equation `2n^2 + 0n - 98 = 0` over the doubled
//! coefficients.

/// The root and the bit width of its `APInt`: the width of the coefficients
/// for the zero root, three times it otherwise.
pub type Solution = (i128, u32);

/// `RoundUp` of `SolveQuadraticEquationWrap`: `v` to a multiple of `a > 0`
/// toward positive infinity.
fn round_up(
    v: i128,
    a: i128,
) -> Option<i128> {
    let t = v.checked_abs()? % a;
    if t == 0 {
        Some(v)
    } else if v < 0 {
        v.checked_add(t)
    } else {
        v.checked_add(a - t)
    }
}

/// `APInt::sqrtFloor` of a non-negative `d`.
fn sqrt_floor(d: i128) -> Option<i128> {
    let mut x = (d as f64).sqrt() as i128;
    while x.checked_mul(x).is_none_or(|sq| sq > d) {
        x -= 1;
    }
    while (x + 1).checked_mul(x + 1).is_some_and(|sq| sq <= d) {
        x += 1;
    }
    Some(x)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APInt.cpp#L2850
/// `APIntOps::SolveQuadraticEquationWrap`: the least `x >= 0` at which
/// `a*x^2 + b*x + c`, in arithmetic modulo `2^range_width`, is zero or wraps
/// (`q(x) = kR`), given the coefficients as signed numbers of `coeff_width`
/// bits. `None` where there is none, or where the arithmetic leaves `i128`.
pub fn solve_quadratic_equation_wrap(
    a: i128,
    b: i128,
    c: i128,
    coeff_width: u32,
    range_width: u32,
) -> Option<Solution> {
    assert!(range_width <= coeff_width && range_width > 1);
    // Identify 0 as a (non)solution immediately.
    let low = if range_width >= 128 { c as u128 } else { (c as u128) & ((1u128 << range_width) - 1) };
    if low == 0 {
        return Some((0, coeff_width));
    }
    let width = coeff_width.checked_mul(3).filter(|width| *width <= 127)?;

    // Make A > 0 for simplicity.
    let (a, b, mut c) = if a < 0 { (a.checked_neg()?, b.checked_neg()?, c.checked_neg()?) } else { (a, b, c) };
    let r = 1i128 << range_width;
    let two_a = a.checked_mul(2)?;
    let sqr_b = b.checked_mul(b)?;
    let pick_low;
    if b >= 0 {
        // The vertex is at a negative location (or at 0): pick the k that
        // makes C - kR negative and closest to 0, and the greater solution.
        c %= r;
        if c > 0 {
            c -= r;
        }
        pick_low = false;
    } else {
        // For a solution to exist the discriminant must be non-negative:
        // kR >= C - B^2/4A.
        let low_kr = round_up(c.checked_sub(sqr_b / two_a.checked_mul(2)?)?, r)?;
        if c > low_kr {
            // Such a k exists: C = C - RoundDown(C, R); the smaller solution.
            c = c.checked_add(round_up(c.checked_neg()?, r)?)?;
            pick_low = true;
        } else {
            // One solution is negative, the other positive: the k farthest up.
            c = c.checked_sub(low_kr)?;
            pick_low = false;
        }
    }

    let d = sqr_b.checked_sub(a.checked_mul(4)?.checked_mul(c)?)?;
    if d < 0 {
        return None;
    }
    let sq = sqrt_floor(d)?;
    let inexact = sq * sq != d;

    // SQ is rounded down, so a low root is taken from SQ+1 when inexact.
    let numerator = if pick_low {
        b.checked_neg()?.checked_sub(sq + i128::from(inexact))?
    } else {
        b.checked_neg()?.checked_add(sq)?
    };
    let (x, rem) = (numerator / two_a, numerator % two_a);
    // The updated coefficients make the exact solution positive.
    if x < 0 {
        return None;
    }
    if !inexact && rem == 0 {
        return Some((x, width));
    }

    // The exact root lies between X and X+1: it must change the sign of q.
    let vx = a.checked_mul(x)?.checked_add(b)?.checked_mul(x)?.checked_add(c)?;
    let vy = vx.checked_add(two_a.checked_mul(x)?)?.checked_add(a)?.checked_add(b)?;
    let sign_change = (vx < 0) != (vy < 0) || (vx == 0) != (vy == 0);
    if !sign_change {
        return None;
    }
    Some((x + 1, width))
}

/// The coefficients of `{l,+,m,+,n}` (signed, `bit_width` bits) as the
/// equation `A x^2 + B x + C = 0` of `bit_width + 1` bits, and the
/// multiplier 2 (`GetQuadraticEquation`).
pub fn quadratic_equation(
    l: i128,
    m: i128,
    n: i128,
    bit_width: u32,
) -> Option<(i128, i128, i128, i128)> {
    if (bit_width + 1) * 3 > 127 {
        return None;
    }
    let a = n;
    let b = m.checked_mul(2)?.checked_sub(a)?;
    let c = l.checked_mul(2)?;
    Some((a, b, c, 2))
}

/// `MinOptional`: the smaller of two signed solutions, or the one there is.
pub fn min_optional(
    x: Option<Solution>,
    y: Option<Solution>,
) -> Option<Solution> {
    match (x, y) {
        (Some(x), Some(y)) => Some(if x.0 < y.0 { x } else { y }),
        (x, None) => x,
        (None, y) => y,
    }
}

/// `TruncIfPossible`: the solution in `bit_width` bits if it fits as a signed
/// number (and `bit_width > 1`), else as it is.
pub fn trunc_if_possible(
    x: Option<Solution>,
    bit_width: u32,
) -> Option<Solution> {
    let (value, width) = x?;
    if bit_width > 1 && bit_width < width {
        let (low, high) = (-(1i128 << (bit_width - 1)), (1i128 << (bit_width - 1)) - 1);
        if (low..=high).contains(&value) {
            return Some((value, bit_width));
        }
    }
    Some((value, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `{-49,+,1,+,2}` is `n*n - 49` (i32): zero at 7.
    #[test]
    fn a_square_minus_forty_nine_is_zero_at_seven() {
        let (a, b, c, _) = quadratic_equation(-49, 1, 2, 32).unwrap();
        assert_eq!(solve_quadratic_equation_wrap(a, b, c, 33, 33).map(|(x, _)| x), Some(7));
    }

    /// `n*n - 50` has no integer root and wraps nowhere sooner: the sign change
    /// at 7 -> 8 gives the wrap solution 8.
    #[test]
    fn a_square_minus_fifty_wraps_at_eight() {
        let (a, b, c, _) = quadratic_equation(-50, 1, 2, 32).unwrap();
        assert_eq!(solve_quadratic_equation_wrap(a, b, c, 33, 33).map(|(x, _)| x), Some(8));
    }

    #[test]
    fn a_coefficient_wider_than_forty_one_bits_has_no_solution() {
        assert_eq!(quadratic_equation(-49, 1, 2, 42), None);
    }
}
