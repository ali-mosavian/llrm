//! Exact value intervals for alias queries.
//!
//! Direct port of `qbopt.analysis.ranges:Interval` and `constants`, limited to
//! the `calls=None` path.  That Python invocation does not consume `dgroup`,
//! memory, or call facts: it converts each pure `consts.known` fact to the
//! singleton interval representing the same unsigned bits.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use super::constants;
use crate::model::mir::{MemRef, MirBody, Value, symbolic_ref};
use crate::object::omf::module::{NO_REGISTER, Space};

/// A non-wrapping mathematical interval at a fixed width.
///
/// Direct port of `qbopt.analysis.ranges:Interval`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// Every value `constants` knows, as the singleton interval an alias query reads.
///
/// Direct port of `qbopt.analysis.ranges:constants(body, dgroup, calls=None)`.
/// The existing value-only `constants::known` is invoked exactly once; like the
/// Python `calls=None` path, this function adds no memory or call facts.
pub(crate) fn constants(body: &MirBody) -> BTreeMap<Value, Interval> {
    constants::known(body)
        .into_iter()
        .map(|(value, fact)| {
            (
                value,
                Interval {
                    low: fact.n.clone(),
                    high: fact.n,
                    width: fact.width,
                },
            )
        })
        .collect()
}

/// A non-wrapping near indexed access as the static byte interval it can touch.
///
/// Direct port of `qbopt.analysis.ranges:covering`.
pub(crate) fn covering(reference: &MemRef, known: &BTreeMap<Value, Interval>) -> MemRef {
    let reference = symbolic_ref(reference);
    let Some(address) = reference.addr else {
        return reference;
    };
    if address.space != Space::Segment || reference.segment.is_some() {
        return reference;
    }
    // Python's `known.get(ref.base)` returns no interval when `base` is None.
    let interval = reference.base.and_then(|base| known.get(&base));
    let Some(interval) = interval else {
        return reference;
    };
    if interval.width != reference.base_width || reference.base_width != 2 {
        return reference;
    }

    let low = BigInt::from(address.disp) + &interval.low;
    let end = BigInt::from(address.disp) + &interval.high + BigInt::from(reference.width);
    let limit = BigInt::from(1_u8) << (8 * reference.base_width);
    if low < BigInt::from(0_u8) || low >= end || end > limit {
        return reference;
    }

    // `Addr` and `MemRef` use host-sized representations, unlike Python's
    // integers. Refuse rather than truncate if a supported Python result does
    // not fit those representations.
    let width = &end - &low;
    let (Ok(disp), Ok(width)) = (i64::try_from(&low), u32::try_from(&width)) else {
        return reference;
    };
    let mut covered = reference.clone();
    covered.addr = Some(crate::object::omf::module::Addr {
        disp,
        base: NO_REGISTER,
        ..address
    });
    covered.base = None;
    covered.width = width;
    covered
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use num_bigint::BigInt;

    use super::{Interval, constants, covering};
    use crate::model::mir::{
        Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, Phi, Symbol, Value,
    };
    use crate::object::omf::module::{Addr, NO_REGISTER, Space};
    use crate::support::PhysicalRegister;

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn copy(at: i64, result: Value, constant: impl Into<BigInt>, width: u32) -> Op {
        let mut op = Op::new(at, None, "", vec![result], vec![]);
        op.kind = Kind::Copy;
        op.args = vec![Arg::Const(Const::new(constant, width))];
        op.results = vec![Arg::Held(Held {
            value: result,
            width,
        })];
        op
    }

    fn indexed(base: Value) -> MemRef {
        let mut address = Addr::new(Space::Segment, 4);
        address.index = 5;
        address.base = PhysicalRegister::new(1);
        address.segment = PhysicalRegister::new(2);
        let mut reference = MemRef::new(Some(address), 2);
        reference.base = Some(base);
        reference.base_width = 2;
        reference
    }

    fn known(
        base: Value,
        low: impl Into<BigInt>,
        high: impl Into<BigInt>,
        width: u32,
    ) -> BTreeMap<Value, Interval> {
        [(
            base,
            Interval {
                low: low.into(),
                high: high.into(),
                width,
            },
        )]
        .into_iter()
        .collect()
    }

    #[test]
    fn direct_ranges_covering_makes_a_static_byte_hull() {
        // `tests/test_ranges.py::test_range_alias_checks_cover_width_and_wrap`:
        // an index in 0..20 at displacement 4, reading two bytes, touches 4..26.
        let base = value(1, 0);
        let covered = covering(&indexed(base), &known(base, 0, 20, 2));

        assert_eq!(covered.addr.unwrap().disp, 4);
        assert_eq!(covered.addr.unwrap().space, Space::Segment);
        assert_eq!(covered.addr.unwrap().index, 5);
        assert_eq!(covered.addr.unwrap().base, NO_REGISTER);
        assert_eq!(covered.addr.unwrap().segment, PhysicalRegister::new(2));
        assert_eq!(covered.base, None);
        assert_eq!(covered.width, 22);
    }

    #[test]
    fn direct_ranges_covering_refuses_negative_and_wrapping_hulls() {
        // The same Python matrix refuses an interval whose low byte is below
        // zero and one whose high byte plus access width wraps the word.
        let base = value(1, 0);
        let reference = indexed(base);

        assert_eq!(covering(&reference, &known(base, -8, 20, 2)), reference);
        assert_eq!(covering(&reference, &known(base, 0, 65_535, 2)), reference);
    }

    #[test]
    fn direct_ranges_covering_refuses_missing_wrong_and_mismatched_intervals() {
        let base = value(1, 0);
        let other = value(2, 0);
        let reference = indexed(base);

        assert_eq!(covering(&reference, &BTreeMap::new()), reference);
        assert_eq!(covering(&reference, &known(other, 0, 20, 2)), reference);
        assert_eq!(covering(&reference, &known(base, 0, 20, 4)), reference);

        let mut wide_base = reference.clone();
        wide_base.base_width = 4;
        assert_eq!(covering(&wide_base, &known(base, 0, 20, 4)), wide_base);
    }

    #[test]
    fn direct_ranges_covering_refuses_explicit_segments_and_wrong_spaces() {
        let base = value(1, 0);
        let reference = indexed(base);
        let interval = known(base, 0, 20, 2);

        let mut segmented = reference.clone();
        segmented.segment = Some(value(2, 0));
        assert_eq!(covering(&segmented, &interval), segmented);

        let mut framed = reference.clone();
        framed.addr.as_mut().unwrap().space = Space::Frame;
        assert_eq!(covering(&framed, &interval), framed);
    }

    #[test]
    fn direct_ranges_covering_normalizes_symbolic_references_first() {
        let base = value(1, 0);
        let segment = value(2, 0);
        let mut reference = indexed(base);
        reference.segment = Some(segment);
        reference.symbolic = Some(Symbol {
            space: Space::Segment,
            index: 7,
            offset: 8,
            width: 2,
            addend: 3,
        });

        let covered = covering(&reference, &known(base, 0, 20, 2));

        // Symbolic normalization clears `base` and the explicit MemRef
        // segment before the interval lookup, so the absent base refuses.
        assert_eq!(covered.addr.unwrap().space, Space::Segment);
        assert_eq!(covered.addr.unwrap().disp, 11);
        assert_eq!(covered.addr.unwrap().index, 7);
        assert_eq!(covered.base, None);
        assert_eq!(covered.segment, None);
        assert_eq!(covered.width, reference.width);
    }

    #[test]
    fn direct_ranges_constants_preserves_masked_unsigned_values_and_widths() {
        let word = value(1, 0);
        let dword = value(2, 0);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                0,
                vec![],
                vec![copy(0, word, -1, 2), copy(0, dword, 0x1_0000_0001_u64, 4)],
                vec![],
            )],
        );

        let facts = constants(&body);

        assert_eq!(
            facts.get(&word),
            Some(&Interval {
                low: BigInt::from(0xffff_u32),
                high: BigInt::from(0xffff_u32),
                width: 2,
            })
        );
        assert_eq!(
            facts.get(&dword),
            Some(&Interval {
                low: BigInt::from(1_u8),
                high: BigInt::from(1_u8),
                width: 4,
            })
        );
    }

    #[test]
    fn direct_ranges_constants_carries_constant_cycle_results() {
        let (seed, joined, carried) = (value(1, 0), value(2, 10), value(3, 10));
        let mut update = Op::new(10, None, "", vec![carried], vec![joined]);
        update.kind = Kind::Add;
        update.args = vec![
            Arg::Held(Held {
                value: joined,
                width: 4,
            }),
            Arg::Const(Const::new(0, 4)),
        ];
        update.results = vec![Arg::Held(Held {
            value: carried,
            width: 4,
        })];
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, seed);
        incoming.insert(10, carried);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![copy(0, seed, 7, 4)], vec![10]),
                MirBlock::new(
                    10,
                    vec![Phi {
                        result: joined,
                        incoming,
                    }],
                    vec![update],
                    vec![10],
                ),
            ],
        );

        let facts = constants(&body);
        let expected = Interval {
            low: BigInt::from(7_u8),
            high: BigInt::from(7_u8),
            width: 4,
        };
        assert_eq!(facts.get(&joined), Some(&expected));
        assert_eq!(facts.get(&carried), Some(&expected));
    }
}
