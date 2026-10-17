//! Core-clock bounds of multiplies and divides, read from the CPU profile, so
//! the compiler can pick a multiply or divide form by cost.
//!
//! # How it works
//! A CPU description states a price per key (`smul_min_w4`, `smul_max_w2`,
//! `sdiv_w4`, `udiv_w2`, ...). This module reads them and returns `Clocks`, a
//! `minimum` and `maximum`, or none where the profile has no such key or the
//! width is not 2 or 4 bytes. A CPU the description does not price yields none,
//! and the caller does not use the form that needed it.
//!
//! A multiply that ends early on a short multiplier (the 486's `10 + max(bits,
//! 3)`) is priced by linear interpolation from the minimum at 3 bits to the
//! maximum at the full width. A negative multiplier takes the profile's
//! `smul_negative_bits` as the floor. An unknown multiplier is priced at the
//! middle of the range here; a division by a constant prices its unknown
//! dividend at the minimum, as gcc does (`division`).
//!
//! # Example
//! The bounds `signed_multiply` returns for a full product, in core clocks:
//!
//! ```text
//! cpu   width   minimum   maximum
//! 386   2       9         22
//! 386   4       9         38
//! 486   2       13        26
//! 486   4       13        42
//! P5    2       11        11
//! P5    4       10        10
//! ```
//!
//! A 486 dword multiply may take 42 clocks, so a reciprocal sequence for a
//! dividend known to need 32 bits is compared against 42.
//!
//! From unit tests `test_full_product_bounds_follow_operand_width` and
//! `test_486_reciprocal_is_priced_by_the_dividend_s_length_and_by_the_minimum_where_unknown`
//! in `timing.rs`.
//!
//! # Used by
//! [`crate::backend::division`] (multiply by reciprocal against a divide) and
//! [`crate::backend::arithmetic`] (constant multiply chains against `imul`).

use crate::backend::cpu::{self as targets, ProfileOrName};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Clocks {
    pub minimum: i64,
    pub maximum: i64,
}

/// The price a CPU states for `key`, or none where its description has none.
fn stated(
    profile: &targets::Profile,
    key: &str,
) -> Option<i64> {
    profile.cost(key).ok()
}

pub fn signed_multiply<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
    full: bool,
) -> Result<Option<Clocks>, String> {
    if ![2, 4].contains(&width) {
        return Ok(None);
    }
    let profile = targets::profile(cpu)?;
    let of = |what: &str| stated(profile, &format!("smul_{what}_w{width}"));
    if let (Some(minimum), Some(maximum)) = (of("min"), of("max")) {
        // Where the CPU prices the full product of a word apart, that figure is
        // both bounds.
        if let Some(clocks) = stated(profile, "smul_full_w2").filter(|_| full && width == 2) {
            return Ok(Some(Clocks { minimum: clocks, maximum: clocks }));
        }
        return Ok(Some(Clocks { minimum, maximum }));
    }
    Ok(None)
}

/// Clocks of a multiply whose multiplier has `bits` significant bits (`None`:
/// unknown), for a CPU whose multiply ends early on a short multiplier: the
/// stated minimum at 3 bits to the stated maximum at the full width, a step per
/// bit (the 486's `10 + max(bits, 3)`, Intel 240440-002 Table 10.1 note 3). A
/// CPU whose two bounds are one has no such dependence. An unknown multiplier
/// is priced at the middle of the range: the estimate with no information on
/// which bit length it has, an assumption and not a fact about any program.
pub fn multiply_clocks<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
    bits: Option<i64>,
) -> Result<Option<i64>, String> {
    multiply_clocks_of(cpu, width, bits, false)
}

/// `multiply_clocks`, for a multiplier of either sign: where the CPU's
/// description gives a negative one a higher floor
/// of bits (`smul_negative_bits`: the 486's `n = 5` against `3`), a negative
/// multiplier of fewer bits costs as one of that many.
pub fn multiply_clocks_of<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
    bits: Option<i64>,
    negative: bool,
) -> Result<Option<i64>, String> {
    let profile = targets::profile(cpu)?;
    let Some(Clocks { minimum, maximum }) = signed_multiply(profile, width, false)? else { return Ok(None) };
    let Some(bits) = bits else { return Ok(Some((minimum + maximum + 1) / 2)) };
    let floor = if negative { stated(profile, "smul_negative_bits").unwrap_or(3) } else { 3 };
    let bits = bits.clamp(floor, width * 8);
    Ok(Some(minimum + (maximum - minimum) * (bits - 3) / (width * 8 - 3)))
}

pub fn signed_divide<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
) -> Result<Option<Clocks>, String> {
    if ![2, 4].contains(&width) {
        return Ok(None);
    }
    let profile = targets::profile(cpu)?;
    Ok(stated(profile, &format!("sdiv_w{width}")).map(|clocks| Clocks { minimum: clocks, maximum: clocks }))
}

/// The unsigned divide's clocks, where the CPU's description has them.
pub fn unsigned_divide<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
) -> Result<Option<Clocks>, String> {
    if ![2, 4].contains(&width) {
        return Ok(None);
    }
    let profile = targets::profile(cpu)?;
    Ok(stated(profile, &format!("udiv_w{width}")).map(|clocks| Clocks { minimum: clocks, maximum: clocks }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::division;
    use crate::model::ir;

    #[test]
    fn test_full_product_bounds_follow_operand_width() {
        for (cpu, width, low, high) in [
            ("386", 2, 9, 22),
            ("386", 4, 9, 38),
            ("486", 2, 13, 26),
            ("486", 4, 13, 42),
            ("P5", 2, 11, 11),
            ("P5", 4, 10, 10),
        ] {
            assert_eq!(signed_multiply(cpu, width, true), Ok(Some(Clocks { minimum: low, maximum: high })));
        }
    }

    /// A dividend known to need 32 bits takes the 486's 42 clocks to multiply
    /// and keeps its division; one nothing is known of is priced at the
    /// early-out minimum, as gcc does (the midpoint, 26, once kept it too: it
    /// is a guess either way, and gcc's is the one the comparison is against).
    #[test]
    fn test_486_reciprocal_is_priced_by_the_dividend_s_length_and_by_the_minimum_where_unknown() {
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let held = ir::Held { value: 1, width: 4 };
        assert_eq!(division::reciprocal(held, 7, &results, &mut fresh, "486", true, Some(32)), Ok(None));
        assert!(division::reciprocal(held, 7, &results, &mut fresh, "486", true, None).unwrap().is_some());
    }

    #[test]
    fn test_unverified_p6_forms_are_not_silently_priced() {
        assert_eq!(signed_multiply("P6", 4, true), Ok(None));
        assert_eq!(signed_divide("P6", 4), Ok(None));
    }

    /// A quotient-only divide should not multiply back and subtract for a dead
    /// remainder.
    #[test]
    fn test_quotient_only_reciprocal_omits_remainder_reconstruction() {
        let (quotient, remainder) = (ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 });
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let held = ir::Held { value: 1, width: 4 };
        let parts = division::reciprocal(held, 7, &[quotient, remainder], &mut fresh, "P5", false, None)
            .unwrap()
            .expect("a reciprocal");
        assert_eq!(parts[parts.len() - 1].dests, [ir::Loc::Held(quotient)]);
        assert!(!parts.iter().any(|one| one.dests.contains(&ir::Loc::Held(remainder))));
        assert!(!parts.iter().any(|one| one.name.as_deref() == Some("sub")));
    }
}
