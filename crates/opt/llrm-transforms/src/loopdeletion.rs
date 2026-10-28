//! `LoopDeletion`: a loop that computes nothing the rest of the function reads
//! is removed, and a loop whose backedge is never taken is made straight code.
//!
//! # How it works
//! For each loop, innermost first, `delete_loop_if_dead` needs a preheader and
//! dedicated exits. A loop the preheader's predecessors never enter (each ends
//! in a constant branch away from it) is deleted at once, its exit phis made
//! poison. Otherwise the loop must have at most one exit block and be dead
//! (`is_loop_dead`): the values its exit phis take are the same from every
//! exiting block and loop-invariant (they are hoisted to the preheader where
//! they can be), no instruction has a side effect, and the loop ends: the
//! function is `mustprogress`, or each loop in it is marked so or has a
//! constant bound on its backedge count from ScalarEvolution.
//!
//! Deleting makes the preheader jump to the exit, gives each exit phi the one
//! value from the preheader, and erases the loop's blocks.
//!
//! A loop that stays gets `break_backedge_if_not_taken`: if its backedge count
//! is zero, or its first iteration provably leaves
//! (`can_prove_exit_on_first_iteration`: the blocks and edges live on the
//! first iteration, found by evaluating the header phis on entry), the latch no
//! longer jumps back, and the loop is straight code.
//!
//! # Deviations
//! - LD1 `makeLoopInvariant` keeps an instruction's `prof` metadata wherever it
//!   was, where LLVM keeps it only for an instruction of the header
//!   ([`crate::twoentryphi::metadata_dropped`]).
//! - LD2 No `MemorySSA`, optimization remarks or debug records: the records of
//!   a deleted loop are dropped, not moved to the exit.
//! - LD3 After a loop with an outer loop loses its backedge, the whole function
//!   is put in LCSSA form again, where LLVM does `formLCSSARecursively` of the
//!   outermost loop.
//! - LD5 The blocks of deleted loops are erased together when the pass ends.
//! - LD4 The analyses are derived again after a change that alters the loops
//!   around it; after the deletion of a loop with no loop around or inside it
//!   they are kept (made again after each, `n` deletions in a function were
//!   `n^2`).
//!
//! # Example
//! A loop that only counts, read by nobody:
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
//!   %next = add i16 %i, 1
//!   %go = icmp slt i16 %next, 10
//!   br i1 %go, label %b1, label %b2
//! ```
//! becomes a jump from `b0` to `b2`.
//!
//! # See also
//! [`LoopDeletionPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopDeletion.cpp#L516),
//! [`deleteLoopIfDead`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopDeletion.cpp#L436),
//! [`deleteDeadLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L521),
//! [`breakLoopBackedge`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L746).

use std::rc::Rc;

use llrm_analysis::scev::analysis::Scevs;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::AssumptionCache;

use crate::lcssa::{arms, from_arms};

/// What the pass reads and changes.
pub struct Run<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub callees: &'a Callees,
    pub outer: &'a Outer,
    pub function: &'a mut Function,
    /// Blocks of deleted loops, empty, erased when the pass is done: erasing
    /// scans the layout, so each deletion would.
    pub emptied: Vec<BlockId>,
    /// The expressions of the passes before (`Scevs`), current while the
    /// function is as they left it: dropped at the first change.
    pub scev: ScalarEvolution,
}

/// `LoopDeletionResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Result {
    Unmodified,
    Modified,
    Deleted,
}

/// `merge`: the stronger of two results.
fn merge(
    a: Result,
    b: Result,
) -> Result {
    if a == Result::Deleted || b == Result::Deleted {
        Result::Deleted
    } else if a == Result::Modified || b == Result::Modified {
        Result::Modified
    } else {
        Result::Unmodified
    }
}

struct Analyzed {
    tree: Rc<DomTree>,
    loops: Rc<LoopInfo>,
    assumptions: AssumptionCache,
}

impl Analyzed {
    fn of(run: &Run) -> Self {
        let tree = Rc::new(DomTree::of(run.function));
        let loops = Rc::new(LoopInfo::analyze(run.function, || &*tree));
        let assumptions = AssumptionCache::of(run.context, run.globals, run.metadata, run.function);
        Self { tree, loops, assumptions }
    }

    /// The manager's trees: LLVM's `LoopStandardAnalysisResults` hands the
    /// pass the ones it holds.
    fn held(
        run: &Run,
        tree: Rc<DomTree>,
        loops: Rc<LoopInfo>,
    ) -> Self {
        let assumptions = AssumptionCache::of(run.context, run.globals, run.metadata, run.function);
        Self { tree, loops, assumptions }
    }
}

/// The first non-phi instruction of `block` is a landing pad: `isEHPad`.
fn is_eh_pad(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .map(|inst| &function.instruction(inst).opcode)
        .find(|opcode| **opcode != Opcode::Phi)
        .is_some_and(|opcode| matches!(opcode, Opcode::LandingPad { .. }))
}

/// `Instruction::isDroppable`: a call of `llvm.assume`.
fn is_droppable(
    run: &Run,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(run.context, run.function, inst)
        .and_then(|global| run.globals.get(global.0 as usize))
        .and_then(GlobalValue::intrinsic)
        == Some(Intrinsic::Assume)
}

/// `Instruction::mayHaveSideEffects`.
fn has_side_effects(
    run: &Run,
    inst: InstId,
) -> bool {
    crate::local::may_have_side_effects(run.context, run.callees, run.globals, run.function, inst)
}

/// `Loop::makeLoopInvariant(I, Changed, InsertPt)`: `inst` and what it needs
/// are moved before `insert` if they can be speculated and read no memory.
fn make_loop_invariant(
    run: &mut Run,
    lp: Loop,
    inst: InstId,
    changed: &mut bool,
    insert: InstId,
) -> bool {
    // Already loop-invariant.
    if !lp.contains_instruction(run.function, inst) {
        return true;
    }
    if !crate::licm::hoist::is_safe_to_speculatively_execute_in(run.context, run.layout, run.function, run.outer, inst)
    {
        return false;
    }
    if llrm_mir::memory::of(run.context, run.callees, run.function, inst).reads {
        return false;
    }
    // EH block instructions are immobile.
    if matches!(
        run.function.instruction(inst).opcode,
        Opcode::LandingPad { .. }
    ) {
        return false;
    }
    // Don't hoist instructions with loop-variant operands.
    for operand in run.function.instruction(inst).operands.clone() {
        if let Operand::Value(value) = operand
            && let ValueDef::Instruction(def) = run.function.value(value).def
            && !make_loop_invariant(run, lp, def, changed, insert)
        {
            return false;
        }
    }
    run.function.move_to(inst, Position::Before(insert)).expect("a placed instruction");
    crate::twoentryphi::metadata_dropped(run.function, inst);
    *changed = true;
    true
}

/// `LoopBlocksRPO`: the blocks of the loop from its header, reverse postorder.
pub(crate) fn loop_blocks_rpo(
    function: &Function,
    lp: Loop,
) -> Vec<BlockId> {
    let header = lp.get_header();
    let mut seen = IdSet::from_iter([header]);
    let mut post = Vec::new();
    let mut stack = vec![(header, function.successors(header).into_iter().collect::<Vec<_>>())];
    while let Some((at, pending)) = stack.last_mut() {
        if let Some(next) = pending.pop() {
            if lp.contains(next) && seen.insert(next) {
                let successors = function.successors(next).into_iter().collect::<Vec<_>>();
                stack.push((next, successors));
            }
        } else {
            post.push(*at);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// `containsIrreducibleCFG`: an edge back to a block already visited that is
/// not a backedge of a loop around its source.
pub(crate) fn contains_irreducible_cfg(
    function: &Function,
    info: &LoopInfo,
    rpo: &[BlockId],
) -> bool {
    let proper_backedge = |src: BlockId, dst: BlockId| {
        let mut current = info.get_loop_for(src);
        while let Some(lp) = current {
            if lp.get_header() == dst {
                return true;
            }
            current = lp.get_parent_loop();
        }
        false
    };
    let mut visited = IdSet::<BlockId>::new();
    for &node in rpo {
        visited.insert(node);
        for succ in function.successors(node) {
            if visited.contains(&succ) && !proper_backedge(node, succ) {
                return true;
            }
        }
    }
    false
}

/// `isLoopDead`: the loop changes nothing the rest of the function reads, and
/// ends.
fn is_loop_dead(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    exiting_blocks: &[BlockId],
    exit_block: Option<BlockId>,
    changed: &mut bool,
    preheader: BlockId,
) -> bool {
    let mut se = std::mem::replace(&mut run.scev, ScalarEvolution::new());
    let dead = is_loop_dead_with(run, analyzed, lp, exiting_blocks, exit_block, changed, preheader, &mut se);
    run.scev = se;
    dead
}

#[allow(clippy::too_many_arguments)]
fn is_loop_dead_with(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    exiting_blocks: &[BlockId],
    exit_block: Option<BlockId>,
    changed: &mut bool,
    preheader: BlockId,
    se: &mut ScalarEvolution,
) -> bool {
    // Make sure that all PHI entries coming from the loop are loop invariant.
    // Because the code is in LCSSA form, any values used outside of the loop
    // must pass through a PHI in the exit block, meaning that this check is
    // sufficient to guarantee that no loop-variant values are used outside of
    // the loop.
    let mut all_entries_invariant = true;
    let mut all_outgoing_values_same = true;
    if let Some(exit) = exit_block {
        let insert = run.function.terminator(preheader).expect("a terminated preheader");
        for phi in crate::edges::phis(run.function, exit) {
            let incoming_from = |run: &Run, block: BlockId| {
                arms(run.function, phi).into_iter().find(|&(_, from)| from == block).map(|(value, _)| value)
            };
            let incoming = incoming_from(run, exiting_blocks[0]);
            // Make sure all exiting blocks produce the same incoming value for
            // the block. If there are different incoming values for different
            // exiting blocks, then it is impossible to statically determine
            // which value should be used.
            all_outgoing_values_same = exiting_blocks[1..].iter().all(|&block| incoming == incoming_from(run, block));
            if !all_outgoing_values_same {
                break;
            }
            if let Some(Operand::Value(value)) = incoming
                && let ValueDef::Instruction(def) = run.function.value(value).def
                && !make_loop_invariant(run, lp, def, changed, insert)
            {
                all_entries_invariant = false;
                break;
            }
        }
    }
    if !all_entries_invariant || !all_outgoing_values_same {
        return false;
    }

    // Make sure that no instructions in the block have potential side-effects.
    // This includes instructions that could write to memory, and loads that
    // are marked volatile.
    for &block in lp.get_blocks() {
        for inst in run.function.iter_block(block) {
            if has_side_effects(run, inst) && !is_droppable(run, inst) {
                return false;
            }
        }
    }

    // The loop or any of its sub-loops looping infinitely is legal. The loop
    // can only be considered dead if either
    // a. the function is mustprogress.
    // b. all (sub-)loops are mustprogress or have a known trip-count.
    if llrm_mir::facts::Facts::of(&run.function.attrs).must_progress() {
        return true;
    }

    let rpo = loop_blocks_rpo(run.function, lp);
    // If the loop contains an irreducible cycle, it may loop infinitely.
    if contains_irreducible_cfg(run.function, &analyzed.loops, &rpo) {
        return false;
    }

    let cx = cx_of(run, analyzed);
    let mut work = vec![lp];
    while let Some(current) = work.pop() {
        let marked = current
            .get_loop_latch(cx.function)
            .is_some_and(|latch| llrm_mir::loopinfo::marked(run.metadata, cx.function, latch));
        if marked {
            continue;
        }
        let s = se.get_backedge_taken_count(&cx, current.get_header(), ExitCountKind::ConstantMaximum);
        if se.is_could_not_compute(s) {
            return false;
        }
        work.extend(current.get_sub_loops());
    }
    true
}

fn cx_of<'a>(
    run: &'a Run,
    analyzed: &'a Analyzed,
) -> Cx<'a> {
    Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    }
}

/// `isLoopNeverExecuted`: no viable path from the entry block to the header,
/// by a local search.
fn is_loop_never_executed(
    run: &Run,
    preheader: BlockId,
) -> bool {
    if run.function.entry() == Some(preheader) {
        return false;
    }
    // All predecessors of the preheader should have a constant conditional
    // branch, with the loop's preheader as not-taken.
    for pred in run.function.predecessors(preheader) {
        let Some(term) = run.function.terminator(pred) else { return false };
        let end = run.function.instruction(term);
        let (Opcode::Br, [Operand::Constant(cond), Operand::Block(first), Operand::Block(second)]) =
            (&end.opcode, &end.operands[..])
        else {
            return false;
        };
        let ConstantKind::Int(bits) = run.context.get(*cond).kind else { return false };
        let taken = if bits != 0 { *first } else { *second };
        if taken == preheader {
            return false;
        }
    }
    true
}

/// `getValueOnFirstIteration`: `value` where the header phis are their
/// entry values, found by simplifying; `value` itself where it does not.
fn value_on_first_iteration(
    run: &mut Run,
    first_iter: &mut IdMap<ValueId, Operand>,
    operand: Operand,
) -> Operand {
    // Quick hack: do not flood cache with non-instruction values.
    let Operand::Value(value) = operand else { return operand };
    let ValueDef::Instruction(inst) = run.function.value(value).def else { return operand };
    // Do we already know cached result?
    if let Some(&existing) = first_iter.get(&value) {
        return existing;
    }
    let instruction = run.function.instruction(inst);
    let (opcode, operands) = (instruction.opcode.clone(), instruction.operands.clone());
    let mut found = None;
    match opcode {
        Opcode::Binary(op) => {
            let lhs = value_on_first_iteration(run, first_iter, operands[0]);
            let rhs = value_on_first_iteration(run, first_iter, operands[1]);
            found = llrm_mir::simplify::simplify_binop_operands(
                run.context,
                run.layout,
                run.function,
                op,
                Flags::default(),
                lhs,
                rhs,
            );
        }
        Opcode::ICmp(predicate) => {
            let lhs = value_on_first_iteration(run, first_iter, operands[0]);
            let rhs = value_on_first_iteration(run, first_iter, operands[1]);
            found =
                llrm_mir::simplify::simplify_icmp_operands(run.context, run.layout, run.function, predicate, lhs, rhs);
        }
        Opcode::Select => {
            let condition = value_on_first_iteration(run, first_iter, operands[0]);
            if let Operand::Constant(id) = condition
                && let ConstantKind::Int(bits) = run.context.get(id).kind
            {
                // `isAllOnesValue` of an `i1`.
                let selected = if bits == 1 { operands[1] } else { operands[2] };
                found = Some(value_on_first_iteration(run, first_iter, selected));
            }
        }
        _ => {}
    }
    let result = found.unwrap_or(operand);
    first_iter.insert(value, result);
    result
}

/// `canProveExitOnFirstIteration`: one of the conditions that dominate the
/// latch must leave on the first iteration.
fn can_prove_exit_on_first_iteration(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> bool {
    let (Some(predecessor), Some(latch)) = (lp.get_loop_predecessor(run.function), lp.get_loop_latch(run.function))
    else {
        return false;
    };
    let rpo = loop_blocks_rpo(run.function, lp);
    // For the optimization to be correct, we need RPOT to have a property that
    // each block is processed after all its predecessors, which may only be
    // violated for headers of the current loop and all nested loops.
    // Irreducible CFG provides multiple ways to break this assumption, so we
    // do not want to deal with it.
    if contains_irreducible_cfg(run.function, &analyzed.loops, &rpo) {
        return false;
    }

    let header = lp.get_header();
    // Blocks that are reachable on the 1st iteration.
    let mut live_blocks = IdSet::from_iter([header]);
    // Edges that are reachable on the 1st iteration, by source.
    let mut live_edges = IdMap::<BlockId, Vec<BlockId>>::new();
    let mut first_iter = IdMap::<ValueId, Operand>::new();

    fn mark_live_edge(
        live_blocks: &mut IdSet<BlockId>,
        live_edges: &mut IdMap<BlockId, Vec<BlockId>>,
        from: BlockId,
        to: BlockId,
    ) {
        live_blocks.insert(to);
        if let Some(list) = live_edges.get_mut(&from) {
            list.push(to);
        } else {
            live_edges.insert(from, vec![to]);
        }
    }
    let is_live_edge = |live_edges: &IdMap<BlockId, Vec<BlockId>>, from: BlockId, to: BlockId| {
        live_edges.get(&from).is_some_and(|list| list.contains(&to))
    };

    for &block in &rpo {
        // This block is not reachable on the 1st iterations.
        if !live_blocks.contains(&block) {
            continue;
        }
        // Skip inner loops.
        if analyzed.loops.get_loop_for(block).map(|one| one.get_header()) != Some(header) {
            for succ in run.function.successors(block) {
                mark_live_edge(&mut live_blocks, &mut live_edges, block, succ);
            }
            continue;
        }

        // If Phi has only one input from all live input blocks, use it.
        for phi in crate::edges::phis(run.function, block) {
            let phi_value = run.function.instruction(phi).result.expect("a phi's value");
            if run.context.types.int_bits(run.function.instruction(phi).ty).is_none() {
                continue;
            }
            // `GetSoleInputOnFirstIteration`.
            let sole: Option<Operand> = if block == header {
                arms(run.function, phi).into_iter().find(|&(_, from)| from == predecessor).map(|(value, _)| value)
            } else {
                let mut only: Option<Operand> = None;
                let mut conflict = false;
                for pred in run.function.predecessors(block) {
                    if !is_live_edge(&live_edges, pred, block) {
                        continue;
                    }
                    let incoming = arms(run.function, phi)
                        .into_iter()
                        .find(|&(_, from)| from == pred)
                        .map(|(value, _)| value)
                        .expect("an arm for each predecessor");
                    // Skip poison. If they are present, we can assume they are
                    // equal to the non-poison input.
                    if let Operand::Constant(id) = incoming
                        && run.context.get(id).kind == ConstantKind::Poison
                    {
                        continue;
                    }
                    // Two inputs.
                    if only.is_some_and(|one| one != incoming) {
                        conflict = true;
                        break;
                    }
                    only = Some(incoming);
                }
                if conflict {
                    None
                } else {
                    Some(only.unwrap_or_else(|| {
                        let ty = run.function.instruction(phi).ty;
                        Operand::Constant(run.context.constant(Constant { ty, kind: ConstantKind::Poison }))
                    }))
                }
            };
            let Some(incoming) = sole else { continue };
            let dominates = match incoming {
                Operand::Value(value) => match run.function.value(value).def {
                    ValueDef::Instruction(def) => {
                        run.function.parent(def).is_some_and(|at| analyzed.tree.dominates(at, block))
                    }
                    _ => true,
                },
                _ => true,
            };
            if dominates {
                let first = value_on_first_iteration(run, &mut first_iter, incoming);
                first_iter.insert(phi_value, first);
            }
        }

        let term = run.function.terminator(block).expect("a terminated block");
        let end = run.function.instruction(term);
        let (opcode, operands) = (end.opcode.clone(), end.operands.clone());
        let all_live = |live_blocks: &mut IdSet<BlockId>, live_edges: &mut IdMap<BlockId, Vec<BlockId>>, run: &Run| {
            for succ in run.function.successors(block) {
                mark_live_edge(live_blocks, live_edges, block, succ);
            }
        };
        if opcode == Opcode::Br && operands.len() == 3 {
            let (Operand::Block(if_true), Operand::Block(if_false)) = (operands[1], operands[2]) else {
                unreachable!("a branch's targets")
            };
            let cond_is_icmp = match operands[0] {
                Operand::Value(value) => match run.function.value(value).def {
                    ValueDef::Instruction(def) => matches!(run.function.instruction(def).opcode, Opcode::ICmp(_)),
                    _ => false,
                },
                _ => false,
            };
            if !cond_is_icmp {
                all_live(&mut live_blocks, &mut live_edges, run);
                continue;
            }
            // Can we prove constant true or false for this condition?
            let known = value_on_first_iteration(run, &mut first_iter, operands[0]);
            if known == operands[0] {
                // Failed to simplify.
                all_live(&mut live_blocks, &mut live_edges, run);
                continue;
            }
            let Operand::Constant(id) = known else {
                // Non-constant condition, cannot analyze any further.
                all_live(&mut live_blocks, &mut live_edges, run);
                continue;
            };
            match run.context.get(id).kind {
                ConstantKind::Poison => {
                    // Branching by undef is undefined behavior in theory; we
                    // are conservative. If there is a non-loop successor,
                    // always assume this branch leaves the loop. Otherwise,
                    // arbitrarily take IfTrue.
                    if lp.contains(if_true) && lp.contains(if_false) {
                        mark_live_edge(&mut live_blocks, &mut live_edges, block, if_true);
                    }
                }
                ConstantKind::Int(bits) => {
                    let target = if bits == 1 { if_true } else { if_false };
                    mark_live_edge(&mut live_blocks, &mut live_edges, block, target);
                }
                _ => all_live(&mut live_blocks, &mut live_edges, run),
            }
        } else if opcode == Opcode::Switch {
            let on_first = value_on_first_iteration(run, &mut first_iter, operands[0]);
            let known = match on_first {
                Operand::Constant(id) => match run.context.get(id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    _ => None,
                },
                _ => None,
            };
            let Some(bits) = known else {
                all_live(&mut live_blocks, &mut live_edges, run);
                continue;
            };
            let Operand::Block(default) = operands[1] else { unreachable!("a default target") };
            let target = operands[2..]
                .chunks(2)
                .find_map(|pair| match (pair[0], pair[1]) {
                    (Operand::Constant(case), Operand::Block(to))
                        if run.context.get(case).kind == ConstantKind::Int(bits) =>
                    {
                        Some(to)
                    }
                    _ => None,
                })
                .unwrap_or(default);
            mark_live_edge(&mut live_blocks, &mut live_edges, block, target);
        } else {
            all_live(&mut live_blocks, &mut live_edges, run);
        }
    }

    // We can break the latch if it wasn't live.
    !is_live_edge(&live_edges, latch, header)
}

/// `changeToUnreachable(term, PreserveLCSSA)`: the block ends in `unreachable`
/// and its successors lose their phi arms from it.
fn change_to_unreachable(
    context: &Context,
    function: &mut Function,
    term: InstId,
) {
    let block = function.parent(term).expect("a placed terminator");
    let successors: Vec<BlockId> = function
        .instruction(term)
        .operands
        .iter()
        .filter_map(|&one| if let Operand::Block(to) = one { Some(to) } else { None })
        .collect();
    let void = function.instruction(term).ty;
    let unreachable = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(unreachable, Position::Before(term)).expect("a placed block");
    function.erase(term).expect("the terminator had no users");
    for succ in successors {
        function.predecessor_removed(context, succ, block, true);
    }
}

/// `breakLoopBackedge`: the latch no longer jumps back to the header.
fn break_loop_backedge(
    run: &mut Run,
    lp: Loop,
) {
    let latch = lp.get_loop_latch(run.function).expect("multiple latches are not supported");
    let header = lp.get_header();
    let had_parent = lp.get_parent_loop().is_some();
    let term = run.function.terminator(latch).expect("a terminated latch");
    let end = run.function.instruction(term);
    let void = end.ty;
    if end.opcode == Opcode::Br && end.operands.len() == 1 {
        change_to_unreachable(&*run.context, run.function, term);
    } else if end.opcode == Opcode::Br && end.operands.len() == 3 && lp.is_loop_exiting(run.function, latch) {
        // Conditional latch/exit - note that latch can be shared by inner and
        // outer loop so the other target doesn't need to an exit.
        let Operand::Block(first) = end.operands[1] else { unreachable!("a branch's target") };
        let Operand::Block(second) = end.operands[2] else { unreachable!("a branch's target") };
        let exit = if lp.contains(first) { second } else { first };
        run.function.predecessor_removed(&*run.context, header, latch, true);
        let jump =
            run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(exit)], Flags::default(), None);
        run.function.insert(jump, Position::Before(term)).expect("a placed block");
        // The new branch keeps the old one's metadata but the loop's.
        for (kind, node) in run.function.instruction(term).metadata.clone() {
            if kind == "dbg" || kind == "annotation" {
                run.function.set_metadata(jump, &kind, Some(node));
            }
        }
        run.function.move_records_before(term, jump);
        run.function.erase(term).expect("the branch had no users");
    } else {
        // General case. By splitting the backedge, and then explicitly making
        // it unreachable we gracefully handle corner cases such as switch and
        // invoke terminators.
        let bridge = run.function.create_block(None);
        // `SplitEdge(Latch, Header)`.
        crate::edges::insert_edge_split(run.function, bridge, latch, header);
        let unreachable =
            run.function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
        run.function.insert(unreachable, Position::End(bridge)).expect("a placed block");
        crate::edges::retarget(run.function, term, header, bridge);
        for phi in crate::edges::phis(run.function, header) {
            let kept: Vec<_> = arms(run.function, phi).into_iter().filter(|&(_, from)| from != latch).collect();
            run.function.set_operands(phi, from_arms(&kept));
        }
    }
    // LCSSA form of the outermost loop, which may have lost a block.
    if had_parent {
        let _ = crate::lcssa::closed(run.function);
    }
}

/// `deleteDeadLoop`: the preheader jumps to the exit, and the loop is erased.
fn delete_dead_loop(
    run: &mut Run,
    lp: Loop,
    preheader: BlockId,
) {
    let blocks = lp.get_blocks().to_vec();
    let inside: IdSet<BlockId> = blocks.iter().copied().collect();
    let exit = lp.get_unique_exit_block(run.function);
    let old = run.function.terminator(preheader).expect("a terminated preheader");
    let void = run.function.instruction(old).ty;
    if let Some(exit) = exit {
        // Rewrite phis in the exit block to get their inputs from the
        // Preheader instead of the exiting block: the first input stays, from
        // the preheader.
        for phi in crate::edges::phis(run.function, exit) {
            let first = arms(run.function, phi)[0].0;
            run.function.set_operands(phi, from_arms(&[(first, preheader)]));
        }
        let jump =
            run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(exit)], Flags::default(), None);
        run.function.insert(jump, Position::Before(old)).expect("a placed block");
        // `Preheader->flushTerminatorDbgRecords()`: what was said before the
        // old branch is said before the new one.
        run.function.move_records_before(old, jump);
    } else {
        let unreachable =
            run.function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
        run.function.insert(unreachable, Position::Before(old)).expect("a placed block");
        run.function.move_records_before(old, unreachable);
    }
    run.function.erase(old).expect("the branch had no users");

    // Given LCSSA form is satisfied, we should not have users of instructions
    // within the dead loop outside of the loop. However, LCSSA doesn't take
    // unreachable uses into account: they are made poison here.
    let mut insts = Vec::new();
    for &block in &blocks {
        for inst in run.function.iter_block(block).collect::<Vec<_>>() {
            insts.push(inst);
            if let Some(result) = run.function.instruction(inst).result {
                let ty = run.function.instruction(inst).ty;
                let poison = Operand::Constant(run.context.constant(Constant { ty, kind: ConstantKind::Poison }));
                for user in run.function.users(result).snapshot() {
                    if run.function.parent(user.user).is_some_and(|at| inside.contains(&at)) {
                        continue;
                    }
                    run.function.set_operand(user.user, user.index as usize, poison);
                }
            }
        }
    }
    // A phi outside the loop (in a block nothing reaches) may still name a
    // block of the loop.
    for &block in &blocks {
        for user in run.function.phi_block_users(block).snapshot() {
            let outside = run.function.parent(user.user).is_some_and(|at| !inside.contains(&at));
            if outside && run.function.instruction(user.user).opcode == Opcode::Phi {
                let phi = user.user;
                let kept: Vec<_> = arms(run.function, phi).into_iter().filter(|&(_, from)| from != block).collect();
                run.function.set_operands(phi, from_arms(&kept));
            }
        }
    }
    // Erase the instructions and the blocks without having to worry about
    // ordering.
    run.function.erase_all(&insts).expect("the loop's instructions");
    run.emptied.extend(blocks);
}

/// `breakBackedgeIfNotTaken`.
fn break_backedge_if_not_taken(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> Result {
    let mut se = std::mem::replace(&mut run.scev, ScalarEvolution::new());
    let result = break_backedge_with(run, analyzed, lp, &mut se);
    run.scev = se;
    result
}

fn break_backedge_with(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    se: &mut ScalarEvolution,
) -> Result {
    if lp.get_loop_latch(run.function).is_none() {
        return Result::Unmodified;
    }
    let header = lp.get_header();
    let (max_zero, btc_zero, non_zero) = {
        let cx = cx_of(run, analyzed);
        let is_zero = |se: &ScalarEvolution, s| se.as_constant(s).is_some_and(|value| value.is_zero());
        let btc_max = se.get_backedge_taken_count(&cx, header, ExitCountKind::ConstantMaximum);
        if is_zero(&se, btc_max) {
            (true, true, false)
        } else {
            let btc = se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
            let zero = is_zero(&se, btc);
            let non_zero = !zero && !se.is_could_not_compute(btc) && se.is_known_non_zero(&cx, btc);
            (false, zero, non_zero)
        }
    };
    if !max_zero && !btc_zero {
        if non_zero {
            return Result::Unmodified;
        }
        if !can_prove_exit_on_first_iteration(run, analyzed, lp) {
            return Result::Unmodified;
        }
    }
    break_loop_backedge(run, lp);
    Result::Deleted
}

/// `deleteLoopIfDead`.
fn delete_loop_if_dead(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> Result {
    // We can only remove the loop if there is a preheader that we can branch
    // from after removing it. Also, if LoopSimplify form is not available,
    // stay out of trouble.
    let Some(preheader) = lp.get_loop_preheader(run.function) else { return Result::Unmodified };
    if !lp.has_dedicated_exits(run.function) {
        return Result::Unmodified;
    }
    let exit_block = lp.get_unique_exit_block(run.function);

    // We can't directly branch to an EH pad. Don't bother handling this edge
    // case.
    if exit_block.is_some_and(|exit| is_eh_pad(run.function, exit)) {
        return Result::Unmodified;
    }

    if let Some(exit) = exit_block
        && is_loop_never_executed(run, preheader)
    {
        // Set incoming value to poison for phi nodes in the exit block.
        for phi in crate::edges::phis(run.function, exit) {
            let ty = run.function.instruction(phi).ty;
            let poison = Operand::Constant(run.context.constant(Constant { ty, kind: ConstantKind::Poison }));
            let incoming: Vec<_> = arms(run.function, phi).into_iter().map(|(_, from)| (poison, from)).collect();
            run.function.set_operands(phi, from_arms(&incoming));
        }
        delete_dead_loop(run, lp, preheader);
        return Result::Deleted;
    }

    // The remaining checks below are for a loop being dead because all
    // statements in the loop are invariant.
    let exiting_blocks = lp.get_exiting_blocks(run.function);

    // We require that the loop has at most one exit block. Otherwise, we'd be
    // in the situation of needing to be able to solve statically which exit
    // block will be branched to, or trying to preserve the branching logic in
    // a loop invariant manner.
    if exit_block.is_none() && !analyzed.loops.has_no_exit_blocks(run.function, lp) {
        return Result::Unmodified;
    }

    // Finally, we have to check that the loop really is dead.
    let mut changed = false;
    if !is_loop_dead(run, analyzed, lp, &exiting_blocks, exit_block, &mut changed, preheader) {
        return if changed { Result::Modified } else { Result::Unmodified };
    }
    delete_dead_loop(run, lp, preheader);
    Result::Deleted
}

/// `LoopDeletionPass` over a function: every loop, innermost first. Whether
/// anything changed.
pub fn delete_loops(run: &mut Run) -> bool {
    let analyzed = Analyzed::of(run);
    delete_loops_with(run, analyzed)
}

fn delete_loops_with(
    run: &mut Run,
    mut analyzed: Analyzed,
) -> bool {
    let mut work: std::collections::VecDeque<BlockId> =
        analyzed.loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    let mut changed = false;
    while let Some(header) = work.pop_front() {
        if !analyzed.loops.is_loop_header(header) || !run.function.is_placed(header) {
            continue;
        }
        let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
        let alone = lp.get_parent_loop().is_none() && lp.get_sub_loops().next().is_none();
        let mark = run.function.mark();
        let mut result = delete_loop_if_dead(run, &analyzed, lp);
        // If we can prove the backedge isn't taken, just break it and be done.
        // This leaves the loop structure in place which means it can handle
        // dispatching to the right exit based on whatever loop invariant
        // structure remains.
        let deleted = result == Result::Deleted;
        if !deleted {
            let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
            result = merge(result, break_backedge_if_not_taken(run, &analyzed, lp));
        }
        if result == Result::Unmodified {
            continue;
        }
        changed = true;
        // What the edits reached is forgotten; the other loops keep their
        // counts.
        match run.function.touched_since(mark) {
            Some(touched) => {
                let mut se = std::mem::replace(&mut run.scev, ScalarEvolution::new());
                se.forget_touched(&cx_of(run, &analyzed), &touched);
                run.scev = se;
            }
            None => run.scev = ScalarEvolution::new(),
        }
        // LD4: a deleted loop with no loop around or inside it leaves the
        // other loops' analyses as they were.
        if !(deleted && alone) {
            analyzed = Analyzed::of(run);
        }
    }
    let emptied = std::mem::take(&mut run.emptied);
    run.function.erase_blocks(&emptied).expect("the blocks of deleted loops are empty and unnamed");
    changed
}

/// `LoopDeletionPass` as a slot of the pipeline.
pub struct LoopDeletion;

impl FunctionPass for LoopDeletion {
    fn name(&self) -> &'static str {
        "loopdeletion"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-deletion")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        let mut run = Run {
            context: unit.context,
            layout: unit.layout,
            globals: &outer.globals,
            metadata: unit.metadata,
            callees: outer.callees(),
            outer: &outer,
            function: unit.function,
            emptied: Vec::new(),
            scev: ScalarEvolution::new(),
        };
        run.scev = held.take();
        let tree = analyses.get::<Dominators>(run.context, run.layout, run.function);
        let loops = analyses.get::<Loops>(run.context, run.layout, run.function);
        let analyzed = Analyzed::held(&run, tree, loops);
        let changed = delete_loops_with(&mut run, analyzed);
        held.put(std::mem::replace(&mut run.scev, ScalarEvolution::new()));
        if changed { PreservedAnalyses::none().preserve::<Scevs>() } else { PreservedAnalyses::all() }
    }
}

#[cfg(test)]
#[path = "loopdeletion_tests.rs"]
mod tests;
