//! SSA use relations over source-neutral MIR.
//!
//! Direct port of `qbopt.analysis.ssa:use_index`.  Python returns operation
//! objects, whose identity distinguishes otherwise equal operations.  Rust
//! returns snapshot-local [`OpOccurrence`] keys for that same relation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::occurrence::{operations, OpOccurrence};
use crate::model::mir::{
    consumed as operation_consumed, Arg, Cell, Held, MemRef, MirBody, Op, Value,
};

/// A substitution followed an id-keyed cycle.
///
/// Direct port of `qbopt.analysis.ssa:provider`'s
/// `ValueError("cyclic value substitution")`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SubstitutionError {
    CyclicValueSubstitution,
}

impl fmt::Display for SubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CyclicValueSubstitution => formatter.write_str("cyclic value substitution"),
        }
    }
}

impl std::error::Error for SubstitutionError {}

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

/// Follow an id-keyed substitution until its provider is unchanged.
///
/// Direct port of `qbopt.analysis.ssa:provider`.
pub(crate) fn provider(
    value: Value,
    swap: &BTreeMap<u32, Value>,
) -> Result<Value, SubstitutionError> {
    let mut value = value;
    let mut seen = BTreeSet::new();
    while let Some(replacement) = swap.get(&value.id) {
        if *replacement == value {
            break;
        }
        if !seen.insert(value.id) {
            return Err(SubstitutionError::CyclicValueSubstitution);
        }
        value = *replacement;
    }
    Ok(value)
}

/// Replace operation reads and memory-address dependencies through `swap`.
///
/// Definitions and held results are deliberately not rewritten.  Merge
/// targets remain untouched: only their source keys are ordinary uses.
/// Direct port of `qbopt.analysis.ssa:substituted`.
pub(crate) fn substituted(op: &Op, swap: &BTreeMap<u32, Value>) -> Result<Op, SubstitutionError> {
    if swap.is_empty() {
        return Ok(op.clone());
    }

    fn reference(
        reference: &MemRef,
        swap: &BTreeMap<u32, Value>,
    ) -> Result<MemRef, SubstitutionError> {
        let mut reference = reference.clone();
        reference.base = reference
            .base
            .map(|value| provider(value, swap))
            .transpose()?;
        reference.segment = reference
            .segment
            .map(|value| provider(value, swap))
            .transpose()?;
        Ok(reference)
    }

    fn operand(argument: &Arg, swap: &BTreeMap<u32, Value>) -> Result<Arg, SubstitutionError> {
        match argument {
            Arg::Held(Held { value, width }) => Ok(Arg::Held(Held {
                value: provider(*value, swap)?,
                width: *width,
            })),
            Arg::Cell(Cell { r#ref }) => Ok(Arg::Cell(Cell {
                r#ref: reference(r#ref, swap)?,
            })),
            _ => Ok(argument.clone()),
        }
    }

    let mut substituted = op.clone();
    substituted.uses = op
        .uses
        .iter()
        .copied()
        .map(|value| provider(value, swap))
        .collect::<Result<_, _>>()?;
    substituted.exits = op
        .exits
        .iter()
        .copied()
        .map(|value| provider(value, swap))
        .collect::<Result<_, _>>()?;
    substituted.args = op
        .args
        .iter()
        .map(|argument| operand(argument, swap))
        .collect::<Result<_, _>>()?;
    substituted.results = op
        .results
        .iter()
        .map(|result| match result {
            Arg::Cell(_) => operand(result, swap),
            _ => Ok(result.clone()),
        })
        .collect::<Result<_, _>>()?;
    substituted.loads = op
        .loads
        .iter()
        .map(|one| reference(one, swap))
        .collect::<Result<_, _>>()?;
    substituted.stores = op
        .stores
        .iter()
        .map(|one| reference(one, swap))
        .collect::<Result<_, _>>()?;
    substituted.memory_values = op
        .memory_values
        .iter()
        .map(|(one, known)| Ok((reference(one, swap)?, known.clone())))
        .collect::<Result<_, _>>()?;
    substituted.merges = op
        .merges
        .iter()
        .map(|(source, target)| Ok((provider(*source, swap)?, *target)))
        .collect::<Result<_, _>>()?;
    Ok(substituted)
}

/// Every value mentioned by a body, in its source declaration order.
///
/// Direct port of `qbopt.analysis.ssa:values`.
pub(crate) fn values(body: &MirBody) -> impl Iterator<Item = Value> + '_ {
    body.blocks.iter().flat_map(|block| {
        block
            .ops
            .iter()
            .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied())
            .chain(
                block.phis.iter().flat_map(|phi| {
                    std::iter::once(phi.result).chain(phi.incoming.values().copied())
                }),
            )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::model::mir::{
        Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, Phi, Value,
    };

    use super::{operations, provider, substituted, use_index, values, SubstitutionError};

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

    /// Direct Rust port of `tests/test_pointer_memory.py`'s two substitution
    /// regressions: a whole-pointer load and CALL-known memory both retain
    /// the replacement address dependency.
    #[test]
    fn substituted_rewrites_whole_pointer_and_known_memory_cells() {
        let pointer = Value::new(1, 0);
        let replacement = Value::new(9, 4);
        let value = Value::new(2, 0);
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(pointer);
        reference.pointer = true;
        let mut op = Op::new(0, None, "mov", vec![value], vec![pointer]);
        op.kind = Kind::Load;
        op.args = vec![Arg::Cell(Cell {
            r#ref: reference.clone(),
        })];
        op.results = vec![Arg::Held(Held { value, width: 2 })];
        op.loads = vec![reference.clone()];
        op.memory_values = vec![(reference, Const::new(7, 2))];

        let changed = substituted(&op, &BTreeMap::from([(pointer.id, replacement)])).unwrap();
        assert!(changed.loads[0].pointer);
        assert_eq!(changed.loads[0].base, Some(replacement));
        assert_eq!(
            changed.args[0],
            Arg::Cell(Cell {
                r#ref: changed.loads[0].clone()
            })
        );
        assert_eq!(changed.uses, vec![replacement]);
        assert_eq!(changed.loads[0].segment, None);
        assert_eq!(changed.memory_values[0].0.base, Some(replacement));
    }

    #[test]
    fn substituted_rewrites_exactly_python_ssa_uses() {
        let old = Value::new(1, 0);
        let middle = Value::new(2, 0);
        let replacement = Value::new(3, 0);
        let defined = Value::new(4, 0);
        let merge_target = Value::new(5, 0);
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(old);
        reference.segment = Some(middle);
        let mut op = Op::new(0, None, "op", vec![defined], vec![old]);
        op.exits = vec![middle];
        op.args = vec![
            Arg::Held(Held {
                value: old,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        op.results = vec![
            Arg::Held(Held {
                value: old,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        op.loads = vec![reference.clone()];
        op.stores = vec![reference.clone()];
        op.memory_values = vec![(reference, Const::new(7, 2))];
        op.merges.insert(old, merge_target);
        let swaps = BTreeMap::from([
            (old.id, middle),
            (middle.id, replacement),
            (merge_target.id, defined),
        ]);

        let changed = substituted(&op, &swaps).unwrap();
        assert_eq!(changed.defines, vec![defined]);
        assert_eq!(changed.uses, vec![replacement]);
        assert_eq!(changed.exits, vec![replacement]);
        assert_eq!(
            changed.args[0],
            Arg::Held(Held {
                value: replacement,
                width: 2
            })
        );
        assert_eq!(
            changed.results[0],
            Arg::Held(Held {
                value: old,
                width: 2
            })
        );
        let argument_cell = match &changed.args[1] {
            Arg::Cell(cell) => cell,
            _ => panic!("argument must remain a cell"),
        };
        assert_eq!(argument_cell.r#ref.base, Some(replacement));
        assert_eq!(argument_cell.r#ref.segment, Some(replacement));
        for reference in changed
            .loads
            .iter()
            .chain(&changed.stores)
            .chain(std::iter::once(&changed.memory_values[0].0))
        {
            assert_eq!(reference.base, Some(replacement));
            assert_eq!(reference.segment, Some(replacement));
        }
        let result_cell = match &changed.results[1] {
            Arg::Cell(cell) => cell,
            _ => panic!("result must remain a cell"),
        };
        assert_eq!(result_cell.r#ref.base, Some(replacement));
        assert_eq!(result_cell.r#ref.segment, Some(replacement));
        assert_eq!(
            changed.merges.iter().collect::<Vec<_>>(),
            vec![(&replacement, &merge_target)]
        );
        assert_eq!(substituted(&op, &BTreeMap::new()).unwrap(), op);
    }

    #[test]
    fn provider_follows_transitively_and_reports_cycles() {
        let first = Value::new(1, 0);
        let second = Value::new(2, 0);
        let third = Value::new(3, 0);
        assert_eq!(
            provider(
                first,
                &BTreeMap::from([(first.id, second), (second.id, third)])
            )
            .unwrap(),
            third
        );
        assert_eq!(
            provider(
                first,
                &BTreeMap::from([(first.id, second), (second.id, first)])
            ),
            Err(SubstitutionError::CyclicValueSubstitution)
        );
        assert_eq!(
            provider(first, &BTreeMap::from([(first.id, first)])).unwrap(),
            first
        );
    }

    #[test]
    fn values_preserves_body_order_and_duplicates() {
        let defined = Value::new(1, 0);
        let used = Value::new(2, 0);
        let exited = Value::new(3, 0);
        let phi_result = Value::new(4, 1);
        let mut first = Op::new(0, None, "first", vec![defined], vec![used]);
        first.exits = vec![exited, used];
        let second = Op::new(1, None, "second", vec![used], vec![defined]);
        let mut phi = Phi::new(phi_result);
        phi.incoming.insert(9, used);
        phi.incoming.insert(7, defined);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![phi], vec![first, second], vec![]),
                MirBlock::new(
                    1,
                    vec![],
                    vec![Op::new(2, None, "third", vec![exited], vec![used])],
                    vec![],
                ),
            ],
        );

        assert_eq!(
            values(&body).collect::<Vec<_>>(),
            vec![
                defined, used, exited, used, used, defined, phi_result, used, defined, exited,
                used,
            ]
        );
    }
}
