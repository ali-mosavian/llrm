//! Port of `tests/test_floatbounds.py`.
//!
//! Skipped, needing `transform`, `wholeseg`, raising or the corpus:
//! `test_fpdeep_reuses_proven_finite_array_loads`,
//! `test_array_reuse_requires_every_element_to_have_proven_integer_bounds`,
//! `test_finite_array_proof_requires_known_aligned_nonwrapping_bytes`,
//! `test_integer_helper_with_a_live_clobbered_result_keeps_it_defined`,
//! `test_computed_runtime_integer_uses_one_conversion`,
//! `test_helper_conversion_respects_its_effect_contract`,
//! `test_runtime_integer_conversion_is_shared_in_emitted_code`,
//! `test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not`.

use num_bigint::BigInt;

use super::evaluated;
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::mir::Kind;

#[test]
fn test_dynamic_arithmetic_requires_exactness_at_every_precision() {
    let rule = Semantics::new(
        [Format::Extended80, Format::Extended80],
        Format::Extended80,
        Precision::Dynamic,
        Rounding::Dynamic,
    );
    let pair = |low: i64, high: i64| (BigInt::from(low), BigInt::from(high));
    for (kind, bounds, expected) in [
        (Kind::Fadd, [(-32768, 32767), (-32768, 32767)], Some((-65536, 65534))),
        (Kind::Fmul, [(-32768, 32767), (-32768, 32767)], None),
        (Kind::Fmul, [(-100, 100), (-100, 100)], Some((-10000, 10000))),
        (Kind::Fsub, [(-10, 10), (-10, 10)], Some((-20, 20))),
        (Kind::Fdiv, [(1, 3), (1, 3)], None),
        (Kind::Fadd, [(1 << 24, 1 << 24), (1, 1)], None),
    ] {
        let inputs = bounds.map(|(low, high)| pair(low, high));
        assert_eq!(
            evaluated(kind, &rule, &inputs),
            expected.map(|(low, high)| pair(low, high)),
            "{kind:?} {bounds:?}"
        );
    }
}
