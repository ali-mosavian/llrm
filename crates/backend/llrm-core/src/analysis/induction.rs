//! Which values are affine functions of a loop's counter.
//!
//! Port of `qbopt/analysis/induction.py`.  `id(op)` is an [`OpOccurrence`]
//! of the analysed body; `floor_div`, `mod_floor`, `modular_inverse` and
//! `gcd` are Python's `//`, `%`, `pow(x, -1, m)` and `math.gcd` on `BigInt`.

use num_bigint::BigInt;

pub fn gcd(mut one: BigInt, mut other: BigInt) -> BigInt {
    while other != BigInt::from(0_u8) {
        let remainder = one % &other;
        one = other;
        other = remainder;
    }
    one
}
