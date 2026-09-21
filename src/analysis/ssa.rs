//! SSA use relations over source-neutral MIR.
//!
//! Direct port of `qbopt.analysis.ssa:use_index`.  Python returns operation
//! objects, whose identity distinguishes otherwise equal operations.  Rust
//! returns snapshot-local [`OpOccurrence`] keys for that same relation.

use std::collections::{BTreeMap, BTreeSet};

use super::occurrence::{operations, OpOccurrence};
use crate::model::mir::{consumed as operation_consumed, MirBody, Value};

/// Each value's operation users, built in one body traversal.
///
/// `Op.uses` includes carried merge inputs; [`operation_consumed`] is the
/// narrower set an operation actually reads.  Callers choose the relation
/// they mean, while construction and body/block/operation ordering stay one
/// shared mechanism.  Each operation occurs at most once in a value's list.
///
/// Direct port of `qbopt.analysis.ssa:use_index`.
pub(crate) fn use_index(
    body: &MirBody,
    values: Option<&BTreeSet<Value>>,
    consumed: bool,
) -> BTreeMap<Value, Vec<OpOccurrence>> {
    let mut users = BTreeMap::<Value, Vec<OpOccurrence>>::new();
    for (occurrence, _, operation) in operations(body) {
        let mut read = if consumed {
            operation_consumed(operation)
        } else {
            operation.uses.iter().copied().collect()
        };
        if let Some(wanted) = values {
            read.retain(|value| wanted.contains(value));
        }
        for value in read {
            users.entry(value).or_default().push(occurrence);
        }
    }
    users
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::model::mir::{Arg, Held, Kind, MirBlock, MirBody, Op, Value};

    use super::{operations, use_index};

    /// A carried high half is a raw use but not an operation the instruction
    /// consumes.  Direct Rust port of
    /// `tests/test_sccp.py:test_ssa_use_index_preserves_modes_filtering_and_operation_order`.
    #[test]
    fn use_index_preserves_modes_filtering_and_operation_order() {
        let source = Value {
            variable: 1,
            ..Value::new(1, 0)
        };
        let carried = Value {
            variable: 2,
            ..Value::new(2, 0)
        };
        let first_result = Value {
            variable: 3,
            ..Value::new(3, 0)
        };
        let second_result = Value {
            variable: 4,
            ..Value::new(4, 1)
        };
        let ignored = Value {
            variable: 5,
            ..Value::new(5, 1)
        };
        let mut first = Op::new(0, None, "", vec![first_result], vec![source, carried]);
        first.kind = Kind::Copy;
        first.args = vec![Arg::Held(Held {
            value: source,
            width: 2,
        })];
        first.results = vec![Arg::Held(Held {
            value: first_result,
            width: 2,
        })];
        first.merges.insert(carried, first_result);

        let mut second = Op::new(1, None, "", vec![second_result], vec![source, ignored]);
        second.kind = Kind::Copy;
        second.args = vec![
            Arg::Held(Held {
                value: source,
                width: 2,
            }),
            Arg::Held(Held {
                value: ignored,
                width: 2,
            }),
        ];
        second.results = vec![Arg::Held(Held {
            value: second_result,
            width: 2,
        })];

        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![first, second], vec![])],
        );
        let occurrences = operations(&body)
            .map(|(occurrence, _, _)| occurrence)
            .collect::<Vec<_>>();
        let wanted = BTreeSet::from([source, carried]);

        let raw = use_index(&body, Some(&wanted), false);
        let consumed = use_index(&body, Some(&wanted), true);

        assert_eq!(
            raw,
            BTreeMap::from([
                (source, vec![occurrences[0], occurrences[1]]),
                (carried, vec![occurrences[0]]),
            ])
        );
        assert_eq!(
            consumed,
            BTreeMap::from([(source, vec![occurrences[0], occurrences[1]])])
        );
    }
}
