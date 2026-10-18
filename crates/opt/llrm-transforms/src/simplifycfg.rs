//! A terminator on a constant becomes a jump, the blocks nothing reaches go,
//! and what that leaves is merged: LLVM's `SimplifyCFGPass` and the
//! `ConstantFoldTerminator`, `removeUnreachableBlocks` and
//! `BasicBlock::removePredecessor` it is made of.
//!
//! # How it works
//! `simplified` is `simplifyFunctionCFGImpl`:
//!
//! 1. `unreachable_removed` walks the body from the entry. Each block it
//!    reaches has its terminator folded first (`terminator_folded`), so only
//!    the successor that still runs is followed; the blocks it did not reach
//!    are deleted, with their edges out (`removePredecessor`) and every use of
//!    what they defined replaced by poison.
//! 2. `iteratively_simplified` visits the blocks in layout order until a visit
//!    changes none (`iterativelySimplifyCFG`). A visit (`block_simplified`,
//!    LLVM's `simplifyOnce`) deletes a block with no predecessor (or only
//!    itself), folds its terminator, removes the duplicate phis of the block
//!    and merges it into its one predecessor.
//! 3. When step 2 changed anything, unreachable blocks are removed again and,
//!    if that removed any, step 2 and the removal alternate until neither does.
//!
//! `terminator_folded` replaces
//!
//! - `br i1 %c, X, X` by `br X`, taking one phi input of `X` away;
//! - `br i1 true/false, A, B` by a jump to the arm that runs;
//! - a `switch` on a constant by a jump to the case it takes (or the default),
//!   a `switch` whose cases all go to one block by a jump, and a case that goes
//!   where the default goes is dropped; a switch left with one case is a `br`
//!   on an `icmp eq`.
//!
//! The condition that nothing reads afterwards is deleted with what only it
//! read. The new `br` carries `!llvm.loop`.
//!
//! ```text
//!   b0: br i1 true, b1, b2         b0: br b1
//!   b1: ...                  ==>   b1: ...        (b2, and its phi inputs
//!   b2: %p = phi [%x, b0], ..                      from b0, are gone)
//! ```
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   br i1 true, label %b1, label %b2
//!
//! b1:
//!   ret i16 %x
//!
//! b2:
//!   ret i16 0
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   ret i16 %x
//! }
//! ```
//! `b2` is no longer reached, so it goes; `b1` has one predecessor, so it is
//! merged into `b0`.
//!
//! From unit test `test_a_branch_on_true_is_a_jump_and_the_dead_arm_goes` in
//! `simplifycfg_tests.rs`.
//!
//! # LLVM structure
//! [`SimplifyCFGPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272)
//! with the options of the early function pipeline slot
//! ([`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1178):
//! the defaults). `MIRROR` unless a line below says `DEVIATE`.
//!
//! | LLVM | here | |
//! |---|---|---|
//! | [`simplifyFunctionCFGImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272) | `simplified` | MIRROR |
//! | [`iterativelySimplifyCFG`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229) | `iteratively_simplified` | MIRROR; D1 |
//! | [`SimplifyCFGOpt::simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303) | `block_simplified` | partly: the steps up to the merge; the rest is not ported yet (D2) |
//! | [`removeUnreachableBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2912) and `markAliveBlocks` | `unreachable_removed` | the terminator folding and the deletion; D3 |
//! | [`ConstantFoldTerminator`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L133) | `terminator_folded` | MIRROR; D4 |
//! | [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446) | `Function::predecessor_removed` (`edit.rs`) | MIRROR (one phi input an edge) |
//! | [`DeleteDeadBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L94) | `dead_blocks_deleted` | MIRROR |
//! | [`EliminateDuplicatePHINodes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1527) | `transform::duplicate_phis_in` | MIRROR |
//! | [`MergeBlockIntoPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237) | `cfg::merged_into_predecessor` | MIRROR |
//!
//! - D1. LLVM's `LoopHeaders` (the targets of back edges, found once) are not
//!   computed: nothing ported so far reads them. They come with the first step
//!   that does (`simplifyUncondBranch`, `NeedCanonicalLoop`).
//! - D2. Not ported yet, each a later change of this series:
//!   `tailMergeBlocksWithSimilarFunctionTerminators`,
//!   `removeUndefIntroducingPredecessor`, `sinkCommonCodeFromPredecessors` (off
//!   at this slot), `foldTwoEntryPHINode` (`phiopt` meanwhile), and the
//!   terminator simplifications `simplifyUncondBranch`, `simplifyCondBranch`,
//!   `simplifySwitch`, `simplifyUnreachable`.
//! - D3. `markAliveBlocks` also turns what follows a call of a `noreturn`
//!   function, `llvm.assume(false)` and a store to `undef` into `unreachable`
//!   (`FoldInstsToUnreachable`): not ported yet. It reads the target's
//!   `NullPointerIsDefined`, which the MIR has no flag for. `invoke`,
//!   `catchswitch` and `indirectbr` do not exist in the MIR.
//! - D4. No branch weights: the MIR has none, so the case weights
//!   `ConstantFoldTerminator` merges and the `!prof` it sets are not kept.
//!   `!make.implicit` is not a MIR kind. There is no `DomTreeUpdater`; as at
//!   LLVM's default (`simplifycfg-require-and-preserve-domtree` off, where the
//!   pass is given no `DominatorTree`) the analyses are invalidated when
//!   anything changed: that is a MIRROR.
//! - D5. Temporary, driven by rows. After a terminator was folded,
//!   `transform::_trivial_phis` runs over the whole body, as `decide` did after
//!   the branches it decided. SimplifyCFG has no such construct: its phi
//!   cleanup is
//!   [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446)
//!   (`hasConstantValue`, L468), of the phis of the successor whose edge went
//!   (here `Function::predecessor_removed`). What `_trivial_phis` does is
//!   InstSimplify's
//!   [`simplifyPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5620),
//!   which InstCombine and EarlyCSE run over the whole function. Whole body,
//!   not the touched successors, because the phis are not there: gvn leaves
//!   `phi [%x, entry], [%self, latch]` in loop headers (premise:
//!   `gvn_tests::test_forwarding_a_loops_cell_to_its_phi_leaves_a_phi_of_itself`),
//!   which `decide` removed whenever it decided something. With the touched
//!   successors only, `bintree` -O3 was 7 instructions longer (twice; LSR
//!   declines the build loop with the phis in), seen failing in
//!   `simplifycfg_tests::test_a_fold_takes_a_phi_of_itself_and_one_value_out_of_the_body`.
//!   It runs once in a pass run that folded, not once per fold; compile
//!   instructions on the largest programs, base to here: `jpeg_single` (C)
//!   -2.0%, `jpeg_fixed` (Nib) -0.3%, `qcport/mdl_ai` -0.4%, `stft_single` (C)
//!   +0.08%, `jpeg_single` (BASIC, flat) +0.06%. Exit: gvn leaves no phi of
//!   itself (#2186) and `hoist` no longer lifts into an unguarded entry
//!   (#2194; a rule in `algebraic` that removed the phis made `x_hanoi2` -O2
//!   22.8% slower through it); then this goes.
//!
//! # Runs
//! Once per round of the function fixed point, before `decide`: the branches
//! on a literal that `fold` just made are jumps before `decide` asks for the
//! facts of the body. Nothing runs at -O0.
//!
//! # Measured
//! The C corpus (`identical.py`: 395 sources, -O1/-O2/-Os/-Omax, 1,580
//! compiles): `decide` decides 646 branches instead of 3,002; the ones on a
//! literal or a constant `switch` go from 2,464 to 153 (the rest are made by
//! `jumpthread`'s own cleanup, after this pass), and it runs 32,640 times
//! instead of 33,932. Of the 1,583 objects 40 differ in bytes, 36 only in label
//! numbers; of the rest, `bintree` -O3 is 7 instructions longer (twice) and
//! `switch_empty_cases` 2 shorter (twice). The 140 bench rows at -O2 and -Os of
//! the 14 programs that differ have every counter unchanged.
//!
//! # Keeps
//! Blocks and edges change: every analysis is invalidated when anything
//! changed.
//!
//! # See also
//! [`crate::cfg`], [`crate::decide`].

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::{cfg, dead, transform};

/// LLVM's `SimplifyCFGPass` at the options of the early function pipeline
/// slot (`SimplifyCFGOptions()`: `PassBuilderPipelines.cpp`).
pub struct SimplifyCfg;

impl FunctionPass for SimplifyCfg {
    fn name(&self) -> &'static str {
        "simplifycfg"
    }

    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let callees = analyses.outer().callees();
        if simplified(unit.context, callees, unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `function` simplified: LLVM's `simplifyFunctionCFGImpl`. Whether anything
/// changed.
pub fn simplified(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let mut folded = false;
    let ever = unreachable_removed(context, callees, function, &mut folded)
        | iteratively_simplified(context, callees, function, &mut folded);
    // `iterativelySimplifyCFG` can make a loop dead; the removal and the
    // visits then alternate until neither does anything.
    if !ever {
        return false;
    }
    if unreachable_removed(context, callees, function, &mut folded) {
        while iteratively_simplified(context, callees, function, &mut folded)
            | unreachable_removed(context, callees, function, &mut folded)
        {}
    }
    if folded {
        // D5.
        transform::_trivial_phis(function).expect("a phi nothing reads is erased");
    }
    true
}

/// Every block visited, in layout order, until a round changes none:
/// `iterativelySimplifyCFG`.
fn iteratively_simplified(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    folded: &mut bool,
) -> bool {
    let (mut changed, mut local) = (false, true);
    while local {
        local = false;
        for block in function.layout().to_vec() {
            if function.is_placed(block) && block_simplified(context, callees, function, block, folded) {
                local = true;
            }
        }
        changed |= local;
    }
    changed
}

/// One visit of `block`: LLVM's `SimplifyCFGOpt::simplifyOnce`, as far as it
/// is ported (D2).
fn block_simplified(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    block: BlockId,
    folded: &mut bool,
) -> bool {
    // No predecessor but not the entry, or only itself: unreachable.
    let predecessors = function.predecessors(block);
    if (predecessors.is_empty() && function.entry() != Some(block)) || predecessors[..] == [block] {
        dead_blocks_deleted(context, function, &[block]);
        return true;
    }
    let mut changed = terminator_folded(context, callees, function, block, true);
    *folded |= changed;
    changed |= transform::duplicate_phis_in(function, block).expect("a phi nothing reads is erased");
    changed | cfg::merged_into_predecessor(function, block)
}

/// The blocks the entry does not reach, deleted, after each block it reaches
/// has had its terminator folded (so a successor that no longer runs is not
/// followed): `removeUnreachableBlocks` over `markAliveBlocks`.
pub fn unreachable_removed(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    folded: &mut bool,
) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut reached = llrm_support::hash::HashSet::default();
    reached.insert(entry);
    let (mut pending, mut changed) = (vec![entry], false);
    while let Some(block) = pending.pop() {
        let one = terminator_folded(context, callees, function, block, true);
        *folded |= one;
        changed |= one;
        for successor in function.successors(block) {
            if reached.insert(successor) {
                pending.push(successor);
            }
        }
    }
    let dead: Vec<BlockId> = function.layout().iter().copied().filter(|block| !reached.contains(block)).collect();
    if dead.is_empty() {
        return changed;
    }
    dead_blocks_deleted(context, function, &dead);
    true
}

/// `blocks` deleted together: each one's edges out told (`removePredecessor`,
/// keeping a one-input phi, since the block it names may go as well), what it
/// defined replaced by poison, its instructions and then the blocks erased.
/// LLVM's `DeleteDeadBlocks`.
fn dead_blocks_deleted(
    context: &mut Context,
    function: &mut Function,
    blocks: &[BlockId],
) {
    for &block in blocks {
        let operands = function.terminator(block).map(|last| function.instruction(last).operands.clone());
        for successor in edges_out_of(&operands.unwrap_or_default()) {
            function.predecessor_removed(context, successor, block, true);
        }
        for inst in function.block(block).instructions().to_vec().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    for &block in blocks {
        function.erase_block(block).expect("an emptied block nothing names");
    }
}

/// The integer `operand` is, if it is a constant one.
fn integer(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// Whether `block` holds nothing but an `unreachable`: LLVM's
/// `SwitchInst::defaultDestUnreachable`.
fn only_unreachable(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .block(block)
        .instructions()
        .iter()
        .find(|&&inst| function.instruction(inst).opcode != Opcode::Phi)
        .is_some_and(|&inst| function.instruction(inst).opcode == Opcode::Unreachable)
}

/// `last`, a terminator, replaced by `opcode` over `operands`; the new one
/// carries `last`'s `!llvm.loop`. Its id.
fn terminator_replaced(
    function: &mut Function,
    last: InstId,
    opcode: Opcode,
    operands: Vec<Operand>,
) -> InstId {
    let void = function.instruction(last).ty;
    let new = function.create_instruction(opcode, void, operands, Flags::default(), None);
    function.insert(new, Position::Before(last)).expect("a placed terminator");
    for (kind, node) in function.instruction(last).metadata.clone() {
        if kind == "llvm.loop" {
            function.annotate(new, &kind, node);
        }
    }
    new
}

/// `block`'s terminator folded if it is on a constant, or sends both arms the
/// same way: LLVM's `ConstantFoldTerminator`. `delete_dead_conditions`: the
/// condition nothing reads afterwards goes, with what only it read.
pub fn terminator_folded(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    block: BlockId,
    delete_dead_conditions: bool,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let (opcode, operands) = (function.instruction(last).opcode.clone(), function.instruction(last).operands.clone());
    match (opcode, &operands[..]) {
        (Opcode::Br, &[condition, Operand::Block(first), Operand::Block(second)]) => {
            let destination = if first == second {
                // `br i1 %c, X, X`: one of the two edges goes.
                function.predecessor_removed(context, first, block, false);
                first
            } else if let Some(value) = integer(context, condition) {
                let (kept, gone) = if value != 0 { (first, second) } else { (second, first) };
                function.predecessor_removed(context, gone, block, false);
                kept
            } else {
                return false;
            };
            terminator_replaced(function, last, Opcode::Br, vec![Operand::Block(destination)]);
            function.erase(last).expect("a terminator defines nothing");
            condition_deleted(context, callees, function, condition, delete_dead_conditions);
            true
        }
        (Opcode::Switch, &[condition, Operand::Block(default), ..]) => {
            switch_folded(context, callees, function, block, last, condition, default, delete_dead_conditions)
        }
        _ => false,
    }
}

/// `condition`, nothing reading it any more, deleted with what only it read.
fn condition_deleted(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    condition: Operand,
    wanted: bool,
) {
    if let (true, Operand::Value(value)) = (wanted, condition) {
        dead::dead_from(context, callees, function, [value]);
    }
}

/// The `switch` `last` of `block` folded: `ConstantFoldTerminator`'s switch
/// case.
#[allow(clippy::too_many_arguments)]
fn switch_folded(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    block: BlockId,
    last: InstId,
    condition: Operand,
    default: BlockId,
    delete_dead_conditions: bool,
) -> bool {
    let mut cases: Vec<(Operand, BlockId)> = function.instruction(last).operands[2..]
        .chunks(2)
        .map(|pair| match pair {
            [case, Operand::Block(target)] => (*case, *target),
            _ => unreachable!("a switch case is a value and a block"),
        })
        .collect();
    let value = integer(context, condition);
    // A default nothing runs is not a way out when a case is.
    let mut only = Some(match cases.first() {
        Some(&(_, first)) if only_unreachable(function, default) => first,
        _ => default,
    });
    let mut changed = false;
    let mut at = 0;
    while at < cases.len() {
        let (case, target) = cases[at];
        if value.is_some() && integer(context, case) == value {
            only = Some(target);
            break;
        }
        // A case that goes where the default goes is no explicit compare.
        if target == default {
            function.predecessor_removed(context, default, block, false);
            cases.remove(at);
            changed = true;
            continue;
        }
        if only != Some(target) {
            only = None;
        }
        at += 1;
    }
    if value.is_some() && only.is_none() {
        only = Some(default);
    }
    let rebuilt = |cases: &[(Operand, BlockId)]| {
        let mut operands = vec![condition, Operand::Block(default)];
        operands.extend(cases.iter().flat_map(|&(case, target)| [case, Operand::Block(target)]));
        operands
    };
    if let Some(destination) = only {
        // The first edge to `destination` stays the jump; every other edge of
        // the switch goes.
        let mut kept = false;
        for successor in edges_out_of(&rebuilt(&cases)) {
            if successor == destination && !kept {
                kept = true;
            } else {
                function.predecessor_removed(context, successor, block, false);
            }
        }
        terminator_replaced(function, last, Opcode::Br, vec![Operand::Block(destination)]);
        function.erase(last).expect("a terminator defines nothing");
        condition_deleted(context, callees, function, condition, delete_dead_conditions);
        return true;
    }
    if let [(case, target)] = cases[..] {
        // One case left: a `br` on an equality.
        let i1 = context.types.int(1);
        let compare = function.create_instruction(
            Opcode::ICmp(IntPredicate::Eq),
            i1,
            vec![condition, case],
            Flags::default(),
            Some("cond"),
        );
        function.insert(compare, Position::Before(last)).expect("a placed terminator");
        let result = function.instruction(compare).result.expect("a compare's value");
        terminator_replaced(
            function,
            last,
            Opcode::Br,
            vec![Operand::Value(result), Operand::Block(target), Operand::Block(default)],
        );
        function.erase(last).expect("a terminator defines nothing");
        return true;
    }
    if changed {
        function.set_operands(last, rebuilt(&cases));
    }
    changed
}

/// The blocks a terminator's `operands` name, once for each edge.
fn edges_out_of(operands: &[Operand]) -> Vec<BlockId> {
    operands
        .iter()
        .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
        .collect()
}

#[cfg(test)]
#[path = "simplifycfg_tests.rs"]
mod tests;
