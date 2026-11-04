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
    assert_eq!(sdiv(neg(7), U256::from_u128(2), 256), neg(3));
    assert_eq!(srem(neg(7), U256::from_u128(2), 256), neg(1));
    assert_eq!(rounding_sdiv(neg(7), U256::from_u128(2), 256, false), neg(4));
    assert_eq!(rounding_sdiv(neg(7), U256::from_u128(2), 256, true), neg(3));
}

#[test]
fn signed_overflow_at_256() {
    let max = !U256::ZERO >> 1;
    let min = U256::ONE << 255;
    assert_eq!(sadd_ov(max, U256::ONE, 256), (min, true));
    assert_eq!(ssub_ov(min, U256::ONE, 256), (max, true));
    assert_eq!(smul_ov(max, U256::from_u128(2), 256).1, true);
    assert_eq!(smul_ov(min, !U256::ZERO, 256).1, true);
    assert_eq!(smul_ov(min, U256::ONE, 256), (min, false));
}

fn huge(limbs: [u128; 4]) -> U512 {
    U512::from_limbs(limbs)
}

/// A 512-bit word was the missing room for the product of two 256-bit
/// integers (`ConstantRange::multiply` of an `i256` range).
#[test]
fn a_512_bit_word_multiplies_divides_and_shifts() {
    let a = huge([0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321, 7, 1 << 100]);
    let b = huge([u128::MAX, 3, 0, 0]);
    let (q, r) = a.divrem(b);
    assert!(r < b);
    assert_eq!(q.wrapping_mul(b).wrapping_add(r), a);
    assert_eq!((a << 200 >> 200).to_limbs()[..2], a.to_limbs()[..2]);
    assert_eq!((a << 3).to_limbs()[0], a.to_limbs()[0] << 3);
    let ones = !U512::ZERO;
    assert_eq!(ones.overflowing_mul(ones), (U512::ONE, true));
    assert_eq!(U512::ONE.overflowing_mul(ones), (ones, false));
    assert_eq!(ones.leading_zeros(), 0);
    assert_eq!(U512::ONE.leading_zeros(), 511);
    assert_eq!((U512::ONE << 300).trailing_zeros(), 300);
    assert_eq!(ones.count_ones(), 512);
}

/// The 256-bit product of two `i128` extremes, in the word that holds it.
#[test]
fn the_product_of_two_256_bit_values_fits_512() {
    let a = !U256::ZERO;
    let wide = |v: U256| U512::from_limbs(v.to_limbs());
    let (product, over) = wide(a).overflowing_mul(wide(a));
    assert!(!over);
    // (2^256 - 1)^2 = 2^512 - 2^257 + 1.
    assert_eq!(product, (!U512::ZERO).wrapping_sub(U512::ONE << 257).wrapping_add(U512::from_u128(2)));
}

/// A mask and a range of a `U256` word cost shifts of the whole word (`mask`
/// was `(1 << bits) - 1`, `Range::new` took it four times): 13.5 M Ir of a
/// jpeg_fixed -O2 compile, 3.2 M of it in `Range::new`. They are built from
/// the width.
#[test]
fn a_mask_and_a_range_of_a_wide_word_need_no_wide_shift() {
    for bits in 1..256 {
        let expected = (0..bits).fold(U256::ZERO, |all, at| all | (U256::ONE << at));
        assert_eq!(mask::<U256>(bits), expected, "mask {bits}");
    }
    for bits in 1..128 {
        assert_eq!(mask::<u128>(bits), (1_u128 << bits) - 1, "mask {bits}");
    }
    assert_eq!(mask::<U256>(256), !U256::ZERO);
    assert_eq!(mask::<u128>(0), 0);
    assert_eq!(mask::<u128>(127), u128::MAX >> 1);
    for width in 1..=256 {
        assert_eq!(sign_bit::<U256>(width), U256::ONE << (width - 1), "sign bit {width}");
    }
    SHIFTS.with(|count| count.set(0));
    let range = crate::constantrange::Range::<U256>::new(32, U256::from_u128(5), U256::from_u128(9));
    let _ = (range, mask::<U256>(64), mask::<U256>(160), sign_bit::<U256>(40));
    let _ = (range.get_signed_min(), range.get_signed_max());
    let shifts = SHIFTS.with(std::cell::Cell::get);
    assert_eq!(shifts, 0, "{shifts} wide shifts to make a range, two masks and a sign bit and read its signed ends");
}
