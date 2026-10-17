//! The `APInt` operations the ValueTracking translations need, on a `u128` and
//! its bit width: `v` holds the low `width` bits and no others.
//!
//! # Example
//! `countl_one(0b1110, 4)` is 3; `signed(0b1110, 4)` is -2.

use std::cmp::min;

use crate::context::mask;

/// `v` with its top `count` of `width` bits set.
pub(crate) fn set_high(
    v: u128,
    width: u32,
    count: u32,
) -> u128 {
    v | (mask(width) & !mask(width - min(count, width)))
}

/// `v` with bits `[low, high)` set.
pub(crate) fn set_range(
    v: u128,
    low: u32,
    high: u32,
) -> u128 {
    if high <= low { v } else { v | (mask(high) & !mask(low)) }
}

pub(crate) fn clear_range(
    v: u128,
    low: u32,
    high: u32,
) -> u128 {
    if high <= low { v } else { v & !(mask(high) & !mask(low)) }
}

pub(crate) fn bit(
    v: u128,
    at: u32,
) -> bool {
    at < 128 && v >> at & 1 == 1
}

pub(crate) fn with_bit(
    v: u128,
    at: u32,
    on: bool,
) -> u128 {
    if on { v | 1_u128 << at } else { v & !(1_u128 << at) }
}

pub(crate) fn countl_zero(
    v: u128,
    width: u32,
) -> u32 {
    if v == 0 { width } else { v.leading_zeros() - (128 - width) }
}

pub(crate) fn countl_one(
    v: u128,
    width: u32,
) -> u32 {
    countl_zero(!v & mask(width), width)
}

pub(crate) fn countr_zero(
    v: u128,
    width: u32,
) -> u32 {
    if v == 0 { width } else { v.trailing_zeros() }
}

pub(crate) fn countr_one(
    v: u128,
    width: u32,
) -> u32 {
    countr_zero(!v & mask(width), width)
}

pub(crate) fn popcount(v: u128) -> u32 {
    v.count_ones()
}

pub(crate) fn sign_bit(width: u32) -> u128 {
    1_u128 << (width - 1)
}

pub(crate) fn signed(
    v: u128,
    width: u32,
) -> i128 {
    if width >= 128 { v as i128 } else { ((v << (128 - width)) as i128) >> (128 - width) }
}

pub(crate) fn sext(
    v: u128,
    from: u32,
    to: u32,
) -> u128 {
    signed(v, from) as u128 & mask(to)
}

pub(crate) fn ashr(
    v: u128,
    width: u32,
    by: u32,
) -> u128 {
    (signed(v, width) >> min(by, width - 1)) as u128 & mask(width)
}

pub(crate) fn lshr(
    v: u128,
    by: u32,
) -> u128 {
    if by >= 128 { 0 } else { v >> by }
}

/// `APInt::ushl_ov`: the shift, and whether a set bit left the width.
pub(crate) fn ushl_ov(
    v: u128,
    width: u32,
    by: u32,
) -> (u128, bool) {
    if by >= width {
        return (0, true);
    }
    ((v << by) & mask(width), by > countl_zero(v, width))
}

pub(crate) fn uadd_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    let (sum, over) = a.overflowing_add(b);
    if width >= 128 { (sum, over) } else { (sum & mask(width), over || sum > mask(width)) }
}

pub(crate) fn usub_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    (a.wrapping_sub(b) & mask(width), a < b)
}

pub(crate) fn umul_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    match a.checked_mul(b) {
        Some(product) if product <= mask(width) => (product, false),
        _ => (a.wrapping_mul(b) & mask(width), true),
    }
}

pub(crate) fn smin_of(width: u32) -> i128 {
    if width >= 128 { i128::MIN } else { -(1_i128 << (width - 1)) }
}

pub(crate) fn smax_of(width: u32) -> i128 {
    if width >= 128 { i128::MAX } else { (1_i128 << (width - 1)) - 1 }
}

pub(crate) fn saturated(
    sum: Option<i128>,
    width: u32,
    negative: bool,
) -> u128 {
    let value = match sum {
        Some(s) if s < smin_of(width) => smin_of(width),
        Some(s) if s > smax_of(width) => smax_of(width),
        Some(s) => s,
        None if negative => smin_of(width),
        None => smax_of(width),
    };
    value as u128 & mask(width)
}

pub(crate) fn sadd_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    saturated(x.checked_add(y), width, x < 0)
}

pub(crate) fn ssub_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    saturated(x.checked_sub(y), width, x < 0)
}

pub(crate) fn uadd_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (sum, over) = uadd_ov(a, b, width);
    if over { mask(width) } else { sum }
}

pub(crate) fn usub_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (diff, over) = usub_ov(a, b, width);
    if over { 0 } else { diff }
}

/// `APIntOps::fshl`: the top `width` bits of `(a:b) << (amt mod width)`.
pub(crate) fn fshl(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { a } else { ((a << by) | lshr(b, width - by)) & mask(width) }
}

/// `APIntOps::fshr`: the low `width` bits of `(a:b) >> (amt mod width)`.
pub(crate) fn fshr(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { b } else { ((a << (width - by)) | lshr(b, by)) & mask(width) }
}

pub(crate) fn clmul(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    (0..width).filter(|i| bit(b, *i)).fold(0, |acc, i| acc ^ (a << i)) & mask(width)
}

pub(crate) fn rotate_amount(
    amt: u128,
    width: u32,
) -> u32 {
    (amt % u128::from(width)) as u32
}

/// `APInt::getActiveBits`: the bits up to the highest set one.
pub(crate) fn active_bits(v: u128) -> u32 {
    128 - v.leading_zeros()
}

/// `APInt::getSignificantBits`: the bits of the narrowest signed integer
/// holding `v`.
pub(crate) fn significant_bits(
    v: u128,
    width: u32,
) -> u32 {
    let sign_bits = if bit(v, width - 1) { countl_one(v, width) } else { countl_zero(v, width) };
    width - sign_bits + 1
}

/// `APInt::smul_ov`.
pub(crate) fn smul_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    let (x, y) = (signed(a, width), signed(b, width));
    match x.checked_mul(y) {
        Some(p) if (smin_of(width)..=smax_of(width)).contains(&p) => (p as u128 & mask(width), false),
        _ => (x.wrapping_mul(y) as u128 & mask(width), true),
    }
}

/// `APInt::umul_sat`.
pub(crate) fn umul_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (product, over) = umul_ov(a, b, width);
    if over { mask(width) } else { product }
}

/// `APInt::smul_sat`.
pub(crate) fn smul_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (product, over) = smul_ov(a, b, width);
    if !over {
        return product;
    }
    let negative = bit(a, width - 1) != bit(b, width - 1);
    (if negative { smin_of(width) } else { smax_of(width) }) as u128 & mask(width)
}

/// `APInt::shl(unsigned)`.
pub(crate) fn shl(
    v: u128,
    width: u32,
    by: u32,
) -> u128 {
    if by >= width { 0 } else { (v << by) & mask(width) }
}

/// `APInt::sshl_ov`: the shift, and whether it changed the sign or lost a bit.
pub(crate) fn sshl_ov(
    v: u128,
    width: u32,
    by: u32,
) -> (u128, bool) {
    if by >= width {
        return (0, true);
    }
    let over = if bit(v, width - 1) { by >= countl_one(v, width) } else { by >= countl_zero(v, width) };
    (shl(v, width, by), over)
}

/// `APInt::ushl_sat`.
pub(crate) fn ushl_sat(
    v: u128,
    width: u32,
    by: u32,
) -> u128 {
    let (shifted, over) = ushl_ov(v, width, by);
    if over { mask(width) } else { shifted }
}

/// `APInt::sshl_sat`.
pub(crate) fn sshl_sat(
    v: u128,
    width: u32,
    by: u32,
) -> u128 {
    let (shifted, over) = sshl_ov(v, width, by);
    if !over {
        return shifted;
    }
    (if bit(v, width - 1) { smin_of(width) } else { smax_of(width) }) as u128 & mask(width)
}

/// `APInt::sdiv`: the quotient rounded towards zero (`INT_MIN / -1` wraps).
pub(crate) fn sdiv(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    signed(a, width).wrapping_div(signed(b, width)) as u128 & mask(width)
}

/// `APInt::srem`: the remainder with the sign of the dividend.
pub(crate) fn srem(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    signed(a, width).wrapping_rem(signed(b, width)) as u128 & mask(width)
}

/// `APIntOps::RoundingUDiv`.
pub(crate) fn rounding_udiv(
    a: u128,
    b: u128,
    up: bool,
) -> u128 {
    let (quotient, rem) = (a / b, a % b);
    if up && rem != 0 { quotient + 1 } else { quotient }
}

/// `APIntOps::RoundingSDiv`, rounding up or down.
pub(crate) fn rounding_sdiv(
    a: u128,
    b: u128,
    width: u32,
    up: bool,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    let quotient = x.wrapping_div(y);
    let rem = x.wrapping_rem(y);
    let result = if rem == 0 {
        quotient
    } else if up {
        if (rem < 0) != (y < 0) { quotient } else { quotient + 1 }
    } else if (rem < 0) != (y < 0) {
        quotient - 1
    } else {
        quotient
    };
    result as u128 & mask(width)
}

/// `APInt::sqrtFloor`.
pub(crate) fn sqrt_floor(v: u128) -> u128 {
    let (mut low, mut high) = (0_u128, 1_u128 << 64);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid.checked_mul(mid).is_some_and(|square| square <= v) { low = mid } else { high = mid }
    }
    low
}

/// `APInt::getLimitedValue`.
pub(crate) fn limited(
    v: u128,
    limit: u32,
) -> u32 {
    min(v, u128::from(limit)) as u32
}
