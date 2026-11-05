//! `Ap` past 128 bits: the `i256` `willNotOverflow` makes of an `i128` op.

use super::Ap;

/// `Ap::new` asserted a width of 128 at most, so the `i256` constant of a
/// widened `i128` multiply panicked IndVars' compile of a quad-precision
/// divide.
#[test]
fn a_256_bit_value_holds_a_product_that_a_u128_cannot() {
    let big = Ap::new(256, u128::MAX);
    let square = big.mul(big);
    assert_eq!((square.bits, square.hi), (1, u128::MAX - 1));
    assert!(square.is_negative(), "bit 255 of 2^256 - 2^129 + 1 is set");
    assert!(big.shl(255).is_negative());
    assert_eq!(Ap::from_signed(256, -1), Ap::all_ones(256));
}

#[test]
fn extending_and_truncating_across_128_bits_keeps_the_value() {
    let x = Ap::from_signed(128, -5);
    assert_eq!(x.sext(256), Ap::from_signed(256, -5));
    assert_eq!(x.zext(256).hi, 0);
    assert_eq!(Ap::from_signed(256, -5).trunc(128), x);
    let (_, over) = Ap::signed_max(256).sadd_ov(Ap::one(256));
    assert!(over);
}
