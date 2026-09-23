//! Port of `tests/test_floatfacts.py`.

use num_bigint::BigInt;

use super::{Finite, Fraction, decoded, encoded, evaluated, known};
use crate::analysis::consts;
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody};
use crate::objectfile::module::{Addr, Space};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;
use crate::support::testing;

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

/// `_MemoryQueries` keyed a reference by its address, and `repeated` builds a
/// fresh store every iteration, so a freed store's address answered for the
/// next one: fpcse's D exited at 145.5 (three iterations) for 487.5.
#[test]
fn test_a_loop_exit_repeats_its_stores_every_iteration() {
    use crate::objectfile::{module, omf};
    let data = std::fs::read("fixtures/omf/fpcse-p-g2.obj").unwrap();
    let found = module::of(&omf::parse(&data).unwrap()).unwrap();
    let blocks = crate::frontends::bc::blocks::partition(&found, &crate::frontends::bc::blocks::code_map(&found).unwrap());
    let raised = crate::model::mir::bodies(&found, &blocks, None, false, false).unwrap();
    let (_, body) = &raised.values[0];
    let exits = super::loop_exits(body, &found.dgroup.members, &found.calls);
    assert_eq!(exits.len(), 1);
    assert_eq!(exits[0].count, BigInt::from(10));
    let stored: Vec<BigInt> = exits[0].stores.iter().map(|(_, fact)| fact.n.clone()).collect();
    assert_eq!(stored, [0x4240_0000, 0x3f40_0000, 0x43f3_c000].map(BigInt::from));
}

/// FPCSE's 2+4, product 48 and quotient 0.75 should not remain opaque facts.
#[test]
fn test_fpcse_known_inputs_reach_float_computations() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let path = format!("fixtures/omf/fpcse-{tag}.obj").to_lowercase();
        let found = testing::module(&path);
        let body = testing::main_body(&found, &testing::blocks_of(&found));
        // Explicit initial contents of this fixture's constant pool, not a
        // default assumption about arbitrary procedure-entry memory.
        let segments = omf::segments(&found.records);
        let mut initial: IndexMap<Addr, BigInt> = IndexMap::default();
        for (_, segment, offset, data) in omf::ledata(&found.records) {
            if segments[segment as usize].as_ref().is_some_and(|one| one.0 == "BC_CN") {
                for (index, byte) in data.iter().enumerate() {
                    initial.insert(Addr { index: segment, ..Addr::new(Space::Segment, offset + index as i64) }, BigInt::from(*byte));
                }
            }
        }
        let facts = known(&body, &found.dgroup.members, &found.calls, Some(&initial));
        let mut computed: IndexMap<Kind, Fraction> = IndexMap::default();
        for op in testing::ops(&body) {
            for result in &op.results {
                if let Arg::Held(one) = result {
                    if let Some(fact) = facts.get(&one.value) {
                        computed.insert(op.kind, fact.value.clone());
                    }
                }
            }
        }
        assert_eq!(computed[&Kind::Fadd], Fraction::from_integer(6), "{tag}");
        assert_eq!(computed[&Kind::Fmul], Fraction::from_integer(48), "{tag}");
        assert_eq!(computed[&Kind::Fdiv], Fraction::new(3, 4), "{tag}");
    }
}

/// A constant-pool seed is an entry fact, not immutable memory after a write.
#[test]
fn test_entry_bytes_are_killed_by_a_store() {
    let found = testing::module("fixtures/omf/fpcse-p-g2.obj");
    let body = testing::main_body(&found, &testing::blocks_of(&found));
    let store = testing::ops(&body).into_iter().find(|op| op.kind == Kind::Store).unwrap();
    let reference = store.stores[0].clone();
    let mut alone = MirBody::clone(&body);
    alone.blocks = vec![MirBlock::new(body.entry, vec![], vec![store.clone()], vec![])];
    let seed: consts::Cells = [((reference.addr.unwrap(), 1), consts::Known::new(255, 1))].into_iter().collect();
    let before = consts::cells(&alone, &found.dgroup.members, &found.calls, None, Some(&seed), None, None, None);
    assert_eq!(*before[&(body.entry, 0)], seed);
    let after = consts::_kills(seed, &store, &IndexMap::default(), &found.dgroup.members, &found.calls, None, None, false, None);
    assert_eq!(after[&(reference.addr.unwrap(), 1)].n, BigInt::from(0));
}
