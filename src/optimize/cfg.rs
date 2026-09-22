//! Exact MIR control-flow cleanup.
//!
//! Direct port of `qbopt/optimize/cfg.py:_empty` and `merged`.  This is not
//! a general CFG simplifier: it combines only the forward, single-entry
//! shape Python combines, retaining source ownership as inert MIR operations.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::loops;
use crate::analysis::ssa::{self, SubstitutionError};
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi};

/// Whether `op` is an inert ownership marker.
///
/// Direct port of `qbopt/optimize/cfg.py:_empty`.
fn empty(op: &Op) -> bool {
    op.kind == Kind::Nothing
        && op.name.is_empty()
        && op.defines.is_empty()
        && op.uses.is_empty()
        && op.args.is_empty()
        && op.results.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && !op.barrier()
        && op.floating.is_none()
        && op.stack.is_none()
        && op.floating_origin.is_none()
}

/// Keep opaque source ownership, but no computation or memory effect.
///
/// This is the local exact equivalent of
/// `qbopt/optimize/transform.py:_empty_operation`.  It is intentionally not
/// `mir::cleared`: the two Python functions clear different fields.
fn empty_operation(op: &Op) -> Op {
    let mut result = op.clone();
    result.op = Some(OpCode::nothing());
    result.name.clear();
    result.kind = Kind::Nothing;
    result.defines.clear();
    result.uses.clear();
    result.array = None;
    result.memory_values.clear();
    result.floating = None;
    result.floating_origin = None;
    result.args.clear();
    result.results.clear();
    result.loads.clear();
    result.stores.clear();
    result.merges = OrderedMap::new();
    result.source_backed = false;
    result.raised = None;
    result.target = None;
    result.cases.clear();
    result.symbol = Some(false);
    result.args_known = true;
    result.memory_complete = true;
    result.reads_complete = true;
    result.opaque_defs = Some(BTreeSet::new());
    result.opaque_uses = Some(BTreeSet::new());
    result.stack = None;
    result.test = None;
    result.indirect = false;
    result
}

/// Merge forward single-entry chains, retaining every original byte owner.
///
/// Unreachable ownership-only blocks between the endpoints move with them.
/// Other intervening blocks and unowned gaps retain their placement. Floating
/// sequence and unrolling provenance still require their original block
/// boundaries.
///
/// Direct port of `qbopt/optimize/cfg.py:merged`.  Python lets substitution
/// raise a `ValueError` for an id-keyed cycle; Rust returns that same refusal
/// explicitly instead of retaining a partially rewritten body.
pub(crate) fn merged(body: &MirBody) -> Result<MirBody, SubstitutionError> {
    // Python's `dict(body.repetitions)`: later duplicate keys win once for
    // the entire fixed-point walk.
    let repeated = body.repetitions.iter().copied().collect::<BTreeMap<_, _>>();
    let mut body = body.clone();

    loop {
        // `sort` is stable, preserving original order for duplicate source
        // addresses just as Python's `sorted` does.
        let mut ordered = body.blocks.clone();
        ordered.sort_by_key(|block| block.at);
        let predecessors = loops::predecessors(&ordered);
        // Python's dict comprehension retains the final duplicate address.
        let positions = ordered
            .iter()
            .enumerate()
            .map(|(index, block)| (block.at, index))
            .collect::<BTreeMap<_, _>>();

        let mut replacement = None;
        for (index, first) in ordered.iter().enumerate() {
            if first.succ.len() != 1 || repeated.contains_key(&first.at) {
                continue;
            }
            let target = first.succ[0];
            let Some(&target_position) = positions.get(&target) else {
                continue;
            };
            if target == body.entry || target_position <= index || repeated.contains_key(&target) {
                continue;
            }
            let second = &ordered[target_position];
            if predecessors.get(&target) != Some(&BTreeSet::from([first.at])) {
                continue;
            }
            let between = &ordered[index + 1..target_position];
            if between.iter().any(|block| {
                block.at == body.entry
                    || repeated.contains_key(&block.at)
                    || !block.phis.is_empty()
                    || !block.succ.is_empty()
                    || !predecessors[&block.at].is_empty()
                    || block.ops.iter().any(|op| !empty(op))
            }) {
                continue;
            }
            if first
                .ops
                .iter()
                .chain(&second.ops)
                .any(|op| op.floating_origin.is_some())
            {
                continue;
            }
            if second.phis.iter().any(|phi| {
                phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([first.at])
                    || phi.incoming.values().any(|value| *value == phi.result)
            }) {
                continue;
            }

            let mut ops = first.ops.clone();
            if let Some(last) = ops.last() {
                if last.barrier() || last.kind == Kind::Opaque {
                    continue;
                }
                if matches!(last.kind, Kind::Branch | Kind::Jump) {
                    if last.kind != Kind::Jump || last.target != Some(target) {
                        continue;
                    }
                    let last = ops.pop().expect("last operation exists");
                    let mut erased = empty_operation(&last);
                    erased.target = None;
                    erased.test = None;
                    ops.push(erased);
                }
            }

            let swaps = second
                .phis
                .iter()
                .map(|phi| {
                    (
                        phi.result.id,
                        *phi.incoming
                            .get(&first.at)
                            .expect("single incoming edge is present"),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            if swaps.values().any(|value| swaps.contains_key(&value.id)) {
                continue;
            }

            let mut moved = ops;
            moved.extend(between.iter().flat_map(|block| block.ops.iter().cloned()));
            let mut marker = Op::new(target, OpCode::nothing(), "", Vec::new(), Vec::new());
            marker.kind = Kind::Nothing;
            moved.push(marker);
            moved.extend(second.ops.iter().cloned());
            replacement = Some((
                first.at,
                target,
                MirBlock {
                    at: first.at,
                    phis: first.phis.clone(),
                    ops: moved,
                    succ: second.succ.clone(),
                },
                std::iter::once(target)
                    .chain(between.iter().map(|block| block.at))
                    .collect::<BTreeSet<_>>(),
                swaps,
            ));
            break;
        }

        let Some((first_at, target, combined, removed, swaps)) = replacement else {
            return Ok(body);
        };
        let mut blocks = Vec::with_capacity(body.blocks.len());
        for block in &body.blocks {
            if removed.contains(&block.at) {
                continue;
            }
            let block = if block.at == first_at {
                &combined
            } else {
                block
            };
            let phis = block
                .phis
                .iter()
                .map(|phi| Phi {
                    result: phi.result,
                    incoming: phi
                        .incoming
                        .iter()
                        .map(|(at, value)| {
                            (
                                if *at == target { first_at } else { *at },
                                swaps.get(&value.id).copied().unwrap_or(*value),
                            )
                        })
                        .collect(),
                })
                .collect::<Vec<_>>();
            blocks.push(MirBlock {
                at: block.at,
                phis,
                ops: block
                    .ops
                    .iter()
                    .map(|op| ssa::substituted(op, &swaps))
                    .collect::<Result<_, _>>()?,
                succ: block.succ.clone(),
            });
        }
        body.blocks = blocks;
    }
}

#[cfg(test)]
mod tests {
    use super::merged;
    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, Phi, Value};

    fn chain() -> MirBody {
        let value = Value::new(1, 0);
        let joined = Value::new(2, 10);
        let mut copy = Op::new(
            0,
            OpCode::Operation(Operation::Move),
            "mov",
            vec![value],
            Vec::new(),
        );
        copy.kind = Kind::Copy;
        copy.args = vec![Arg::Const(Const::new(7, 2))];
        copy.results = vec![Arg::Held(Held { value, width: 2 })];
        copy.absorbed = vec![1];

        let mut jump = Op::new(
            3,
            OpCode::Operation(Operation::Jump),
            "jmp",
            Vec::new(),
            Vec::new(),
        );
        jump.kind = Kind::Jump;
        jump.target = Some(10);
        jump.absorbed = vec![2, 3];

        let mut argument = Op::new(
            10,
            OpCode::Operation(Operation::Push),
            "push",
            Vec::new(),
            vec![joined],
        );
        argument.kind = Kind::Arg;
        argument.args = vec![Arg::Held(Held {
            value: joined,
            width: 2,
        })];
        argument.absorbed = vec![4];

        let mut join = Phi::new(joined);
        join.incoming.insert(0, value);
        MirBody::new(
            0,
            vec![
                MirBlock::new(0, Vec::new(), vec![copy, jump], vec![10]),
                MirBlock::new(10, vec![join], vec![argument], Vec::new()),
            ],
        )
    }

    /// A constant passed through a statement join must remain the same PRINT
    /// argument. Direct port of
    /// `tests/test_cfg_merge.py:test_single_entry_phi_is_replaced_and_jump_bytes_are_retained`.
    #[test]
    fn single_entry_phi_is_replaced_and_jump_bytes_are_retained() {
        let result = merged(&chain()).unwrap();
        assert_eq!(result.blocks.len(), 1);
        let block = &result.blocks[0];
        assert!(block.phis.is_empty());
        assert!(block.succ.is_empty());
        assert_eq!(
            block.ops.last().unwrap().args,
            vec![Arg::Held(Held {
                value: Value::new(1, 0),
                width: 2,
            })]
        );
        assert_eq!(block.ops[1].kind, Kind::Nothing);
        assert_eq!(block.ops[1].op, Some(OpCode::nothing()));
        assert_eq!(block.ops[1].absorbed, vec![2, 3]);
        assert_eq!(block.ops[2].op, Some(OpCode::nothing()));
        assert_eq!(merged(&result).unwrap(), result);
    }

    /// IVARM's cloned store and increment remained separate, blocking loop
    /// evaluation. Direct port of
    /// `tests/test_cfg_merge.py:test_cloned_chain_without_source_bytes_merges`.
    #[test]
    fn cloned_chain_without_source_bytes_merges() {
        let mut body = chain();
        for block in &mut body.blocks {
            for op in &mut block.ops {
                op.absorbed.clear();
            }
        }
        let result = merged(&body).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(
            result.blocks[0].ops.last().unwrap().args,
            vec![Arg::Held(Held {
                value: Value::new(1, 0),
                width: 2,
            })]
        );
    }

    /// IVARM's removed load donated its bytes outside the store/increment
    /// chain. Direct port of
    /// `tests/test_cfg_merge.py:test_transferred_byte_ownership_does_not_block_chain_merge`.
    #[test]
    fn transferred_byte_ownership_does_not_block_chain_merge() {
        let mut body = chain();
        let mut owner = Op::new(
            -1,
            OpCode::Operation(Operation::Nothing),
            "",
            Vec::new(),
            Vec::new(),
        );
        owner.kind = Kind::Nothing;
        owner.absorbed = vec![3];
        body.blocks.insert(
            0,
            MirBlock::new(-1, Vec::new(), vec![owner.clone()], Vec::new()),
        );
        body.blocks[1].ops[1].absorbed = vec![2];

        let result = merged(&body).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(
            result.block(0).unwrap().ops.last().unwrap().args,
            vec![Arg::Held(Held {
                value: Value::new(1, 0),
                width: 2,
            })]
        );
        assert_eq!(result.block(-1).unwrap().ops, vec![owner]);
    }

    /// Alternate entries, repeated blocks, another predecessor, and a live
    /// intervening block must keep their original layout. Direct port of
    /// `tests/test_cfg_merge.py:test_merge_preserves_alternate_entries_and_layout`.
    #[test]
    fn merge_preserves_alternate_entries_and_layout() {
        let mut entry = chain();
        entry.entry = 10;
        assert_eq!(merged(&entry).unwrap(), entry);

        let mut other_predecessor = chain();
        other_predecessor
            .blocks
            .push(MirBlock::new(20, Vec::new(), Vec::new(), vec![10]));
        assert_eq!(merged(&other_predecessor).unwrap(), other_predecessor);

        let mut repetition = chain();
        repetition.repetitions = vec![(10, 2)];
        assert_eq!(merged(&repetition).unwrap(), repetition);

        let mut intervening = chain();
        let operation = intervening.blocks[1].ops[0].clone();
        intervening
            .blocks
            .push(MirBlock::new(5, Vec::new(), vec![operation], Vec::new()));
        assert_eq!(merged(&intervening).unwrap(), intervening);
    }

    /// A later join must still receive the value from the merged path.
    /// Direct port of
    /// `tests/test_cfg_merge.py:test_successor_phi_edge_is_renamed_to_the_surviving_block`.
    #[test]
    fn successor_phi_edge_is_renamed_to_the_surviving_block() {
        let mut body = chain();
        let value = body.blocks[0].ops[0].defines[0];
        let joined = Value::new(3, 20);
        body.blocks[1].succ = vec![20];
        let mut incoming = Phi::new(joined);
        incoming.incoming.insert(10, value);
        incoming.incoming.insert(30, Value::new(4, 30));
        body.blocks
            .push(MirBlock::new(20, vec![incoming], Vec::new(), Vec::new()));
        body.blocks
            .push(MirBlock::new(30, Vec::new(), Vec::new(), vec![20]));

        let result = merged(&body).unwrap();
        assert_eq!(result.block(0).unwrap().succ, vec![20]);
        assert_eq!(
            result.block(20).unwrap().phis[0]
                .incoming
                .iter()
                .map(|(at, value)| (*at, *value))
                .collect::<Vec<_>>(),
            vec![(0, value), (30, Value::new(4, 30))]
        );
    }

    /// BOOLS's eliminated arms still own original object bytes after merging.
    /// Direct port of
    /// `tests/test_cfg_merge.py:test_unreachable_ownership_between_blocks_moves_without_losing_spans`.
    #[test]
    fn unreachable_ownership_between_blocks_moves_without_losing_spans() {
        let mut body = chain();
        body.blocks[0].ops[1].absorbed = vec![2];
        let mut empty = Op::new(
            5,
            OpCode::Operation(Operation::Nothing),
            "",
            Vec::new(),
            Vec::new(),
        );
        empty.kind = Kind::Nothing;
        empty.absorbed = vec![3];
        body.blocks
            .push(MirBlock::new(5, Vec::new(), vec![empty], Vec::new()));

        let result = merged(&body).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(
            result.blocks[0]
                .ops
                .iter()
                .map(|op| (op.at, op.absorbed.clone()))
                .collect::<Vec<_>>(),
            vec![
                (0, vec![1]),
                (3, vec![2]),
                (5, vec![3]),
                (10, vec![]),
                (10, vec![4])
            ]
        );
    }
}
