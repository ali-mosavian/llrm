//! Exact MIR control-flow cleanup.
//!
//! Direct port of `qbopt/optimize/cfg.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::loops;
use crate::analysis::ssa::{self, SubstitutionError};
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode, Phi};
use crate::optimize::transform::_empty_operation;

/// Direct port of `qbopt/optimize/cfg.py:_empty`.
fn _empty(op: &Op) -> bool {
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
        // Python tests `op.stack` for truth: a zero depth counts as empty.
        && op.stack.unwrap_or(0) == 0
        && op.floating_origin.is_none()
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
pub(crate) fn merged(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    // Python's `dict(body.repetitions)`: later duplicate keys win once for
    // the entire fixed-point walk.
    let repeated = body.repetitions.iter().copied().collect::<BTreeMap<_, _>>();
    let mut body = body.clone();

    loop {
        // `sort` is stable, preserving original order for duplicate source
        // addresses just as Python's `sorted` does.
        let mut ordered = body.blocks.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|block| block.at);
        let predecessors = loops::predecessors(&body.blocks);
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
            let second = ordered[target_position];
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
                    || block.ops.iter().any(|op| !_empty(op))
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

            let mut erased = None;
            if let Some(last) = first.ops.last() {
                if last.barrier() || last.kind == Kind::Opaque {
                    continue;
                }
                if matches!(last.kind, Kind::Branch | Kind::Jump) {
                    if last.kind != Kind::Jump || last.target != Some(target) {
                        continue;
                    }
                    let mut empty = _empty_operation(last);
                    empty.target = None;
                    empty.test = None;
                    erased = Some(empty);
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

            let kept = first.ops.len() - usize::from(erased.is_some());
            let mut moved = first.ops[..kept].to_vec();
            moved.extend(erased);
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
                    cold: first.cold,
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
                cold: block.cold,
            });
        }
        Rc::make_mut(&mut body).blocks = blocks;
    }
}

#[cfg(test)]
mod tests {
    //! Ports of `tests/test_cfg_merge.py`.
    use std::rc::Rc;
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
    fn test_single_entry_phi_is_replaced_and_jump_bytes_are_retained() {
        let result = merged(&Rc::new(MirBody::clone(&chain()))).unwrap();
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
    fn test_cloned_chain_without_source_bytes_merges() {
        let mut body = chain();
        for block in &mut body.blocks {
            for op in &mut block.ops {
                op.absorbed.clear();
            }
        }
        let result = merged(&Rc::new(MirBody::clone(&body))).unwrap();
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
    fn test_transferred_byte_ownership_does_not_block_chain_merge() {
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

        let result = merged(&Rc::new(MirBody::clone(&body))).unwrap();
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
    fn test_merge_preserves_alternate_entries_and_layout() {
        let mut entry = chain();
        entry.entry = 10;
        assert_eq!(*merged(&Rc::new(entry.clone())).unwrap(), entry);

        let mut other_predecessor = chain();
        other_predecessor
            .blocks
            .push(MirBlock::new(20, Vec::new(), Vec::new(), vec![10]));
        assert_eq!(*merged(&Rc::new(other_predecessor.clone())).unwrap(), other_predecessor);

        let mut repetition = chain();
        repetition.repetitions = vec![(10, 2)];
        assert_eq!(*merged(&Rc::new(repetition.clone())).unwrap(), repetition);

        let mut intervening = chain();
        let operation = intervening.blocks[1].ops[0].clone();
        intervening
            .blocks
            .push(MirBlock::new(5, Vec::new(), vec![operation], Vec::new()));
        assert_eq!(*merged(&Rc::new(intervening.clone())).unwrap(), intervening);
    }

    /// A later join must still receive the value from the merged path.
    /// Direct port of
    /// `tests/test_cfg_merge.py:test_successor_phi_edge_is_renamed_to_the_surviving_block`.
    #[test]
    fn test_successor_phi_edge_is_renamed_to_the_surviving_block() {
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

        let result = merged(&Rc::new(MirBody::clone(&body))).unwrap();
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
    fn test_unreachable_ownership_between_blocks_moves_without_losing_spans() {
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

        let result = merged(&Rc::new(MirBody::clone(&body))).unwrap();
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

    /// Python `_empty` tests `op.stack` for truth, so `stack=0` is empty
    /// (checked against Python); the draft's `is_none` refused it.
    #[test]
    fn test_empty_accepts_zero_stack_depth() {
        let mut op = Op::new(5, OpCode::Operation(Operation::Nothing), "", Vec::new(), Vec::new());
        op.kind = Kind::Nothing;
        op.stack = Some(0);
        assert!(super::_empty(&op));
    }

    fn udtrng() -> (Rc<crate::objectfile::module::Module>, Rc<MirBody>) {
        use crate::support::testing;
        let found = testing::module("fixtures/regressions/udtrng-p-g2.obj");
        let body = testing::main_body(&found, &testing::blocks_of(&found));
        (found, body)
    }

    /// UDTRNG's END arms falsely rejoined the guarded array accesses in MIR.
    #[test]
    fn test_end_guards_have_no_return_edge_in_raised_control_flow() {
        let (found, body) = udtrng();
        let ends = |block: &&MirBlock| {
            block.ops.last().is_some_and(|last| found.calls.get(&last.at).map(String::as_str) == Some("B$CEND"))
        };
        let exits: Vec<&MirBlock> = body.blocks.iter().filter(ends).collect();
        assert_eq!(exits.len(), 2);
        assert!(exits.iter().all(|block| block.succ.is_empty()));
        let parents = |at: i64| {
            body.blocks.iter().filter(|block| block.succ.contains(&at)).map(|block| block.at).collect::<Vec<_>>()
        };
        assert_eq!((parents(0x60), parents(0x6c)), (vec![0x30], vec![0x60]));
    }

    /// UDTRNG's bounds guards hid their read inside CMP, leaving range analysis no SSA value to constrain.
    #[test]
    fn test_udtrng_bounds_compare_explicit_values() {
        let (_, body) = udtrng();
        let guards: Vec<Op> = crate::support::testing::ops(&body)
            .into_iter()
            .filter(|op| [0x54, 0x60].contains(&op.at) && op.op == Some(OpCode::Operation(Operation::Compare)))
            .collect();
        assert_eq!(guards.len(), 2);
        assert!(guards.iter().all(|op| op.loads.is_empty() && matches!(op.args[0], Arg::Held(_))));
    }

    /// UDTRNG lost both slot bounds when the next statement reloaded the same cell.
    #[test]
    fn test_udtrng_guards_constrain_subsequent_reads_of_slot() {
        use crate::analysis::ranges::Interval;
        use crate::frontend::arrayfacts::{_edge, _read, _transfer, Fact, State};
        let (_, body) = udtrng();
        let block = |at: i64| body.block(at).unwrap();
        let (state, _) = _transfer(block(0x30), &State::default(), false);
        let state = _edge(&state, block(0x30), 0x60).unwrap();
        let (state, _) = _transfer(block(0x60), &state, false);
        let state = _edge(&state, block(0x60), 0x6c).unwrap();
        let cell = &block(0x6c).ops[0].args[0];
        let expected = Interval { low: 0.into(), high: 2.into(), width: 2 };
        assert_eq!(_read(cell, &state), Some(Fact::Interval(expected)));
    }

    /// An unknown or returning END-shaped call must not erase a reachable path.
    #[test]
    fn test_only_established_terminal_contracts_remove_return_edges() {
        use crate::abi::runtime::{self, Control};
        use crate::support::testing;
        let found = testing::module("fixtures/regressions/udtrng-p-g2.obj");
        let mapped = crate::frontend::blocks::code_map(&found).unwrap();
        let contracts = runtime::for_module(&found, None).unwrap();
        let original: Vec<_> = crate::frontend::blocks::partition(&found, &mapped)
            .into_iter()
            .filter(|block| {
                let last = block.insns.last().map(|insn| insn.at as i64);
                last.and_then(|at| found.calls.get(&at)).map(String::as_str) == Some("B$CEND")
            })
            .collect();
        assert!(!original.is_empty());
        for (known, terminal) in [(false, true), (true, false)] {
            let mut changed = contracts.clone();
            for contract in changed.values_mut() {
                contract.established = known;
                contract.control = if terminal { Control::Never } else { Control::Returns };
            }
            let kept = crate::frontend::raising_control::terminal_edges(original.clone(), &changed);
            assert_eq!(kept, original, "known={known} terminal={terminal}");
        }
    }

    /// BOOLS still split four constant stores and PRINT across four live blocks.
    #[test]
    fn test_bools_constant_program_is_one_live_block() {
        use crate::support::testing;
        for tag in ["q-o", "p-g2", "v-g3"] {
            let (result, states) = testing::emitted_states(&testing::data(format!("fixtures/omf/bools-{tag}.obj")));
            assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{}", result.reason);
            assert_eq!(states.last().unwrap().2.blocks.len(), 1, "{tag}");
        }
    }

    /// LOCALP printed 28/DONE in BC but nothing after optimization lost main's exit.
    #[test]
    fn test_localp_keeps_termination_after_interleaved_procedure() {
        use crate::support::testing;
        for tag in ["q-o", "p-g2", "v-g3"] {
            let result = testing::emitted_lir(format!("fixtures/regressions/localp-{tag}.obj"));
            let found = testing::loaded_bytes(&result.data).unwrap();
            let mapped = crate::frontend::blocks::code_map(&found).unwrap();
            let terminals: Vec<i64> =
                found.calls.iter().filter(|(_, name)| name.as_str() == "B$CENP").map(|(at, _)| *at).collect();
            let [terminal] = <[i64; 1]>::try_from(terminals).unwrap();
            assert!(mapped.starts.contains(&(terminal as usize)), "{tag}");
        }
    }
}
