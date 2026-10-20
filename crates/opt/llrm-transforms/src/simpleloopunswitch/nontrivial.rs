//! `unswitchNontrivialInvariants`: a branch, `select` or switch in a loop on
//! a loop-invariant condition is decided before the loop, at the price of one
//! copy of the loop for each way it goes.
//!
//! # How it works
//! The preheader is split, so that a new block holds the test. The loop is
//! cloned once for each successor of the terminator that is unswitched
//! (`buildClonedLoopBlocks`), with its preheader and the exit blocks (split
//! first, so that a merge block takes the exit phis of both copies). A block
//! the terminator's successor dominates, which only that way can reach, is
//! cloned for that way only. The terminator moves to the new block and goes to
//! the clone or the original; in each copy it becomes a jump to its
//! successor. A condition made of several invariants (`and`, `or`) is
//! unswitched in the one direction that collapses it, and stays in the
//! retained copy.
//!
//! ```text
//!   pre -> H -> B(br c, X, Y) -> H     pre(br c, pre.us, pre') ; pre.us -> H.us -> B.us(br X.us) -> H.us
//!            \-> exit                                           pre' -> H -> B(br Y) -> H ; exits merged
//! ```
//!
//! The clones and the original lose the blocks no edge reaches, and the uses
//! of the invariant in each copy become the constant it has there.
//!
//! # Deviations
//! - SU7 No `DominatorTree`, `LoopInfo` or `AssumptionCache` update (D8 of the
//!   plan page): the dominator tree is made once the edges are wired, to find
//!   the dead blocks and the uses of the invariant in each copy, and the loops
//!   again after. Cloned loops are found by that, not built by
//!   `buildClonedLoops`, and new loops go on the work list in the order the
//!   loop forest gives them.
//! - SU8 LCSSA and loop-simplify form are made again for the whole function
//!   (`formLCSSA`, `formDedicatedExitBlocks` do it for the loops reached).
//! - SU9 See `simpleloopunswitch.rs`.
//! - SU10 New blocks go at the end of the layout (SU3), not before the
//!   preheader.
//! - SU11 `CanProveNotTakenFirstIteration` compares two constants only
//!   (`simplifyCmpInst`).

use llrm_mir::context::ConstantKind;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::{
    is_guaranteed_not_to_be_undef_or_poison, is_guaranteed_to_transfer_execution_to_successor,
};

use super::candidates::{Candidate, successors};
use super::{Analyzed, Run, def_of, logical, skip_trivial_select};
use crate::edges::{self, phis};
use crate::lcssa::{arms, from_arms};

/// `Loop::isSafeToCloneConditionally` and `isSafeForNoNTrivialUnswitching`:
/// no irreducible cycle and no call that cannot be duplicated in the loop.
pub(super) fn is_safe(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> bool {
    let rpo = crate::loopdeletion::loop_blocks_rpo(run.function, lp);
    !crate::loopdeletion::contains_irreducible_cfg(run.function, &analyzed.loops, &rpo)
        && !lp.get_blocks().iter().any(|&block| {
            run.function
                .iter_block(block)
                .any(
                    |inst| matches!(
                        &run.function.instruction(inst).opcode,
                        Opcode::Call(call) | Opcode::Invoke(call) if call.attrs.iter().any(|attr| matches!(attr, llrm_mir::opcode::Attribute::Flag(flag) if flag == "convergent" || flag == "noduplicate"))
                    ),
                )
        })
}

/// `turnSelectIntoBranch`: `select c, a, b` becomes a branch on `c` over a
/// block that joins by a phi. The branch.
pub(super) fn turn_select_into_branch(
    run: &mut Run,
    select: InstId,
) -> InstId {
    let head = run.function.parent(select).expect("a placed select");
    let operands = run.function.instruction(select).operands.clone();
    let ty = run.function.instruction(select).ty;
    let void = run.context.types.void();
    let tail = edges::split_block_at(run.function, head, select);
    let then = run.function.create_block(None);
    run.function.insert_block(then, None).expect("a new block");
    let jump = run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(tail)], Flags::default(), None);
    run.function.insert(jump, Position::End(then)).expect("a new block");
    let old = run.function.terminator(head).expect("split_block_at ends the head with a jump");
    run.function.set_operands(old, Vec::new());
    run.function.erase(old).expect("a jump nothing reads");
    let branch = run
        .function
        .create_instruction(
            Opcode::Br,
            void,
            vec![operands[0], Operand::Block(then), Operand::Block(tail)],
            Flags::default(),
            None,
        );
    run.function.insert(branch, Position::End(head)).expect("a placed block");
    let phi = run
        .function
        .create_instruction(
            Opcode::Phi,
            ty,
            from_arms(&[(operands[1], then), (operands[2], head)]),
            Flags::default(),
            None,
        );
    run.function.insert(phi, Position::Before(select)).expect("a placed select");
    let old_value = run.function.instruction(select).result.expect("a select's value");
    let new_value = run.function.instruction(phi).result.expect("a phi's value");
    run.function.replace_value(old_value, Operand::Value(new_value));
    run.function.set_operands(select, Vec::new());
    run.function.erase(select).expect("a select nothing reads");
    branch
}

/// A `Query` at `cxt`, with the dominator tree and assumptions of `analyzed`.
fn query_at<'a>(
    run: &'a Run,
    analyzed: &'a Analyzed,
    cxt: Option<InstId>,
) -> Query<'a> {
    Query {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        dt: Some(&analyzed.tree),
        assumptions: Some(&analyzed.assumptions),
        dom_conditions: None,
        cond_context: None,
        cxt,
    }
}

/// `!isGuaranteedNotToBeUndefOrPoison(cond, AC, preheader terminator, DT)`.
pub(super) fn should_insert_freeze_of_select(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    cond: Operand,
) -> bool {
    let preheader = lp.get_loop_preheader(run.function).expect("a loop in simplified form has a preheader");
    let at = run.function.terminator(preheader);
    !is_guaranteed_not_to_be_undef_or_poison(&query_at(run, analyzed, at), cond, 0)
}

/// `shouldInsertFreeze`: the condition of `ti` may be poison and `ti` may not
/// run on every trip, so that hoisting it could make the loop branch on
/// poison where it did not.
pub(super) fn should_insert_freeze(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    cond: Operand,
    ti: InstId,
) -> bool {
    !is_guaranteed_to_execute(run, analyzed, lp, ti) && should_insert_freeze_of_select(run, analyzed, lp, cond)
}

/// `ICFLoopSafetyInfo::isGuaranteedToExecute`: nothing before `inst` in its
/// block may leave without reaching it, and every path of the loop from the
/// header leads to its block (`allLoopPathsLeadToBlock`).
fn is_guaranteed_to_execute(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    inst: InstId,
) -> bool {
    let q = query_at(run, analyzed, None);
    let block = run.function.parent(inst).expect("a placed instruction");
    let may_leave = |inst| !is_guaranteed_to_transfer_execution_to_successor(&q, inst);
    if run.function.iter_block(block).take_while(|&one| one != inst).any(may_leave) {
        return false;
    }
    // The header is always reached once the loop is entered.
    if block == lp.get_header() {
        return true;
    }
    // The blocks of the loop on every path from the header to `block`.
    let mut before: IdSet<BlockId> = IdSet::new();
    let mut work = Vec::new();
    for pred in run.function.predecessors(block) {
        if lp.contains(pred) && before.insert(pred) {
            work.push(pred);
        }
    }
    while let Some(pred) = work.pop() {
        // We are not interested in backedges and we don't want to leave the
        // loop.
        if pred == lp.get_header() {
            continue;
        }
        for next in run.function.predecessors(pred) {
            if lp.contains(next) && before.insert(next) {
                work.push(next);
            }
        }
    }
    // Bail out if a latch block is part of the predecessor set: we may take
    // the backedge to the header and not execute other latch successors.
    if run.function.predecessors(lp.get_header()).iter().any(|pred| before.contains(pred)) {
        return false;
    }
    let mut checked: IdSet<BlockId> = IdSet::new();
    for pred in before.iter() {
        // A predecessor that may throw has a side exit.
        if run.function.iter_block(pred).any(may_leave) {
            return false;
        }
        // `block` dominates `pred`, so if `pred` runs, `block` must run.
        if analyzed.tree.dominates(block, pred) {
            continue;
        }
        for succ in successors(run, pred) {
            if checked.insert(succ)
                && succ != block
                && !before.contains(&succ)
                && (lp.contains(succ) || !cannot_take_first_iteration(run, lp, succ))
            {
                return false;
            }
        }
    }
    true
}

/// `CanProveNotTakenFirstIteration`: the branch that leaves the loop for
/// `exit` is not taken on the first trip, since its compare of a header phi
/// with a bound is decided by the phi's start. SU11: `simplifyCmpInst` of the
/// start and the bound is a compare of two constants.
fn cannot_take_first_iteration(
    run: &Run,
    lp: Loop,
    exit: BlockId,
) -> bool {
    let function = &*run.function;
    let Some(exiting) = edges::single_predecessor(function, exit) else { return false };
    let Some(branch) = function.terminator(exiting) else { return false };
    let end = function.instruction(branch);
    if end.opcode != Opcode::Br || end.operands.len() != 3 {
        return false;
    }
    let (on_true, on_false) = (end.operands[1], end.operands[2]);
    let constant_bits = |operand: Operand| match operand {
        Operand::Constant(id) => match run.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero => Some(0),
            _ => None,
        },
        _ => None,
    };
    // If the condition is constant and false leads to the exit, the loop
    // always runs the true branch.
    if let Some(bits) = constant_bits(end.operands[0]) {
        return (if bits & 1 == 1 { on_false } else { on_true }) == Operand::Block(exit);
    }
    let Some(compare) = def_of(run, end.operands[0]) else { return false };
    let Opcode::ICmp(mut predicate) = function.instruction(compare).opcode else { return false };
    let is_header_phi = |operand: Operand| {
        def_of(run, operand).is_some_and(|phi| {
            function.instruction(phi).opcode == Opcode::Phi && function.parent(phi) == Some(lp.get_header())
        })
    };
    let (mut lhs, mut rhs) = (function.instruction(compare).operands[0], function.instruction(compare).operands[1]);
    if !is_header_phi(lhs) {
        predicate = predicate.swapped();
        (lhs, rhs) = (rhs, lhs);
        if !is_header_phi(lhs) {
            return false;
        }
    }
    // The induction variable starts at the value coming into the header from
    // outside the loop.
    let Some(predecessor) = lp.get_loop_predecessor(function) else { return false };
    let phi = def_of(run, lhs).expect("a header phi");
    let Some((start, _)) = arms(function, phi).into_iter().find(|&(_, from)| from == predecessor) else {
        return false;
    };
    let (Some(start), Some(bound)) = (constant_bits(start), constant_bits(rhs)) else { return false };
    let ty = function.operand_type(run.context, lhs).expect("a typed phi");
    let width = run.context.types.int_bits(ty).unwrap_or(64);
    let truth = predicate.holds(start, bound, width);
    // The exit is the true target when the condition holds.
    if on_true == Operand::Block(exit) { !truth } else { truth }
}

/// Every block of `root`'s dominator subtree.
fn dom_subtree(
    tree: &DomTree,
    root: BlockId,
) -> Vec<BlockId> {
    let mut seen = vec![root];
    let mut at = 0;
    while at < seen.len() {
        let children = tree.children(Some(seen[at]));
        seen.extend(children);
        at += 1;
    }
    seen
}

/// `RecursivelyDeleteTriviallyDeadInstructions` of `root`.
fn delete_trivially_dead(
    run: &mut Run,
    root: Operand,
) {
    let mut work: Vec<InstId> = def_of(run, root).into_iter().collect();
    while let Some(inst) = work.pop() {
        if run.function.is_erased(inst)
            || !crate::local::is_instruction_trivially_dead(run.context, run.callees, run.globals, run.function, inst)
        {
            continue;
        }
        let operands = run.function.instruction(inst).operands.clone();
        run.function.set_operands(inst, Vec::new());
        run.function.erase(inst).expect("a dead instruction");
        work.extend(operands.into_iter().filter_map(|one| def_of(run, one)));
    }
}

/// `deleteDeadBlocksFromLoop`'s erasing: the values of the dead blocks read
/// anywhere become poison, the references between them are dropped, and the
/// blocks go.
fn erase_dead_blocks(
    run: &mut Run,
    dead: &[BlockId],
) {
    let poison = |run: &Run, value| {
        let ty = run.function.value(value).ty;
        Operand::Constant(
            run.context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
        )
    };
    let insts: Vec<InstId> =
        dead.iter().flat_map(|&block| run.function.iter_block(block).collect::<Vec<_>>()).collect();
    for &inst in &insts {
        if let Some(result) = run.function.instruction(inst).result
            && !run.function.users(result).is_empty()
        {
            let replacement = poison(run, result);
            run.function.replace_value(result, replacement);
        }
    }
    for &inst in &insts {
        run.function.set_operands(inst, Vec::new());
    }
    run.function.erase_all(&insts).expect("the dead blocks' instructions");
    run.function.erase_blocks(dead).expect("the dead blocks are empty and nothing names them");
}

/// `buildClonedLoopBlocks`: the preheader, the loop's blocks and the exit
/// blocks a copy for `unswitched` needs, cloned and wired. The clone of the
/// preheader and the map from each original block to its clone.
#[allow(clippy::too_many_arguments)]
fn build_cloned_loop_blocks(
    run: &mut Run,
    lp: Loop,
    loop_ph: BlockId,
    exit_blocks: &[BlockId],
    parent_bb: BlockId,
    unswitched: BlockId,
    dominating_succ: &IdMap<BlockId, BlockId>,
) -> (BlockId, ValueMap) {
    let mut vmap = ValueMap::default();
    let mut new_blocks: Vec<BlockId> = Vec::new();
    // We skip cloning blocks when they have a dominating succ that is not the
    // succ we are cloning for.
    let skip = |block: BlockId| dominating_succ.get(&block).is_some_and(|&succ| succ != unswitched);
    let clone = |run: &mut Run, old: BlockId, vmap: &mut ValueMap, new_blocks: &mut Vec<BlockId>| {
        let new = run.function.clone_block(old, vmap);
        run.function.insert_block(new, None).expect("a new block");
        new_blocks.push(new);
        new
    };

    let cloned_ph = clone(run, loop_ph, &mut vmap, &mut new_blocks);
    for &block in lp.get_blocks() {
        if !skip(block) {
            clone(run, block, &mut vmap, &mut new_blocks);
        }
    }

    // Split all the loop exit edges so that when we clone the exit blocks, if
    // any of the exit blocks are *also* a preheader for some other loop, we
    // don't create multiple predecessors entering the loop header.
    for &exit in exit_blocks {
        if skip(exit) {
            continue;
        }
        let merge = edges::split_block(run.function, exit);
        let cloned_exit = clone(run, exit, &mut vmap, &mut new_blocks);
        // Create a merge phi for each exit phi.
        for phi in phis(run.function, exit) {
            let ty = run.function.instruction(phi).ty;
            let result = run.function.instruction(phi).result.expect("a phi's value");
            let cloned = match vmap.values.get(&result) {
                Some(&Operand::Value(one)) => Operand::Value(one),
                _ => unreachable!("every exit phi is cloned"),
            };
            let first =
                run.function.iter_block(merge).find(|&inst| run.function.instruction(inst).opcode != Opcode::Phi);
            let merged = run.function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
            match first {
                Some(first) => run.function.insert(merged, Position::Before(first)),
                None => run.function.insert(merged, Position::End(merge)),
            }
            .expect("a placed block");
            let merged_value = run.function.instruction(merged).result.expect("a phi's value");
            run.function.replace_value(result, Operand::Value(merged_value));
            run.function.set_operands(merged, from_arms(&[(Operand::Value(result), exit), (cloned, cloned_exit)]));
        }
    }

    // Rewrite the instructions in the cloned blocks to refer to the
    // instructions in the cloned blocks.
    for &block in &new_blocks {
        for inst in run.function.iter_block(block).collect::<Vec<_>>() {
            run.function.remap_instruction(inst, &vmap);
        }
    }

    // Update any phis in the cloned successors of the skipped blocks to not
    // have spurious incoming values.
    for &block in lp.get_blocks() {
        if !skip(block) {
            continue;
        }
        for succ in successors(run, block) {
            if let Some(&cloned_succ) = vmap.blocks.get(&succ) {
                for phi in phis(run.function, cloned_succ) {
                    run.function.remove_incoming_value(run.context, phi, block, false);
                }
            }
        }
    }

    // Remove the cloned parent as a predecessor of any successor we ended up
    // cloning other than the unswitched one.
    let cloned_parent = vmap.blocks[&parent_bb];
    for succ in successors(run, parent_bb) {
        if succ == unswitched {
            continue;
        }
        if let Some(&cloned_succ) = vmap.blocks.get(&succ) {
            run.function.predecessor_removed(run.context, cloned_succ, cloned_parent, true);
        }
    }

    // Replace the cloned branch with an unconditional branch to the cloned
    // unswitched successor.
    let cloned_succ = vmap.blocks[&unswitched];
    let terminator = run.function.terminator(cloned_parent).expect("a terminated clone");
    let void = run.function.instruction(terminator).ty;
    let condition = run.function.instruction(terminator).operands[0];
    run.function.set_operands(terminator, Vec::new());
    run.function.erase(terminator).expect("a terminator nothing reads");
    let jump =
        run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(cloned_succ)], Flags::default(), None);
    run.function.insert(jump, Position::End(cloned_parent)).expect("a placed block");
    delete_trivially_dead(run, condition);

    // If there are duplicate entries in the phis because of multiple edges to
    // the unswitched successor, we need to nuke all but one as we replaced it
    // with a direct branch.
    for phi in phis(run.function, cloned_succ) {
        let mut incoming = arms(run.function, phi);
        let mut found = false;
        incoming.retain(|&(_, from)| {
            if from != cloned_parent {
                return true;
            }
            !std::mem::replace(&mut found, true)
        });
        run.function.set_operands(phi, from_arms(&incoming));
    }
    (cloned_ph, vmap)
}

/// What an unswitch made: the headers of the loops cloned.
pub(super) struct Unswitched {
    pub cloned_headers: Vec<BlockId>,
}

/// `unswitchNontrivialInvariants`, for a candidate that is a branch or switch
/// (a `select` has been turned into a branch). Leaves the function in
/// loop-simplify and LCSSA form.
pub(super) fn unswitch_nontrivial_invariants(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
    candidate: &Candidate,
    insert_freeze: bool,
) -> Unswitched {
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    let ti = candidate.terminator;
    let invariants = &candidate.invariants;
    let parent_bb = run.function.parent(ti).expect("a placed terminator");
    let operands = run.function.instruction(ti).operands.clone();
    let is_switch = run.function.instruction(ti).opcode == Opcode::Switch;
    let void = run.function.instruction(ti).ty;
    let cond = if is_switch { operands[0] } else { skip_trivial_select(run, operands[0]) };
    // We can only unswitch switches, and conditional branches with an
    // invariant condition, or combining invariant conditions with an
    // instruction.
    let full_unswitch = is_switch || cond == invariants[0];

    // When we are unswitching a set of invariants combined with `and` or `or`,
    // the combining operation determines the best direction to unswitch: we
    // want to unswitch the direction that will collapse the branch.
    let mut direction = true;
    let mut cloned_succ = 0usize;
    if !full_unswitch {
        let root = def_of(run, cond).expect("a partial unswitch has an instruction as the condition");
        if logical(run, root, false).is_none() && logical(run, root, true).is_some() {
            direction = false;
            cloned_succ = 1;
        }
    }

    let block_at = |operand: Operand| match operand {
        Operand::Block(block) => block,
        _ => unreachable!("a terminator's target"),
    };
    let retained_succ = if is_switch { block_at(operands[1]) } else { block_at(operands[2 - cloned_succ]) };
    let mut unswitched_succs: Vec<BlockId> = Vec::new();
    if is_switch {
        for pair in operands[2..].chunks(2) {
            let to = block_at(pair[1]);
            if to != retained_succ && !unswitched_succs.contains(&to) {
                unswitched_succs.push(to);
            }
        }
    } else {
        unswitched_succs.push(block_at(operands[1 + cloned_succ]));
    }

    let exit_blocks = lp.get_unique_exit_blocks(run.function);

    // If the edge from this terminator to a successor dominates that
    // successor, store a map from each block in its dominator subtree to it.
    // This lets us tell when cloning for a particular successor if a block is
    // dominated by some *other* successor with a single data structure.
    let mut dominating_succ: IdMap<BlockId, BlockId> = IdMap::new();
    for &succ in std::iter::once(&retained_succ).chain(&unswitched_succs) {
        let preds = run.function.predecessors(succ);
        if preds.len() == 1 || preds.iter().all(|&pred| pred == parent_bb || analyzed.tree.dominates(succ, pred)) {
            for block in dom_subtree(&analyzed.tree, succ) {
                dominating_succ.insert(block, succ);
            }
        }
    }

    // Split the preheader, so that we know that there is a safe place to
    // insert the conditional branch. The original preheader becomes the split
    // point between the unswitched versions, and the original loop gets a new
    // preheader.
    let split_bb = lp.get_loop_preheader(run.function).expect("a loop in simplified form has a preheader");
    let loop_ph = edges::split_edge(run.function, split_bb, header);

    // Clone the loop for each unswitched successor.
    let mut vmaps: Vec<ValueMap> = Vec::new();
    let mut cloned_phs: IdMap<BlockId, BlockId> = IdMap::new();
    for &succ in &unswitched_succs {
        let (cloned_ph, vmap) =
            build_cloned_loop_blocks(run, lp, loop_ph, &exit_blocks, parent_bb, succ, &dominating_succ);
        cloned_phs.insert(succ, cloned_ph);
        vmaps.push(vmap);
    }

    // The stitching of the branched code back together depends on whether
    // we're doing full unswitching or not with the exception that we always
    // want to nuke the initial terminator placed in the split block.
    let jump = run.function.terminator(split_bb).expect("a terminated split block");
    run.function.set_operands(jump, Vec::new());
    run.function.erase(jump).expect("a jump nothing reads");
    if full_unswitch {
        // Keep a clone of the terminator, to take the parent off the
        // successors it leaves.
        let new_ti = run.function.clone_instruction(ti);
        // Splice the terminator from the original loop and rewrite its
        // successors.
        run.function.move_to(ti, Position::End(split_bb)).expect("a placed terminator");
        run.function.insert(new_ti, Position::End(parent_bb)).expect("a block without a terminator");
        let mut moved = operands.clone();
        if is_switch {
            moved[1] = Operand::Block(loop_ph);
            for pair in moved[2..].chunks_mut(2) {
                pair[1] = Operand::Block(if block_at(pair[1]) == retained_succ {
                    loop_ph
                } else {
                    cloned_phs[&block_at(pair[1])]
                });
            }
        } else {
            moved[1 + cloned_succ] = Operand::Block(cloned_phs[&unswitched_succs[0]]);
            moved[2 - cloned_succ] = Operand::Block(loop_ph);
        }
        run.function.set_operands(ti, moved);
        let mut value = cond;
        if insert_freeze {
            let ty = run.function.operand_type(run.context, value).expect("a typed condition");
            let freeze = run.function.create_instruction(Opcode::Freeze, ty, vec![value], Flags::default(), None);
            run.function.insert(freeze, Position::Before(ti)).expect("a placed terminator");
            value = Operand::Value(run.function.instruction(freeze).result.expect("a freeze's value"));
        }
        run.function.set_operand(ti, 0, value);

        // Now unhook the successor relationship as we'll be replacing the
        // terminator with a direct branch.
        if is_switch {
            // The parent is no longer a predecessor of *every* case
            // successor: an unswitched one loses the edge, and a duplicate
            // edge to the retained successor is one the direct branch does not
            // need.
            for pair in operands[2..].chunks(2) {
                run.function.predecessor_removed(run.context, block_at(pair[1]), parent_bb, true);
            }
        } else {
            run.function.predecessor_removed(run.context, unswitched_succs[0], parent_bb, true);
        }
        // Create a new unconditional branch to the continuing block (as
        // opposed to the one cloned).
        run.function.set_operands(new_ti, Vec::new());
        run.function.erase(new_ti).expect("a terminator nothing reads");
        let jump = run
            .function
            .create_instruction(Opcode::Br, void, vec![Operand::Block(retained_succ)], Flags::default(), None);
        run.function.insert(jump, Position::End(parent_bb)).expect("a block without a terminator");
    } else {
        super::build_partial_unswitch_conditional_branch(
            run,
            split_bb,
            invariants,
            direction,
            cloned_phs[&unswitched_succs[0]],
            loop_ph,
            true,
            Some((analyzed, ti)),
        );
    }

    // With the edges wired, the dominator tree tells which cloned blocks no
    // edge reaches and which copy each use of an invariant is in (SU7).
    let tree = DomTree::of(run.function);
    let mut dead: Vec<BlockId> = Vec::new();
    for &block in lp.get_blocks().iter().chain(&exit_blocks) {
        for vmap in &vmaps {
            if let Some(&cloned) = vmap.blocks.get(&block)
                && !tree.is_reachable_from_entry(cloned)
            {
                for succ in successors(run, cloned) {
                    run.function.predecessor_removed(run.context, succ, cloned, false);
                }
                dead.push(cloned);
            }
        }
    }
    erase_dead_blocks(run, &dead);

    // The original loop and its exits lose the blocks no edge reaches, and the
    // blocks only they reached.
    let mut dead_set: IdSet<BlockId> = IdSet::new();
    let mut dead: Vec<BlockId> = Vec::new();
    let mut candidates: Vec<BlockId> = exit_blocks.iter().chain(lp.get_blocks()).copied().collect();
    while let Some(block) = candidates.pop() {
        if !dead_set.contains(&block) && !tree.is_reachable_from_entry(block) {
            for succ in successors(run, block) {
                run.function.predecessor_removed(run.context, succ, block, false);
                candidates.push(succ);
            }
            dead_set.insert(block);
            dead.push(block);
        }
    }
    erase_dead_blocks(run, &dead);

    if !is_switch {
        // If we unswitched a branch which collapses the condition to a known
        // constant we want to replace all the uses of the invariants within
        // both the original and cloned blocks.
        let cloned_ph = cloned_phs[&unswitched_succs[0]];
        // When considering multiple partially-unswitched invariants we can't
        // just go replace them with constants in both branches: for `and`, the
        // continue side means true for each invariant, and for `or` false, so
        // the unswitched side learns nothing from several of them.
        let replace_unswitched = full_unswitch || invariants.len() == 1;
        for &invariant in invariants {
            let Operand::Value(value) = invariant else { unreachable!("an invariant is a value") };
            let ty = run.function.value(value).ty;
            let unswitched_replacement = Operand::Constant(run.context.int(ty, i128::from(direction)));
            let continue_replacement = Operand::Constant(run.context.int(ty, i128::from(!direction)));
            let uses: Vec<_> = run.function.users(value).iter().map(|one| (one.user, one.index)).collect();
            for (user, index) in uses {
                let Some(block) = run.function.parent(user) else { continue };
                // Replace it with the 'continue' side if in the main loop
                // body, and the unswitched if in the cloned blocks.
                if tree.dominates(loop_ph, block) {
                    run.function.set_operand(user, index as usize, continue_replacement);
                } else if replace_unswitched && tree.dominates(cloned_ph, block) {
                    run.function.set_operand(user, index as usize, unswitched_replacement);
                }
            }
        }
    }

    // We can change which blocks are exit blocks of all the cloned sibling
    // loops, the current loop, and any parent loops which shared exit blocks
    // with the current loop: re-form LCSSA, then dedicated exits (SU8).
    let _ = crate::lcssa::formed(run.function);
    crate::loopsimplify::simplified(run.function);

    let cloned_headers = vmaps.iter().filter_map(|vmap| vmap.blocks.get(&header).copied()).collect();
    Unswitched { cloned_headers }
}

#[cfg(test)]
#[path = "../simpleloopunswitch_nontrivial_tests.rs"]
mod tests;
