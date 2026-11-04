//! `apint`: the wide division family and signed overflow checks at 256 bits.

use super::*;

fn big(
    hi: u128,
    lo: u128,
) -> U256 {
    (U256::from_u128(hi) << 128) | U256::from_u128(lo)
}

/// `a == q * b + r` and `r < b` for operands above 128 bits.
#[test]
fn a_wide_division_reconstructs_the_dividend() {
    let a = big(0xdead_beef_u128 << 64 | 7, 0x1234_5678_9abc_def0_u128 << 64 | 99);
    for b in [U256::from_u128(3), big(0, u128::MAX), big(1, 5), big(u128::MAX >> 1, 17), big(u128::MAX, u128::MAX)] {
        let (q, r) = a.divrem(b);
        assert!(r < b, "{r:?} < {b:?}");
        assert_eq!(q.wrapping_mul(b).wrapping_add(r), a, "{a:?} / {b:?}");
    }
}

/// A divisor of 2^255 or more needed a remainder step past 256 bits.
#[test]
fn a_divisor_above_two_to_the_255_divides() {
    let b = big(1 << 127 | 3, 0);
    assert_eq!((!U256::ZERO).divrem(b), (U256::ONE, (!U256::ZERO).wrapping_sub(b)));
}

/// The signed family at width 256 agrees with the same values at width 8.
#[test]
fn the_signed_family_at_256_matches_8() {
    let neg = |m: u128| U256::ZERO.wrapping_sub(U256::from_u128(m));
    // -7 / 2 = -3, -7 % 2 = -1, rounding down -4, rounding up -3.
    assert_eq!(sdiv_word(neg(7), U256::from_u128(2), 256), neg(3));
    assert_eq!(srem_word(neg(7), U256::from_u128(2), 256), neg(1));
    assert_eq!(rounding_sdiv_word(neg(7), U256::from_u128(2), 256, false), neg(4));
    assert_eq!(rounding_sdiv_word(neg(7), U256::from_u128(2), 256, true), neg(3));
}

#[test]
fn signed_overflow_at_256() {
    let max = !U256::ZERO >> 1;
    let min = U256::ONE << 255;
    assert_eq!(sadd_ov_word(max, U256::ONE, 256), (min, true));
    assert_eq!(ssub_ov_word(min, U256::ONE, 256), (max, true));
    assert_eq!(smul_ov_word(max, U256::from_u128(2), 256).1, true);
    assert_eq!(smul_ov_word(min, !U256::ZERO, 256).1, true);
    assert_eq!(smul_ov_word(min, U256::ONE, 256), (min, false));
}
