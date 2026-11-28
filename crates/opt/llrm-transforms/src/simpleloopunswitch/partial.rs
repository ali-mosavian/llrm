//! Partially invariant conditions: `hasPartialIVCondition`
//! ([`LoopUtils.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L2352))
//! and `buildPartialInvariantUnswitchConditionalBranch`
//! ([`SimpleLoopUnswitch.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L354)).
//!
//! # How it works
//! The header's branch tests a compare (or truncation) of loads in the loop. If
//! no store or call on any path from one successor back to the header may write
//! what those loads read, the test is the same on every trip that takes that
//! path: the loads and what feeds them are copied before the loop, and the loop
//! is unswitched on the copy.
//!
//! DEVIATE SU12 (owner w9, exit: MemorySSA is passed to the pass): the pin
//! walks the uses of each load's defining `MemoryAccess`, up to `MSSAThreshold`
//! accesses, for a `MemoryDef` on the path that may modify the location. Here
//! every store, call and atomic in the path's blocks is asked of the alias
//! query, with no cap. The answer is the same where MemorySSA is precise, and
//! a def before the load in the header is asked too, which the pin does not.
//! `PathIsNoop` and `ExitForPath` are not made: `SimpleLoopUnswitch` never
//! reads them.

use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::alias::AliasQuery;
use llrm_mir::dense::IdMap;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{CastOp, Opcode};

use super::Run;

/// `IVConditionInfo`: the instructions to copy, condition first, and the
/// value the condition has on the path that leaves it unchanged.
#[derive(Clone, Debug)]
pub(super) struct PartialIv {
    pub to_duplicate: Vec<InstId>,
    pub known_value: bool,
}

/// The value `inst` defines, as an operand.
pub(super) fn result_of(
    run: &Run,
    inst: InstId,
) -> Operand {
    Operand::Value(run.function.instruction(inst).result.expect("a copied instruction has a value"))
}

/// `hasPartialIVCondition` of the loop `lp`.
pub(super) fn has_partial_iv_condition(
    run: &Run,
    lp: Loop,
) -> Option<PartialIv> {
    let header = lp.get_header();
    let term = run.function.terminator(header)?;
    let branch = run.function.instruction(term);
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    // The case with the condition outside the loop is handled earlier. A
    // compare or a truncation may use a load.
    let Operand::Value(cond) = branch.operands[0] else { return None };
    let cond_inst = super::def_of(run, Operand::Value(cond))?;
    let cond_end = run.function.instruction(cond_inst);
    if !matches!(
        cond_end.opcode,
        Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Cast(CastOp::Trunc)
    )
        || !lp.contains(run.function.parent(cond_inst)?)
    {
        return None;
    }
    let mut to_duplicate = vec![cond_inst];
    let mut work: Vec<Operand> = cond_end.operands.clone();
    let mut loads: Vec<InstId> = Vec::new();
    while let Some(operand) = work.pop() {
        let Some(inst) = super::def_of(run, operand) else { continue };
        if !run.function.parent(inst).is_some_and(|block| lp.contains(block)) {
            continue;
        }
        // TODO in the pin: support additional instructions.
        let end = run.function.instruction(inst);
        match end.opcode {
            Opcode::Load { volatile: false, .. } => loads.push(inst),
            Opcode::Load { volatile: true, .. } => return None,
            Opcode::GetElementPtr { .. } => {}
            _ => return None,
        }
        to_duplicate.push(inst);
        work.extend(end.operands.iter().copied());
    }
    // If we branch to the same successor, partial unswitching will not be
    // beneficial.
    let Operand::Block(on_true) = branch.operands[1] else { return None };
    let Operand::Block(on_false) = branch.operands[2] else { return None };
    if on_true == on_false {
        return None;
    }
    let view = Unit::within(run.context, run.layout, run.function, run.outer);
    let accesses = Accesses::lazy();
    let aa = AliasQuery::new(&view, &accesses);
    let mut locations = Vec::new();
    for &load in &loads {
        locations.push(aa.location_of(load)?);
    }
    let no_clobbers_on_path = |succ: BlockId| -> bool {
        // The blocks of the loop on a path from `succ` to the header.
        let mut seen: IdMap<BlockId, ()> = IdMap::new();
        seen.insert(header, ());
        let mut work = vec![succ];
        while let Some(current) = work.pop() {
            if !lp.contains(current) || seen.insert(current, ()).is_some() {
                continue;
            }
            work.extend(run.function.successors(current));
        }
        // Require at least 2 blocks on a path through the loop. This skips
        // paths that directly exit the loop.
        if seen.len() < 2 {
            return false;
        }
        for (block, ()) in seen.iter() {
            for inst in run.function.iter_block(block) {
                match run.function.instruction(inst).opcode {
                    Opcode::Atomic(_) => return false,
                    Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) => {
                        if locations.iter().any(|loc| aa.get_mod_ref_info(inst, loc).writes) {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    };
    if no_clobbers_on_path(on_true) {
        return Some(PartialIv { to_duplicate, known_value: true });
    }
    if no_clobbers_on_path(on_false) {
        return Some(PartialIv { to_duplicate, known_value: false });
    }
    None
}

/// `buildPartialInvariantUnswitchConditionalBranch`: the instructions of
/// `to_duplicate` copied to the end of `block`, last first, each reading the
/// copies of what it read, and a branch on the copy of the first.
pub(super) fn build_partial_invariant_unswitch_conditional_branch(
    run: &mut Run,
    block: BlockId,
    to_duplicate: &[InstId],
    direction: bool,
    unswitched: BlockId,
    normal: BlockId,
) {
    use llrm_mir::edit::Position;
    use llrm_mir::opcode::Flags;
    let mut copies: IdMap<llrm_mir::module::ValueId, Operand> = IdMap::new();
    for &inst in to_duplicate.iter().rev() {
        let old = run.function.instruction(inst).result.expect("a copied instruction has a value");
        let copy = run.function.clone_instruction(inst);
        run.function.insert(copy, Position::End(block)).expect("a placed block");
        let operands = run.function.instruction(copy).operands.clone();
        for (at, operand) in operands.into_iter().enumerate() {
            if let Operand::Value(value) = operand
                && let Some(&mapped) = copies.get(&value)
            {
                run.function.set_operand(copy, at, mapped);
            }
        }
        copies.insert(old, Operand::Value(run.function.instruction(copy).result.expect("a copy's value")));
    }
    let cond = copies[&match result_of(run, to_duplicate[0]) {
        Operand::Value(value) => value,
        _ => unreachable!("a value"),
    }];
    let (first, second) = if direction { (unswitched, normal) } else { (normal, unswitched) };
    let void = run.context.types.void();
    let branch = run
        .function
        .create_instruction(
            Opcode::Br,
            void,
            vec![cond, Operand::Block(first), Operand::Block(second)],
            Flags::default(),
            None,
        );
    run.function.insert(branch, Position::End(block)).expect("a placed block");
}
