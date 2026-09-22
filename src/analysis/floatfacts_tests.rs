//! Port of `tests/test_floatfacts.py`.
//!
//! Skipped, needing the corpus raise: `test_fpcse_known_inputs_reach_float_computations`,
//! `test_entry_bytes_are_killed_by_a_store`.

use num_bigint::BigInt;

use super::{Finite, Fraction, decoded, encoded, evaluated};
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::mir::Kind;

fn finite(value: Fraction) -> Finite {
    Finite::new(value, false)
}

#[test]
fn test_unary_facts_preserve_negation_absolute_value_and_zero_sign() {
    let rule = Semantics::new([Format::Extended80], Format::Extended80, Precision::Exact, Rounding::None);
    for (kind, number, negative_zero, expected, expected_negative_zero) in [
        (Kind::Fneg, 2, false, -2, false),
        (Kind::Fabs, 2, false, 2, false),
        (Kind::Fneg, -2, false, 2, false),
        (Kind::Fabs, -2, false, 2, false),
        (Kind::Fneg, 0, false, 0, true),
        (Kind::Fneg, 0, true, 0, false),
        (Kind::Fabs, 0, true, 0, false),
    ] {
        let fact = evaluated(kind, &rule, &[Finite::new(Fraction::from_integer(number), negative_zero)]);
        assert_eq!(fact, Some(Finite::new(Fraction::from_integer(expected), expected_negative_zero)));
    }
}

#[test]
fn test_single_bit_patterns_decode_without_host_float() {
    for (bits, expected, negative_zero) in [
        (0x4000_0000_u32, Fraction::from_integer(2), false),
        (0x4080_0000, Fraction::from_integer(4), false),
        (0x3f40_0000, Fraction::new(3, 4), false),
        (0xc040_0000, Fraction::from_integer(-3), false),
        (0, Fraction::from_integer(0), false),
        (0x8000_0000, Fraction::from_integer(0), true),
    ] {
        let fact = decoded(&BigInt::from(bits), Format::Binary32).unwrap();
        assert!(fact.value == expected && fact.negative_zero == negative_zero);
        assert_eq!(encoded(&fact, Format::Binary32), Some(BigInt::from(bits)));
    }
}

#[test]
fn test_subnormals_infinities_and_nans_are_not_exception_free_inputs() {
    for bits in [1_u32, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0x7f80_0001] {
        assert_eq!(decoded(&BigInt::from(bits), Format::Binary32), None);
    }
}

#[test]
fn test_exact_arithmetic_respects_dynamic_precision_and_zero_sign() {
    let rule = Semantics::new(
        [Format::Extended80, Format::Extended80],
        Format::Extended80,
        Precision::Dynamic,
        Rounding::Dynamic,
    );
    for (kind, left, right, expected) in [
        (Kind::Fadd, 2, 4, Some(Fraction::from_integer(6))),
        (Kind::Fmul, 6, 8, Some(Fraction::from_integer(48))),
        (Kind::Fdiv, 6, 8, Some(Fraction::new(3, 4))),
        (Kind::Fdiv, 1, 3, None),
        (Kind::Fdiv, 1, 0, None),
        (Kind::Fadd, 1 << 24, 1, None),
        (Kind::Fsub, 1, 1, None),
    ] {
        let result = evaluated(
            kind,
            &rule,
            &[finite(Fraction::from_integer(left)), finite(Fraction::from_integer(right))],
        );
        assert_eq!(result.map(|result| result.value), expected, "{kind:?} {left} {right}");
    }
}

#[test]
fn test_single_store_does_not_keep_an_extended_intermediate() {
    let rule = Semantics::new([Format::Extended80], Format::Binary32, Precision::Destination, Rounding::Dynamic);
    assert_eq!(evaluated(Kind::Fstore, &rule, &[finite(Fraction::from_integer((1 << 24) + 1))]), None);
}

#[test]
fn test_sqrt_facts_require_an_exact_rational_square() {
    let rule = Semantics::new([Format::Extended80], Format::Extended80, Precision::Dynamic, Rounding::Dynamic);
    for (number, negative_zero, expected) in [
        (Fraction::from_integer(1_048_576), false, Some(finite(Fraction::from_integer(1024)))),
        (Fraction::new(9, 16), false, Some(finite(Fraction::new(3, 4)))),
        (Fraction::from_integer(0), true, Some(Finite::new(Fraction::from_integer(0), true))),
        (Fraction::from_integer(2), false, None),
        (Fraction::from_integer(-1), false, None),
    ] {
        assert_eq!(evaluated(Kind::Fsqrt, &rule, &[Finite::new(number, negative_zero)]), expected);
    }
}
