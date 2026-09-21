//! Enter a loop at its body after its entry has been proven separately.
//!
//! Direct port of `qbopt/optimize/rotate.py:at_body` and `_swapped`.
//! `analysis::induction` establishes whether a rotation is legal; this file
//! only reconstructs the proven MIR shape.

use std::collections::BTreeMap;

use crate::analysis::ssa::{self, SubstitutionError};
use crate::model::mir::{MirBlock, MirBody, Op, OrderedMap, Phi, Value};
use crate::model::mir_loops::Loop;

/// Enter `loop` at `first`, moving its header phis there by hand.
///
/// Direct port of `qbopt/optimize/rotate.py:at_body`.  Python raises from
/// `ssa.substituted` on a cyclic map; Rust makes that mechanical difference
/// explicit through [`SubstitutionError`] rather than silently retaining an
/// old operation.
pub(crate) fn at_body(
    body: &MirBody,
    loop_: &Loop,
    preheader: i64,
    header: &MirBlock,
    first: &MirBlock,
    ops: &[Op],
    entry_succ: Option<&[i64]>,
) -> Result<MirBody, SubstitutionError> {
    let latch = *loop_
        .latches
        .iter()
        .next()
        .expect("at_body requires one latch");
    let serial = ssa::values(body)
        .chain(
            ops.iter()
                .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied()),
        )
        .map(|value| value.id)
        .max()
        .expect("at_body requires a value")
        + 1;
    let moved = header
        .phis
        .iter()
        .enumerate()
        .map(|(index, phi)| {
            (
                phi.result.id,
                Value {
                    id: serial + index as u32,
                    at: first.at,
                    ..phi.result
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let latest = |value: Value| moved.get(&value.id).copied().unwrap_or(value);
    let ending = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                latest(
                    *phi.incoming
                        .get(&latch)
                        .expect("header phi has a latch input"),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let initial = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                *phi.incoming
                    .get(&preheader)
                    .expect("header phi has a preheader input"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| *at != header.at)
        .collect::<std::collections::BTreeSet<_>>();
    let entry = header
        .phis
        .iter()
        .map(|phi| Phi {
            result: moved[&phi.result.id],
            incoming: OrderedMap::from_iter([
                (
                    preheader,
                    *phi.incoming
                        .get(&preheader)
                        .expect("header phi has a preheader input"),
                ),
                (header.at, ending[&phi.result.id]),
            ]),
        })
        .collect::<Vec<_>>();

    let mut counts = body
        .loop_trip_counts
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    if let Some(count) = counts.remove(&header.at) {
        if counts
            .get(&first.at)
            .is_none_or(|previous| *previous == count)
        {
            counts.insert(first.at, count);
        }
    }

    let mut integer_ranges = body.integer_ranges.clone();
    for phi in &header.phis {
        if let Some(interval) = integer_ranges.remove(&phi.result) {
            integer_ranges.insert(moved[&phi.result.id], interval);
        }
    }

    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let swap = if inside.contains(&block.at) {
                &moved
            } else {
                &ending
            };
            let phis = block
                .phis
                .iter()
                .map(|phi| {
                    let mut incoming = phi
                        .incoming
                        .iter()
                        .map(|(at, value)| {
                            (
                                *at,
                                if inside.contains(at) {
                                    moved.get(&value.id).copied().unwrap_or(*value)
                                } else {
                                    ending.get(&value.id).copied().unwrap_or(*value)
                                },
                            )
                        })
                        .collect::<OrderedMap<_, _>>();
                    if entry_succ.is_some_and(|successors| successors.contains(&block.at))
                        && block.at != first.at
                    {
                        if let Some(zero) = phi.incoming.get(&header.at) {
                            incoming
                                .insert(preheader, initial.get(&zero.id).copied().unwrap_or(*zero));
                        }
                    }
                    Phi {
                        result: phi.result,
                        incoming,
                    }
                })
                .collect::<Vec<_>>();
            let rewritten = MirBlock {
                at: block.at,
                phis,
                ops: block
                    .ops
                    .iter()
                    .map(|op| swapped(op, swap))
                    .collect::<Result<_, _>>()?,
                succ: block.succ.clone(),
            };
            if block.at == preheader {
                Ok(MirBlock {
                    ops: ops.to_vec(),
                    succ: entry_succ
                        .filter(|successors| !successors.is_empty())
                        .map_or_else(|| vec![first.at], |successors| successors.to_vec()),
                    ..rewritten
                })
            } else if block.at == header.at {
                Ok(MirBlock {
                    phis: Vec::new(),
                    ..rewritten
                })
            } else if block.at == first.at {
                Ok(MirBlock {
                    phis: entry.clone(),
                    ..rewritten
                })
            } else {
                Ok(rewritten)
            }
        })
        .collect::<Result<Vec<_>, SubstitutionError>>()?;

    Ok(MirBody {
        blocks,
        integer_ranges,
        loop_trip_counts: counts.into_iter().collect(),
        ..body.clone()
    })
}

/// Apply one non-chained id substitution to an operation.
///
/// Direct port of `qbopt/optimize/rotate.py:_swapped`.
fn swapped(op: &Op, swap: &BTreeMap<u32, Value>) -> Result<Op, SubstitutionError> {
    if swap.is_empty() || !op.uses.iter().any(|value| swap.contains_key(&value.id)) {
        return Ok(op.clone());
    }
    let one_step = swap
        .iter()
        .filter(|(source, target)| !swap.contains_key(&target.id) || target.id == **source)
        .map(|(source, target)| (*source, *target))
        .collect::<BTreeMap<_, _>>();
    ssa::substituted(op, &one_step)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use num_bigint::BigInt;

    use super::{at_body, swapped};
    use crate::model::mir::{IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, Phi, Value};
    use crate::model::mir_loops::Loop;

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn operation(at: i64, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut op = Op::new(at, None, "", defines, uses);
        op.kind = kind;
        op
    }

    fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
        Phi {
            result,
            incoming: incoming.iter().copied().collect(),
        }
    }

    /// Header phi rotation must distinguish split values of one variable:
    /// `tests/test_rotate.py` regressed when a post-loop call answer was
    /// renamed to the loop counter and the exit disappeared.
    #[test]
    fn at_body_moves_phis_and_substitutes_one_step() {
        let initial = Value {
            variable: 9,
            version: 1,
            ..value(1, 0)
        };
        let counter = Value {
            variable: 9,
            version: 2,
            ..value(2, 1)
        };
        let next = Value {
            variable: 9,
            version: 3,
            ..value(3, 2)
        };
        let same_variable_but_not_phi = Value {
            variable: 9,
            version: 4,
            ..value(4, 3)
        };
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![operation(0, Kind::Jump, vec![], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![operation(1, Kind::Branch, vec![], vec![counter])],
                    vec![2, 4],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(
                    3,
                    vec![],
                    vec![operation(3, Kind::Jump, vec![], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    4,
                    vec![],
                    vec![operation(
                        4,
                        Kind::Return,
                        vec![],
                        vec![next, same_variable_but_not_phi],
                    )],
                    vec![],
                ),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &body.blocks[0].ops,
            None,
        )
        .unwrap();

        let moved = Value {
            id: 5,
            at: 2,
            ..counter
        };
        assert!(changed.blocks[1].phis.is_empty());
        assert_eq!(
            changed.blocks[2].phis,
            vec![phi(moved, &[(0, initial), (1, next)])]
        );
        assert_eq!(changed.blocks[2].ops[0].uses, vec![moved]);
        assert_eq!(
            changed.blocks[4].ops[0].uses,
            vec![next, same_variable_but_not_phi]
        );
        assert_eq!(changed.blocks[0].succ, vec![2]);
    }

    #[test]
    fn swapped_filters_chains_and_only_starts_from_operation_uses() {
        let first = value(1, 0);
        let middle = value(2, 1);
        let last = value(3, 2);
        let mut memory = MemRef::new(None, 2);
        memory.base = Some(first);
        let mut only_memory = operation(0, Kind::Load, vec![], vec![]);
        only_memory.loads = vec![memory.clone()];
        assert_eq!(
            swapped(&only_memory, &BTreeMap::from([(first.id, middle)])).unwrap(),
            only_memory
        );
        let mut uses = operation(0, Kind::Load, vec![], vec![first]);
        uses.loads = vec![memory];
        let changed = swapped(
            &uses,
            &BTreeMap::from([(first.id, middle), (middle.id, last)]),
        )
        .unwrap();
        assert_eq!(changed.uses, vec![first]);
        assert_eq!(changed.loads[0].base, Some(first));
    }

    #[test]
    fn at_body_allocates_after_supplied_operations_and_repairs_zero_trip_exit_facts() {
        let initial = value(1, 0);
        let counter = value(2, 1);
        let next = value(3, 2);
        let guard = value(40, 0);
        let supplied_use = value(41, 0);
        let supplied_exit = value(42, 0);
        let exit_value = value(5, 4);
        let exit_phi = phi(exit_value, &[(1, counter), (8, value(6, 8))]);
        let mut body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1]),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![],
                    vec![2, 4],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(3, vec![], vec![], vec![1]),
                MirBlock::new(4, vec![exit_phi], vec![], vec![]),
            ],
        );
        let range = IntegerRange::new(BigInt::from(0), BigInt::from(9), 2);
        body.integer_ranges.insert(counter, range.clone());
        body.loop_trip_counts = vec![(1, 7), (1, 8), (2, 99), (9, 1)];
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let mut guard_op = operation(0, Kind::Branch, vec![guard], vec![supplied_use]);
        guard_op.exits = vec![supplied_exit];
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &[guard_op.clone()],
            Some(&[3, 4]),
        )
        .unwrap();

        let moved = Value {
            id: 43,
            at: 2,
            ..counter
        };
        assert_eq!(changed.blocks[2].phis[0].result, moved);
        assert_eq!(changed.blocks[0].ops, vec![guard_op.clone()]);
        assert_eq!(changed.blocks[0].succ, vec![3, 4]);
        assert_eq!(changed.blocks[4].phis[0].incoming.get(&0), Some(&initial));
        assert_eq!(changed.integer_ranges.get(&counter), None);
        assert_eq!(changed.integer_ranges.get(&moved), Some(&range));
        assert_eq!(changed.loop_trip_counts, vec![(2, 99), (9, 1)]);

        let mut no_collision = body.clone();
        no_collision.loop_trip_counts = vec![(1, 7), (1, 8), (9, 1)];
        let transferred = at_body(
            &no_collision,
            &loop_,
            0,
            &no_collision.blocks[1],
            &no_collision.blocks[2],
            &[guard_op],
            Some(&[3, 4]),
        )
        .unwrap();
        assert_eq!(transferred.loop_trip_counts, vec![(2, 8), (9, 1)]);
    }

    #[test]
    fn at_body_rebuilds_every_duplicate_address_block_in_source_order() {
        let initial = value(1, 0);
        let counter = value(2, 1);
        let next = value(3, 2);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1]),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![],
                    vec![2],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(3, vec![], vec![], vec![1]),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(20, Kind::Return, vec![], vec![counter])],
                    vec![],
                ),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(changed.blocks.len(), 5);
        assert_eq!(changed.blocks[2].phis.len(), 1);
        assert_eq!(changed.blocks[4].phis.len(), 1);
        assert_eq!(
            changed.blocks[2].ops[0].uses[0].id,
            changed.blocks[2].phis[0].result.id
        );
        assert_eq!(
            changed.blocks[4].ops[0].uses[0].id,
            changed.blocks[4].phis[0].result.id
        );
    }
}
