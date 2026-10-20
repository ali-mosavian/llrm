//! `SimpleLoopUnswitch`: a branch or switch in a loop on a loop-invariant
//! condition, one side of which leaves the loop, is decided before the loop
//! instead of on every trip (trivial). With `non_trivial`, a condition whose
//! sides both stay in the loop is decided there too, at the price of a copy of
//! the loop for each way (`nontrivial.rs`, chosen by `candidates.rs`).
//!
//! # How it works
//! `unswitch_all_trivial_conditions` walks from the loop header along the
//! blocks the loop would run first. A block with a side effect ends the walk.
//! A conditional branch whose condition (or, for an `and`/`or` of conditions,
//! some of its inputs) is loop-invariant, and one of whose successors is an
//! exit, is unswitched by `unswitch_trivial_branch`; so is a `switch` on an
//! invariant, by `unswitch_trivial_switch`.
//!
//! Unswitching splits the preheader, so that a new preheader holds the
//! loop-invariant test: the branch itself moves there, with the exit as one
//! successor and the new preheader as the other, and in the loop it becomes a
//! jump. The exit block is split where other edges reach it, and the phis of
//! the exit take the value they had on entry. Uses of the condition in the loop
//! become the constant it has when the loop is entered. Nothing is copied.
//!
//! ```text
//!   pre -> H(br c, exit, B) -> B -> H        pre(br c, exit, pre') -> H(br B) -> B -> H
//!                  \-> exit                                    \-> exit
//! ```
//!
//! A loop whose latch is reached from the branch by a latch with no side
//! effect and whose bound ScalarEvolution knows has the branch's latch edge
//! sent to the latch's exit as well.
//!
//! # Deviations
//! - SU1 `revisitCurrentLoop` is not translated (the pipeline is a flat list of
//!   passes): after a loop is unswitched it is looked at again here.
//! - SU2 No `DominatorTree`, `LoopInfo` or `MemorySSA` update (D8 of the plan
//!   page): the analyses are derived again only where the nest changed, which
//!   is when a hoisted loop is put back in loop-simplify and LCSSA form for the
//!   whole function (`hoistLoopToNewParent`, SU3).
//! - SU3 New blocks go at the end of the layout, not beside their neighbours.
//! - SU4 No profile estimate for a partial branch (`EstimateProfile`), no debug
//!   locations, no convergent-call test (MIR has none).
//! - SU5 `isGuaranteedNotToBeUndefOrPoison` of a frozen invariant is the
//!   poison-only form with no dominator tree: MIR has no `undef`.
//! - SU6 ScalarEvolution is made for the one query, not kept and forgotten.
//! - SU7, SU8, SU10, SU11 are the non-trivial form's: see `nontrivial.rs`.
//! - SU9 No partially invariant conditions (`hasPartialIVCondition` and
//!   `buildPartialInvariantUnswitchConditionalBranch`: a loop-header test of
//!   loads MemorySSA shows unchanged on a path), no injected invariant
//!   conditions, no guards (MIR has none). Costs 2 of the 21 corpus functions
//!   `opt` unswitches (4 of 20 rotated), which the oracle classifies. Exit: the
//!   MemorySSA migration lands (mssa-green-2127, 2026-10-29).
//!
//! # Example
//! Before:
//! ```mir
//! b0:
//!   %z = icmp eq i16 %n, 0
//!   br label %head
//! head:
//!   %i = phi i16 [ %m, %b0 ], [ %next, %body ]
//!   br i1 %z, label %out, label %body
//! ```
//! After, the test once, in the old preheader:
//! ```mir
//! b0:
//!   %z = icmp eq i16 %n, 0
//!   br i1 %z, label %out, label %b0.head
//! ```
//!
//! # Runs
//! In the `trivialunswitch@732` slot of LLVM's order, LPM1 after the second
//! LICM. Non-trivially at -O3 and -Omax, where the pin builds
//! `SimpleLoopUnswitchPass(NonTrivial = Level == O3)`; otherwise trivially
//! only.
//!
//! # See also
//! [`unswitchTrivialBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L580),
//! [`unswitchTrivialSwitch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L867),
//! [`unswitchAllTrivialConditions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L1172),
//! [`unswitchLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L3431).

use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;
use llrm_mir::valuetracking::{AssumptionCache, Query};

use crate::edges::{self, phis};
use crate::lcssa::{arms, from_arms};

mod candidates;
mod nontrivial;

/// What the pass reads and changes.
pub struct Run<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub callees: &'a Callees,
    pub tti: &'a dyn Tti,
    pub function: &'a mut Function,
}

struct Analyzed {
    tree: DomTree,
    loops: LoopInfo,
    assumptions: AssumptionCache,
}

impl Analyzed {
    fn of(run: &Run) -> Self {
        let tree = DomTree::of(run.function);
        let loops = LoopInfo::analyze(run.function, || &tree);
        let assumptions = AssumptionCache::of(run.context, run.globals, run.metadata, run.function);
        Self { tree, loops, assumptions }
    }
}

/// What one unswitch did: the loops around the nest changed, and the
/// analyses must be made again.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Step {
    done: bool,
    rebuilt: bool,
}

// ---- glue: what the C++ gets from `Value`, `Instruction` and `IRBuilder` ----

fn def_of(
    run: &Run,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match run.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

fn int_of(
    run: &Run,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match run.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// `skipTrivialSelect`: `(select x, true, false)` is `x`.
fn skip_trivial_select(
    run: &Run,
    mut cond: Operand,
) -> Operand {
    while let Some(inst) = def_of(run, cond) {
        let end = run.function.instruction(inst);
        if end.opcode != Opcode::Select
            || int_of(run, end.operands[1]) != Some(1)
            || int_of(run, end.operands[2]) != Some(0)
        {
            break;
        }
        cond = end.operands[0];
    }
    cond
}

/// `m_LogicalAnd` / `m_LogicalOr` of `inst`: its two inputs, an `and`/`or` or a
/// `select` that acts as one, over `i1`.
fn logical(
    run: &Run,
    inst: InstId,
    and: bool,
) -> Option<(Operand, Operand)> {
    let end = run.function.instruction(inst);
    if run.context.types.int_bits(end.ty) != Some(1) {
        return None;
    }
    match &end.opcode {
        Opcode::Binary(op) if *op == if and { BinaryOp::And } else { BinaryOp::Or } => {
            Some((end.operands[0], end.operands[1]))
        }
        Opcode::Select if and && int_of(run, end.operands[2]) == Some(0) => Some((end.operands[0], end.operands[1])),
        Opcode::Select if !and && int_of(run, end.operands[1]) == Some(1) => Some((end.operands[0], end.operands[2])),
        _ => None,
    }
}

/// `collectHomogenousInstGraphLoopInvariants`: the loop-invariant inputs of the
/// graph of `and`s (or `or`s) `root` is the head of.
fn collect_homogenous_inst_graph_loop_invariants(
    run: &Run,
    lp: Loop,
    root: InstId,
) -> Vec<Operand> {
    let mut invariants = Vec::new();
    let is_and = logical(run, root, true).is_some();
    let is_or = logical(run, root, false).is_some();
    // Build a worklist and recurse through operators collecting invariants.
    let mut worklist = vec![root];
    let mut visited = IdSet::from_iter([root]);
    while let Some(inst) = worklist.pop() {
        for &op in &run.function.instruction(inst).operands {
            // Skip constants as unswitching isn't interesting for them.
            if matches!(op, Operand::Constant(_) | Operand::Block(_)) {
                continue;
            }
            // Add it to our result if loop invariant.
            if lp.is_loop_invariant(run.function, op) {
                invariants.push(op);
                continue;
            }
            // If not an instruction with the same opcode, nothing we can do.
            if let Some(next) = def_of(run, skip_trivial_select(run, op))
                && ((is_and && logical(run, next, true).is_some()) || (is_or && logical(run, next, false).is_some()))
                && visited.insert(next)
            {
                worklist.push(next);
            }
        }
    }
    invariants
}

/// `replaceLoopInvariantUses`: uses of `invariant` in the loop read
/// `replacement`.
fn replace_loop_invariant_uses(
    run: &mut Run,
    lp: Loop,
    invariant: Operand,
    replacement: Operand,
) {
    let Operand::Value(value) = invariant else { unreachable!("why are we unswitching on a constant?") };
    let uses: Vec<_> = run.function.users(value).iter().map(|one| (one.user, one.index)).collect();
    for (user, index) in uses {
        if lp.contains_instruction(run.function, user) {
            run.function.set_operand(user, index as usize, replacement);
        }
    }
}

/// `isLoopHeaderPHI`.
fn is_loop_header_phi(
    run: &Run,
    lp: Loop,
    value: Operand,
) -> bool {
    def_of(run, value).is_some_and(|inst| {
        run.function.instruction(inst).opcode == Opcode::Phi && run.function.parent(inst) == Some(lp.get_header())
    })
}

/// `getLoopEntryValue`: a header phi's value from the preheader, else `value`.
fn loop_entry_value(
    run: &Run,
    lp: Loop,
    value: Operand,
) -> Operand {
    if !is_loop_header_phi(run, lp, value) {
        return value;
    }
    let phi = def_of(run, value).expect("a header phi");
    let preheader = lp.get_loop_preheader(run.function).expect("a preheader");
    arms(run.function, phi)
        .into_iter()
        .find(|&(_, from)| from == preheader)
        .map(|(entry, _)| entry)
        .expect("a header phi has an input from the preheader")
}

/// `areLoopExitPHIsTrivial`: the phis of `exit` take loop-invariant values (or,
/// with `allow_header_phis`, header phis) from `exiting`.
fn are_loop_exit_phis_trivial(
    run: &Run,
    lp: Loop,
    exiting: BlockId,
    exit: BlockId,
    allow_header_phis: bool,
) -> bool {
    for phi in phis(run.function, exit) {
        let incoming = arms(run.function, phi)
            .into_iter()
            .find(|&(_, from)| from == exiting)
            .map(|(value, _)| value)
            .expect("a phi has an input from each predecessor");
        if !lp.is_loop_invariant(run.function, incoming)
            && !(allow_header_phis && is_loop_header_phi(run, lp, incoming))
        {
            return false;
        }
    }
    true
}

/// `Instruction::mayHaveSideEffects`.
fn has_side_effects(
    run: &Run,
    inst: InstId,
) -> bool {
    crate::local::may_have_side_effects(run.context, run.callees, run.globals, run.function, inst)
}

/// `rewritePHINodesForUnswitchedExitBlock`: the exit is the unswitched block;
/// its phis come from the old preheader with their value on entry.
fn rewrite_phi_nodes_for_unswitched_exit_block(
    run: &mut Run,
    lp: Loop,
    unswitched: BlockId,
    old_exiting: BlockId,
    old_ph: BlockId,
) {
    for phi in phis(run.function, unswitched) {
        let incoming: Vec<_> = arms(run.function, phi)
            .into_iter()
            .map(|(value, from)| {
                debug_assert_eq!(from, old_exiting, "found incoming block different from unique predecessor");
                (loop_entry_value(run, lp, value), old_ph)
            })
            .collect();
        run.function.set_operands(phi, from_arms(&incoming));
    }
}

/// `rewritePHINodesForExitAndUnswitchedBlocks`: `exit` stays an exit; the
/// split-off `unswitched` block gets a phi choosing between the old
/// preheader and the exit.
fn rewrite_phi_nodes_for_exit_and_unswitched_blocks(
    run: &mut Run,
    lp: Loop,
    exit: BlockId,
    unswitched: BlockId,
    old_exiting: BlockId,
    old_ph: BlockId,
    full_unswitch: bool,
) {
    let first = run.function.block(unswitched).first().expect("a terminated block");
    for phi in phis(run.function, exit) {
        let ty = run.function.instruction(phi).ty;
        let new = run.function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
        run.function.insert(new, Position::Before(first)).expect("a placed block");
        let mut incoming = arms(run.function, phi);
        let mut added: Vec<(Operand, BlockId)> = Vec::new();
        // Walk backwards over the old phi's inputs.
        for at in (0..incoming.len()).rev() {
            if incoming[at].1 != old_exiting {
                continue;
            }
            let value = incoming[at].0;
            if full_unswitch {
                // No more edge from the old exiting block to the exit block.
                incoming.remove(at);
            }
            added.push((loop_entry_value(run, lp, value), old_ph));
        }
        if full_unswitch {
            run.function.set_operands(phi, from_arms(&incoming));
        }
        // Now replace the old phi with the new one and wire the old one in as
        // an input to the new one.
        let old_value = run.function.instruction(phi).result.expect("a phi's value");
        let new_value = run.function.instruction(new).result.expect("a phi's value");
        run.function.replace_value(old_value, Operand::Value(new_value));
        added.push((Operand::Value(old_value), exit));
        run.function.set_operands(new, from_arms(&added));
    }
}

/// `hoistLoopToNewParent`: the loop, now missing an exit, may belong to a
/// loop further out; its nest is put in loop-simplify and LCSSA form again
/// (SU3). Whether it moved.
fn hoist_loop_to_new_parent(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
) -> bool {
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    // If the loop is already at the top level, we can't hoist it anywhere.
    let Some(old_parent) = lp.get_parent_loop() else { return false };
    let mut new_parent: Option<Loop> = None;
    for exit in lp.get_exit_blocks(run.function) {
        if let Some(exit_loop) = analyzed.loops.get_loop_for(exit)
            && new_parent.is_none_or(|one| one.contains_loop(Some(exit_loop)))
        {
            new_parent = Some(exit_loop);
        }
    }
    if new_parent.map(|one| one.get_header()) == Some(old_parent.get_header()) {
        return false;
    }
    crate::loopsimplify::simplified(run.function);
    let _ = crate::lcssa::closed(run.function);
    true
}

/// `buildPartialUnswitchConditionalBranch`: the invariants, frozen where they
/// might be poison, are joined with `or` (`direction`) or `and` at the end of
/// `block`, and branched on.
fn build_partial_unswitch_conditional_branch(
    run: &mut Run,
    block: BlockId,
    invariants: &[Operand],
    direction: bool,
    unswitched: BlockId,
    normal: BlockId,
    insert_freeze: bool,
    // The analyses and the instruction a freeze is judged at (`AC`, `DT`, `I`).
    judged_at: Option<(&Analyzed, InstId)>,
) {
    let void = run.context.types.void();
    let mut frozen = Vec::new();
    for &inv in invariants {
        let mut value = inv;
        if insert_freeze {
            let query = Query {
                context: &*run.context,
                globals: run.globals,
                metadata: run.metadata,
                function: &*run.function,
                layout: run.layout,
                dt: judged_at.map(|(analyzed, _)| &analyzed.tree),
                assumptions: judged_at.map(|(analyzed, _)| &analyzed.assumptions),
                dom_conditions: None,
                cond_context: None,
                cxt: judged_at.map(|(_, at)| at),
            };
            if !llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison(&query, inv, 0) {
                let ty = match inv {
                    Operand::Value(one) => run.function.value(one).ty,
                    _ => unreachable!("an invariant is a value"),
                };
                let freeze = run.function.create_instruction(Opcode::Freeze, ty, vec![inv], Flags::default(), None);
                run.function.insert(freeze, Position::End(block)).expect("a placed block");
                value = Operand::Value(run.function.instruction(freeze).result.expect("a freeze's value"));
            }
        }
        frozen.push(value);
    }
    // `IRB.CreateOr(FrozenInvariants)` / `CreateAnd`: a left fold.
    let op = if direction { BinaryOp::Or } else { BinaryOp::And };
    let ty = match frozen[0] {
        Operand::Value(one) => run.function.value(one).ty,
        _ => unreachable!("an invariant is a value"),
    };
    let mut cond = frozen[0];
    for &next in &frozen[1..] {
        let inst = run.function.create_instruction(Opcode::Binary(op), ty, vec![cond, next], Flags::default(), None);
        run.function.insert(inst, Position::End(block)).expect("a placed block");
        cond = Operand::Value(run.function.instruction(inst).result.expect("a binary's value"));
    }
    let (first, second) = if direction { (unswitched, normal) } else { (normal, unswitched) };
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

/// `unswitchTrivialBranch`: whether the branch `bi` was unswitched.
fn unswitch_trivial_branch(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
    bi: InstId,
) -> Step {
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    // The loop invariant values that we want to unswitch.
    let mut invariants: Vec<Operand> = Vec::new();
    // When true, we're fully unswitching the branch rather than just
    // unswitching some input conditions to the branch.
    let mut full_unswitch = false;

    let cond = skip_trivial_select(run, run.function.instruction(bi).operands[0]);
    if lp.is_loop_invariant(run.function, cond) {
        invariants.push(cond);
        full_unswitch = true;
    } else {
        if let Some(root) = def_of(run, cond) {
            invariants = collect_homogenous_inst_graph_loop_invariants(run, lp, root);
        }
        if invariants.is_empty() {
            return Step::default();
        }
    }
    let Operand::Block(succ0) = run.function.instruction(bi).operands[1] else { unreachable!("a branch's target") };
    let Operand::Block(succ1) = run.function.instruction(bi).operands[2] else { unreachable!("a branch's target") };
    let parent = run.function.parent(bi).expect("a placed branch");

    let loop_latch = lp.get_loop_latch(run.function);
    let ul_exit = loop_latch.and_then(|_| analyzed.loops.get_unique_latch_exit_block(run.function, lp));
    let mut latch_idx = None;
    if let (true, Some(latch), Some(_)) = (full_unswitch, loop_latch, ul_exit) {
        if succ0 == latch && lp.contains(succ1) {
            latch_idx = Some(0);
        } else if succ1 == latch && lp.contains(succ0) {
            latch_idx = Some(1);
        }
    }

    let mut modified_branch = false;
    // Redirecting the latch edge to the exit block will cause us to skip latch
    // instructions. This can only be done if the latch instructions don't have
    // side effects.
    if let (Some(index), Some(latch), Some(exit)) = (latch_idx, loop_latch, ul_exit)
        && are_loop_exit_phis_trivial(run, lp, latch, exit, false)
        && !run.function.iter_block(latch).any(|inst| has_side_effects(run, inst))
    {
        // We need to prove the loop is finite, otherwise this change will
        // convert it to a finite loop. This conservative check is good enough
        // as we are mostly interested in perfect countable loop nests that
        // perform calculations on arrays.
        let mut se = ScalarEvolution::new();
        let cx = Cx {
            context: &*run.context,
            globals: run.globals,
            metadata: run.metadata,
            function: &*run.function,
            layout: run.layout,
            loops: &analyzed.loops,
            tree: &analyzed.tree,
            assumptions: &analyzed.assumptions,
        };
        let max = se.get_backedge_taken_count(&cx, header, ExitCountKind::ConstantMaximum);
        if !se.is_could_not_compute(max) {
            run.function.predecessor_removed(&*run.context, latch, parent, false);
            run.function.set_operand(bi, 1 + index, Operand::Block(exit));
            for phi in phis(run.function, exit) {
                let mut incoming = arms(run.function, phi);
                let v = incoming
                    .iter()
                    .find(|&&(_, from)| from == latch)
                    .map(|&(value, _)| value)
                    .expect("an input from the latch");
                incoming.push((v, parent));
                run.function.set_operands(phi, from_arms(&incoming));
            }
            modified_branch = true;
        }
    }

    // Check that one of the branch's successors exits, and which one.
    let mut exit_direction = true;
    let mut loop_exit_succ_idx = 0usize;
    let Operand::Block(mut loop_exit_bb) = run.function.instruction(bi).operands[1] else { unreachable!() };
    if lp.contains(loop_exit_bb) {
        exit_direction = false;
        loop_exit_succ_idx = 1;
        let Operand::Block(second) = run.function.instruction(bi).operands[2] else { unreachable!() };
        loop_exit_bb = second;
        if lp.contains(loop_exit_bb) {
            assert!(!modified_branch, "modified the branch but didn't unswitch");
            return Step::default();
        }
    }
    let Operand::Block(continue_bb) = run.function.instruction(bi).operands[2 - loop_exit_succ_idx] else {
        unreachable!("a branch's target")
    };

    // If the exit incomings aren't loop-invariant, the unswitch is still
    // trivial when every non-invariant incoming is a header phi.
    if !modified_branch && !are_loop_exit_phis_trivial(run, lp, parent, loop_exit_bb, true) {
        return Step::default();
    }

    // When unswitching only part of the branch's condition, we need the exit
    // block to be reached directly from the partially unswitched input. This
    // can be done when the exit block is along the true edge and the branch
    // condition is a graph of `or` operations, or the exit block is along the
    // false edge and the condition is a graph of `and` operations.
    if !full_unswitch {
        let root = def_of(run, cond).expect("a partial unswitch has an instruction");
        if logical(run, root, !exit_direction).is_none() {
            assert!(!modified_branch, "modified the branch but didn't unswitch");
            return Step::default();
        }
    }

    // Split the preheader, so that we know that there is a safe place to
    // insert the conditional branch. We will change the preheader to have a
    // conditional branch on LoopCond.
    let old_ph = lp.get_loop_preheader(run.function).expect("a preheader");
    let new_ph = edges::split_edge(run.function, old_ph, header);

    // Now that we have a place to insert the conditional branch, create a place
    // to branch to: this is the exit block out of the loop that we are
    // unswitching. We need to split this if there are other loop predecessors.
    // Because the loop is in simplified form, *any* other predecessor is
    // enough.
    let unique_predecessor = |run: &Run, block: BlockId| {
        let list: Vec<BlockId> = run.function.predecessors(block).iter().copied().collect();
        (list.len() == 1).then(|| list[0])
    };
    let unswitched_bb = if full_unswitch && unique_predecessor(run, loop_exit_bb).is_some() {
        debug_assert_eq!(unique_predecessor(run, loop_exit_bb), Some(parent), "a branch's parent isn't a predecessor");
        loop_exit_bb
    } else {
        edges::split_block(run.function, loop_exit_bb)
    };

    // Actually move the invariant uses into the unswitched position. If
    // possible, we do this by moving the instructions, but when doing partial
    // unswitching we do it by building a new merge of the values in the
    // unswitched position.
    let old_term = run.function.terminator(old_ph).expect("a terminated preheader");
    let void = run.function.instruction(old_term).ty;
    run.function.set_operands(old_term, Vec::new());
    run.function.erase(old_term).expect("the jump had no users");
    if full_unswitch {
        // If fully unswitching, we can use the existing branch instruction.
        // Splice it into the old PH to gate reaching the new preheader and
        // re-point its successors.
        run.function.move_to(bi, Position::End(old_ph)).expect("a placed branch");
        run.function.set_operand(bi, 0, cond);
        // Create a new unconditional branch that will continue the loop as a
        // new terminator.
        let jump = run
            .function
            .create_instruction(Opcode::Br, void, vec![Operand::Block(continue_bb)], Flags::default(), None);
        run.function.insert(jump, Position::End(parent)).expect("a placed block");
        run.function.set_operand(bi, 1 + loop_exit_succ_idx, Operand::Block(unswitched_bb));
        run.function.set_operand(bi, 2 - loop_exit_succ_idx, Operand::Block(new_ph));
    } else {
        // Only unswitching a subset of inputs to the condition, so we will
        // need to build a new branch that merges the invariant inputs.
        build_partial_unswitch_conditional_branch(
            run,
            old_ph,
            &invariants,
            exit_direction,
            unswitched_bb,
            new_ph,
            true,
            None,
        );
    }

    // Rewrite the relevant phi nodes.
    if unswitched_bb == loop_exit_bb {
        rewrite_phi_nodes_for_unswitched_exit_block(run, lp, unswitched_bb, parent, old_ph);
    } else {
        rewrite_phi_nodes_for_exit_and_unswitched_blocks(
            run,
            lp,
            loop_exit_bb,
            unswitched_bb,
            parent,
            old_ph,
            full_unswitch,
        );
    }

    // The constant we can replace all of our invariants with inside the loop
    // body. If any of the invariants have a value other than this the loop
    // won't be entered.
    for &invariant in &invariants {
        let Operand::Value(value) = invariant else { unreachable!("an invariant is a value") };
        let ty = run.function.value(value).ty;
        let replacement = Operand::Constant(run.context.int(ty, if exit_direction { 0 } else { 1 }));
        replace_loop_invariant_uses(run, lp, invariant, replacement);
    }

    // If this was full unswitching, we may have changed the nesting
    // relationship for this loop so hoist it to its correct parent if needed.
    let rebuilt = full_unswitch && hoist_loop_to_new_parent(run, analyzed, header);
    Step { done: true, rebuilt }
}

/// `unswitchTrivialSwitch`: whether the switch `si` was unswitched.
fn unswitch_trivial_switch(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
    si: InstId,
) -> Step {
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    let loop_cond = run.function.instruction(si).operands[0];
    // If this isn't switching on an invariant condition, we can't unswitch it.
    if !lp.is_loop_invariant(run.function, loop_cond) {
        return Step::default();
    }
    let parent = run.function.parent(si).expect("a placed switch");

    // The same check must be used both for the default and the exit cases.
    let is_trivially_unswitchable_exit_block = |run: &Run, block: BlockId| {
        // Not an exit block if it is inside loop L.
        if lp.contains(block) {
            return false;
        }
        // Not trivial to unswitch if its phis aren't loop invariant.
        if !are_loop_exit_phis_trivial(run, lp, parent, block, false) {
            return false;
        }
        // We do not unswitch a block that only has an unreachable statement,
        // as it's possible this is a previously unswitched block.
        let term = run.function.block(block).last().expect("a terminated block");
        let only = run.function.iter_block(block).find(|&inst| run.function.instruction(inst).opcode != Opcode::Phi)
            == Some(term);
        run.function.instruction(term).opcode != Opcode::Unreachable || !only
    };

    let operands = run.function.instruction(si).operands.clone();
    let Operand::Block(default_dest) = operands[1] else { unreachable!("a default target") };
    // (value, successor) of each case, in order.
    let cases: Vec<(Operand, BlockId)> = operands[2..]
        .chunks(2)
        .map(|pair| match pair[1] {
            Operand::Block(to) => (pair[0], to),
            _ => unreachable!("a case target"),
        })
        .collect();
    let exit_case_indices: Vec<usize> = cases
        .iter()
        .enumerate()
        .filter(|&(_, &(_, to))| is_trivially_unswitchable_exit_block(run, to))
        .map(|(index, _)| index)
        .collect();
    let mut default_exit_bb = None;
    if is_trivially_unswitchable_exit_block(run, default_dest) {
        default_exit_bb = Some(default_dest);
    } else if exit_case_indices.is_empty() {
        return Step::default();
    }

    // Store the exit cases into a separate data structure and remove them from
    // the switch. Walking backwards, the last case goes first.
    let mut exit_cases: Vec<(Operand, BlockId)> = Vec::new();
    for &index in exit_case_indices.iter().rev() {
        exit_cases.push(cases[index]);
    }
    let remaining: Vec<(Operand, BlockId)> =
        cases.iter().enumerate().filter(|(index, _)| !exit_case_indices.contains(index)).map(|(_, &one)| one).collect();

    // Check if after this all of the remaining cases point at the same
    // successor.
    let mut common_succ_bb = None;
    if !remaining.is_empty() && remaining[1..].iter().all(|&(_, to)| to == remaining[0].1) {
        common_succ_bb = Some(remaining[0].1);
    }
    if default_exit_bb.is_none() {
        // If we're not unswitching the default, we need it to match any cases
        // to have a common successor or if we have no cases it is the common
        // successor.
        if remaining.is_empty() {
            common_succ_bb = Some(default_dest);
        } else if Some(default_dest) != common_succ_bb {
            common_succ_bb = None;
        }
    }

    let void = run.function.instruction(si).ty;
    // Where the switch still goes once the exit cases and the default (if it
    // exits) are taken out: the edges from the parent to a block that is not
    // among these are the unswitched ones.
    let mut kept_targets: Vec<BlockId> = remaining.iter().map(|&(_, to)| to).collect();
    if default_exit_bb.is_none() {
        kept_targets.push(default_dest);
    }

    // Split the preheader, so that we know that there is a safe place to
    // insert the switch.
    let old_ph = lp.get_loop_preheader(run.function).expect("a preheader");
    let new_ph = edges::split_edge(run.function, old_ph, header);
    let old_term = run.function.terminator(old_ph).expect("a terminated preheader");
    run.function.set_operands(old_term, Vec::new());
    run.function.erase(old_term).expect("the jump had no users");

    // Rewrite the IR for the unswitched basic blocks. This requires two steps.
    // First, we split any exit blocks with remaining in-loop predecessors.
    // Then we update the phis in one of two ways depending on if there was a
    // split.
    let mut unswitched_exit_bbs: Vec<BlockId> = Vec::new();
    let mut split_exit_bb_map: Vec<(BlockId, BlockId)> = Vec::new();
    let pred_empty = |run: &Run, block: BlockId| {
        !kept_targets.contains(&block) && run.function.predecessors(block).iter().all(|&one| one == parent)
    };
    if let Some(default_exit) = default_exit_bb {
        if pred_empty(run, default_exit) {
            unswitched_exit_bbs.push(default_exit);
            rewrite_phi_nodes_for_unswitched_exit_block(run, lp, default_exit, parent, old_ph);
        } else {
            let split = edges::split_block(run.function, default_exit);
            rewrite_phi_nodes_for_exit_and_unswitched_blocks(run, lp, default_exit, split, parent, old_ph, true);
            split_exit_bb_map.push((default_exit, split));
            default_exit_bb = Some(split);
        }
    }
    // Walk the exit cases in reverse so that we split in the same order as
    // the cases appeared.
    for index in (0..exit_cases.len()).rev() {
        let exit_bb = exit_cases[index].1;
        // If this case is the last edge into the exit block, we can simply
        // reuse it as it will no longer be a loop exit. No mapping necessary.
        if pred_empty(run, exit_bb) {
            // Only rewrite once.
            if !unswitched_exit_bbs.contains(&exit_bb) {
                unswitched_exit_bbs.push(exit_bb);
                rewrite_phi_nodes_for_unswitched_exit_block(run, lp, exit_bb, parent, old_ph);
            }
            continue;
        }
        // Otherwise we need to split the exit block so that we retain an exit
        // block from the loop and a target for the unswitched condition.
        let split = match split_exit_bb_map.iter().find(|&&(from, _)| from == exit_bb) {
            Some(&(_, split)) => split,
            None => {
                let split = edges::split_block(run.function, exit_bb);
                rewrite_phi_nodes_for_exit_and_unswitched_blocks(run, lp, exit_bb, split, parent, old_ph, true);
                split_exit_bb_map.push((exit_bb, split));
                split
            }
        };
        // Update the case pair to point to the split block.
        exit_cases[index].1 = split;
    }

    // Now add the unswitched switch: the exit cases in the order they were,
    // and the default to the new preheader.
    let mut new_operands = vec![loop_cond, Operand::Block(new_ph)];
    for &(value, to) in exit_cases.iter().rev() {
        new_operands.push(value);
        new_operands.push(Operand::Block(to));
    }
    // If the default was unswitched, re-point it and add explicit cases for
    // entering the loop.
    if let Some(default_exit) = default_exit_bb {
        new_operands[1] = Operand::Block(default_exit);
        // We removed all the exit cases, so we just copy the cases to the
        // unswitched switch.
        for &(value, _) in &remaining {
            new_operands.push(value);
            new_operands.push(Operand::Block(new_ph));
        }
    }
    let new_si = run.function.create_instruction(Opcode::Switch, void, new_operands, Flags::default(), None);
    run.function.insert(new_si, Position::End(old_ph)).expect("a placed block");

    // If we ended up with a common successor for every path through the switch
    // after unswitching, rewrite it to an unconditional branch to make it easy
    // to recognize. Otherwise we potentially have to recognize the default
    // case pointing at unreachable and other complexity.
    if let Some(common) = common_succ_bb {
        // We may have had multiple edges to this common successor block, so
        // remove them as predecessors. We skip the first one, either the
        // default or the actual first case.
        let mut skipped_first = default_exit_bb.is_none();
        for _ in &remaining {
            if !skipped_first {
                skipped_first = true;
                continue;
            }
            run.function.predecessor_removed(&*run.context, common, parent, true);
        }
        let jump =
            run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(common)], Flags::default(), None);
        run.function.insert(jump, Position::Before(si)).expect("a placed block");
        run.function.erase(si).expect("the switch had no users");
    } else if default_exit_bb.is_some() {
        assert!(!remaining.is_empty(), "if we had no cases we'd have a common successor");
        // Move the last case to the default successor. This is valid as if the
        // default got unswitched it cannot be reached. This has the advantage
        // of being simple and keeping the number of edges from this switch to
        // successors the same, and avoiding any phi update complexity.
        let last = remaining.last().copied().expect("a case");
        let mut operands = vec![loop_cond, Operand::Block(last.1)];
        for &(value, to) in &remaining[..remaining.len() - 1] {
            operands.push(value);
            operands.push(Operand::Block(to));
        }
        run.function.set_operands(si, operands);
    }

    // We may have changed the nesting relationship for this loop so hoist it
    // to its correct parent if needed.
    let rebuilt = hoist_loop_to_new_parent(run, analyzed, header);
    Step { done: true, rebuilt }
}

/// `unswitchAllTrivialConditions`: the trivial conditions of the loop headed by
/// `header`, one after another; whether any was unswitched.
fn unswitch_all_trivial_conditions(
    run: &mut Run,
    analyzed: &mut Analyzed,
    header: BlockId,
) -> bool {
    let mut changed = false;
    // If loop header has only one reachable successor we should keep looking
    // for trivial condition candidates in the successor as well.
    let mut current = header;
    let mut visited = IdSet::from_iter([current]);
    loop {
        // Check if there are any side-effecting instructions (e.g. stores,
        // calls, volatile loads) in the part of the loop that the code *would*
        // execute without unswitching.
        if run.function.iter_block(current).collect::<Vec<_>>().into_iter().any(|inst| has_side_effects(run, inst)) {
            return changed;
        }
        let term = run.function.terminator(current).expect("a terminated block");
        let end = run.function.instruction(term);
        let (opcode, operands) = (end.opcode.clone(), end.operands.clone());

        let step = if opcode == Opcode::Switch {
            // Don't bother trying to unswitch past a switch with a constant
            // condition.
            if matches!(operands[0], Operand::Constant(_)) {
                return changed;
            }
            let step = unswitch_trivial_switch(run, analyzed, header, term);
            if !step.done {
                // Couldn't unswitch this one so we're done.
                return changed;
            }
            step
        } else if opcode == Opcode::Br && operands.len() == 3 {
            // Don't bother trying to unswitch past a branch with a constant
            // value.
            if matches!(skip_trivial_select(run, operands[0]), Operand::Constant(_)) {
                return changed;
            }
            // Found a trivial condition candidate: non-foldable conditional
            // branch. If we fail to unswitch this, we can't do anything else
            // that is trivial.
            let step = unswitch_trivial_branch(run, analyzed, header, term);
            if !step.done {
                return changed;
            }
            step
        } else {
            // We do not understand other terminator instructions.
            return changed;
        };
        changed = true;
        if step.rebuilt {
            *analyzed = Analyzed::of(run);
        }

        // If unswitching turned the terminator into an unconditional branch
        // then we can continue; if only some of the conditions feeding a
        // branch were unswitched, it stays a conditional one.
        let term = run.function.terminator(current).expect("a terminated block");
        let end = run.function.instruction(term);
        if end.opcode != Opcode::Br || end.operands.len() != 1 {
            return changed;
        }
        let Operand::Block(next) = end.operands[0] else { unreachable!("a jump's target") };
        current = next;
        // When continuing, if we exit the loop or reach a previous visited
        // block, then we can not reach any trivial condition candidates.
        let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
        if !lp.contains(current) || !visited.insert(current) {
            return changed;
        }
    }
}

/// What `unswitchLoop` did to a loop: whether it changed, and the headers of
/// the loops it cloned, which `postUnswitch` hands to the pass manager.
#[derive(Default)]
struct Unswitched {
    changed: bool,
    siblings: Vec<BlockId>,
}

/// `unswitchBestCondition`: the cheapest non-trivial candidate, if its cost is
/// below the threshold.
fn unswitch_best_condition(
    run: &mut Run,
    analyzed: &mut Analyzed,
    header: BlockId,
) -> Option<Vec<BlockId>> {
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    // Collect all invariant conditions within this loop (as opposed to an
    // inner loop which would be handled when visiting that inner loop).
    let candidates = candidates::collect(run, analyzed, lp);
    if candidates.is_empty() {
        return None;
    }
    let mut best = candidates::best(run, analyzed, lp, &candidates)?;
    let insert_freeze;
    if run.function.instruction(best.terminator).opcode == Opcode::Select {
        // Select instructions with a poison conditional do not propagate
        // poison, but branching on poison causes UB: freeze the condition of
        // the branch the select becomes.
        let cond = run.function.instruction(best.terminator).operands[0];
        insert_freeze = nontrivial::should_insert_freeze_of_select(run, analyzed, lp, cond);
        best.terminator = nontrivial::turn_select_into_branch(run, best.terminator);
        // The branch made new blocks in the loop (SU7).
        *analyzed = Analyzed::of(run);
    } else {
        let cond = run.function.instruction(best.terminator).operands[0];
        let cond = if run.function.instruction(best.terminator).opcode == Opcode::Switch {
            cond
        } else {
            skip_trivial_select(run, cond)
        };
        insert_freeze = nontrivial::should_insert_freeze(run, analyzed, lp, cond, best.terminator);
    }
    llrm_support::debug!("simpleloopunswitch", "unswitching non-trivial terminator in b{}", header.0);
    Some(nontrivial::unswitch_nontrivial_invariants(run, analyzed, header, &best, insert_freeze).cloned_headers)
}

/// `unswitchLoop`: the trivial path, then, with `non_trivial`, the best
/// non-trivial condition.
fn unswitch_loop(
    run: &mut Run,
    analyzed: &mut Analyzed,
    header: BlockId,
    non_trivial: bool,
) -> Unswitched {
    // Must be in loop simplified form: we need a preheader and dedicated
    // exits.
    {
        let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
        if lp.get_loop_preheader(run.function).is_none()
            || !lp.has_dedicated_exits(run.function)
            || lp.get_loop_latch(run.function).is_none()
        {
            return Unswitched::default();
        }
    }
    // Try trivial unswitch first before looking at other blocks in the loop.
    if unswitch_all_trivial_conditions(run, analyzed, header) {
        return Unswitched { changed: true, siblings: Vec::new() };
    }
    // Skip non-trivial unswitching for optsize functions.
    let optsize = run
        .function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
            ),
        );
    let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    if !non_trivial || optsize || !nontrivial::is_safe(run, analyzed, lp) {
        return Unswitched::default();
    }
    match unswitch_best_condition(run, analyzed, header) {
        Some(siblings) => Unswitched { changed: true, siblings },
        None => Unswitched::default(),
    }
}

/// `SimpleLoopUnswitchPass` over a function: every loop, innermost first. The
/// loops a non-trivial unswitch clones are looked at after. Whether anything
/// changed.
pub fn unswitch_loops(
    run: &mut Run,
    non_trivial: bool,
) -> bool {
    let mut analyzed = Analyzed::of(run);
    let mut work: std::collections::VecDeque<BlockId> =
        analyzed.loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    let mut changed = false;
    while let Some(header) = work.pop_front() {
        if !run.function.is_placed(header) || !analyzed.loops.is_loop_header(header) {
            continue;
        }
        // SU1: a loop that was unswitched is looked at again.
        loop {
            let step = unswitch_loop(run, &mut analyzed, header, non_trivial);
            if !step.changed {
                break;
            }
            changed = true;
            work.extend(step.siblings);
            // The loops around it may have changed: derive them again where
            // the nest is not a top-level loop, or the pass cloned blocks.
            let top_level = analyzed.loops.get_loop_for(header).is_some_and(|one| one.get_parent_loop().is_none());
            if !top_level || non_trivial {
                analyzed = Analyzed::of(run);
            }
            if !run.function.is_placed(header) || !analyzed.loops.is_loop_header(header) {
                break;
            }
        }
    }
    changed
}

/// `SimpleLoopUnswitchPass` as a slot of the pipeline: `NonTrivial` is the
/// pin's `Level == O3`.
pub struct SimpleLoopUnswitch {
    pub non_trivial: bool,
}

impl FunctionPass for SimpleLoopUnswitch {
    fn name(&self) -> &'static str {
        "simpleloopunswitch"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("simple-loop-unswitch")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = crate::looprotate::target_tti(outer.target());
        let mut run = Run {
            context: unit.context,
            layout: unit.layout,
            globals: &outer.globals,
            metadata: unit.metadata,
            callees: outer.callees(),
            tti: &*tti,
            function: unit.function,
        };
        if unswitch_loops(&mut run, self.non_trivial) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

#[cfg(test)]
#[path = "simpleloopunswitch_tests.rs"]
mod tests;
