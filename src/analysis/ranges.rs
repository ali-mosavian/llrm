//! Exact value intervals for alias queries.
//!
//! Direct port of `qbopt.analysis.ranges:Interval` and `constants`, limited to
//! the `calls=None` path.  That Python invocation does not consume `dgroup`,
//! memory, or call facts: it converts each pure `consts.known` fact to the
//! singleton interval representing the same unsigned bits.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use super::constants;
use crate::model::mir::{MirBody, Value};

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

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use super::{Interval, constants};
    use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, Phi, Value};

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
