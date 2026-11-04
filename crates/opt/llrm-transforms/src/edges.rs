//! Edge surgery on conditional branches: a new block on one arm of a `br i1`,
//! holding the operations that belong to that edge and not to either arm's
//! block.
//!
//! # How it works
//! A conditional edge is one of the two arms of a block's `br i1`.
//! `conditional` tests that `block` ends in a two-way `br` with one arm to
//! `target`; `explicit` adds that the arm is the branch's taken (condition
//! true) operand.
//!
//! `split(function, source, target, instructions)` creates a block, places the
//! given instructions in it followed by a `br target`, redirects the `source`
//! branch's operands that name `target` to the new block, and renames `source`
//! to the new block in the incoming blocks of `target`'s phis. It refuses an
//! edge that is not a conditional one. Block ids are never reused, so
//! `create_block` is a fresh label.
//!
//! ```text
//!   source: br i1 %c, target, other        source: br i1 %c, bridge, other
//!   target: %p = phi [%v, source], ..  ==> bridge: <instructions>
//!                                                  br target
//!                                          target: %p = phi [%v, bridge], ..
//! ```
//!
//! `conditions_decided` makes a phi operand that is a branch's own condition,
//! read on the edge that branch's arm is, that arm's constant: `phi i1 [%c,
//! %latch]` in an exit is `false` (GVN's `propagateEquality` for the edge's own
//! phi uses, which `fold` runs after rotation and `gvn` before).
//!
//! `retarget` redirects every operand of a terminator that names one block to
//! another, and `phis` lists a block's leading phis.
//!
//! # Example
//! Before, `%x` is loaded at the join of `b1` and `b2`, where only `b1` has the
//! cell's value:
//!
//! ```mir
//! b2:
//!   br i1 %d, label %b3, label %b4
//!
//! b3:
//!   %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//! ; ...
//! ```
//!
//! After load PRE inserts the missing load on the `b2`-to-`b3` edge:
//!
//! ```mir
//! b2:
//!   br i1 %d, label %0, label %b4
//!
//! 0:
//!   %1 = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b3:
//!   %x1 = phi i16 [ %y, %b1 ], [ %1, %0 ]
//! ; ...
//! ```
//!
//! `b2` has another successor, so the load goes in a new block `0` on the edge,
//! not in `b2`.
//!
//! From unit test `test_missing_conditional_edge_gets_its_own_load_block` in
//! `gvn_load_tests.rs`.
//!
//! # Runs
//! Not a pass: a library. `split` is called by `rotate` and
//! `simpleloopunswitch`; `phis` and `retarget` by most of the CFG-rewriting
//! passes.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};

use crate::lcssa::{arms, from_arms};

/// `target` is the edge `block`'s conditional branch takes when its
/// condition holds: the old branch's own target, not its fall-through.
pub fn explicit(
    function: &Function,
    block: BlockId,
    target: BlockId,
) -> bool {
    conditional(function, block, target)
        && branch(function, block).is_some_and(|one| function.instruction(one).operands[1] == Operand::Block(target))
}

/// `block` ends in a two-way conditional branch, one way to `target`.
pub fn conditional(
    function: &Function,
    block: BlockId,
    target: BlockId,
) -> bool {
    let successors = function.successors(block);
    successors.len() == 2 && successors.contains(&target) && branch(function, block).is_some()
}

/// `block`'s terminator, if it is `br i1`.
fn branch(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function
        .terminator(block)
        .filter(
            |&one| {
                let instruction = function.instruction(one);
                instruction.opcode == Opcode::Br && instruction.operands.len() == 3
            },
        )
}

/// A new block on the conditional edge `source` to `target`, holding
/// `instructions` (created, placed nowhere) and a jump on.
pub fn split(
    function: &mut Function,
    source: BlockId,
    target: BlockId,
    instructions: Vec<InstId>,
) -> Result<BlockId, String> {
    if !function.layout().contains(&source) || !conditional(function, source, target) {
        return Err("edge split does not identify a conditional edge".into());
    }
    let parent = branch(function, source).expect("a conditional edge ends in a branch");
    let void = function.instruction(parent).ty;
    let bridge = function.create_block(None);
    function.insert_block(bridge, None)?;
    for inst in instructions {
        function.insert(inst, Position::End(bridge))?;
    }
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(target)], Flags::default(), None);
    function.insert(jump, Position::End(bridge))?;
    retarget(function, parent, target, bridge);
    for phi in phis(function, target) {
        let operands = function
            .instruction(phi)
            .operands
            .iter()
            .map(|&one| if one == Operand::Block(source) { Operand::Block(bridge) } else { one })
            .collect();
        function.set_operands(phi, operands);
    }
    Ok(bridge)
}

/// `BasicBlock::getSinglePredecessor`: the block when exactly one edge, of one
/// block, enters `block`.
pub fn single_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let mut edges =
        function.block_users(block).iter().filter(|one| function.instruction(one.user).opcode.is_terminator());
    let first = edges.next()?;
    edges.next().is_none().then(|| function.parent(first.user)).flatten()
}

/// `isCriticalEdge`
/// ([CFG.cpp:106](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CFG.cpp#L106)),
/// for the first edge from `from` to `to`: `from` has several edges out and
/// `to` more than one in.
pub fn is_critical_edge(
    function: &Function,
    from: BlockId,
    to: BlockId,
) -> bool {
    let Some(terminator) = function.terminator(from) else { return false };
    let out = function.instruction(terminator).operands.iter().filter(|one| matches!(one, Operand::Block(_))).count();
    out > 1 && single_predecessor(function, to).is_none()
}

/// `SplitCriticalEdge(Src, Dst)`
/// ([BasicBlockUtils.h:244](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/BasicBlockUtils.h#L244))
/// without analyses to keep: a block holding a `br to` on the first edge from
/// `from` to `to`, if that edge is critical and `to` is not a landing pad.
pub fn split_critical_edge(
    function: &mut Function,
    from: BlockId,
    to: BlockId,
) -> Option<BlockId> {
    if !is_critical_edge(function, from, to) {
        return None;
    }
    let first = function.iter_block(to).find(|&one| function.instruction(one).opcode != Opcode::Phi);
    if first.is_some_and(|one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. })) {
        return None;
    }
    let terminator = function.terminator(from)?;
    let slot = function.instruction(terminator).operands.iter().position(|one| *one == Operand::Block(to))?;
    let void = function.instruction(terminator).ty;
    let bridge = function.create_block(None);
    // Right after `from`, as `SplitKnownCriticalEdge` lays it out
    // (BreakCriticalEdges.cpp:186).
    function.insert_block(bridge, Some(from)).ok()?;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(to)], Flags::default(), None);
    function.insert(jump, Position::End(bridge)).ok()?;
    function.set_operand(terminator, slot, Operand::Block(bridge));
    // One input of each phi is the edge that moved.
    for phi in phis(function, to) {
        let at = function.instruction(phi).operands.iter().position(|one| *one == Operand::Block(from))?;
        function.set_operand(phi, at, Operand::Block(bridge));
    }
    Some(bridge)
}

/// Where `SplitEdge` lays the block it makes (BasicBlockUtils.cpp:672): a
/// critical edge's after `from`, a successor with one predecessor is split
/// before itself, else `from` is split at the bottom, after itself. `bridge`
/// is created and not placed; ask before the edge is retargeted.
pub fn insert_edge_split(
    function: &mut Function,
    bridge: BlockId,
    from: BlockId,
    to: BlockId,
) {
    if single_predecessor(function, to).is_some() && !is_critical_edge(function, from, to) {
        function.insert_block_before(bridge, to).expect("a new block");
    } else {
        function.insert_block(bridge, Some(from)).expect("a new block");
    }
}

/// `SplitEdge(from, to)`
/// ([BasicBlockUtils.cpp:672](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L672)):
/// a new block on the edges `from` has to `to`, which `to`'s phis read it for.
pub fn split_edge(
    function: &mut Function,
    from: BlockId,
    to: BlockId,
) -> BlockId {
    let term = function.terminator(from).expect("a terminated block");
    let void = function.instruction(term).ty;
    let bridge = function.create_block(None);
    insert_edge_split(function, bridge, from, to);
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(to)], Flags::default(), None);
    function.insert(jump, Position::End(bridge)).expect("a placed block");
    retarget(function, term, to, bridge);
    for phi in phis(function, to) {
        let mut incoming = arms(function, phi);
        let mut seen = false;
        incoming.retain_mut(|(_, source)| {
            if *source != from {
                return true;
            }
            *source = bridge;
            !std::mem::replace(&mut seen, true)
        });
        function.set_operands(phi, from_arms(&incoming));
    }
    bridge
}

/// `SplitBlock(old, old->begin())`
/// ([BasicBlockUtils.cpp:1079](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1079)):
/// the instructions of `old` after its phis move to a new block, `old` jumps
/// to; the successors' phis name the new one.
pub fn split_block(
    function: &mut Function,
    old: BlockId,
) -> BlockId {
    let first = function
        .iter_block(old)
        .find(|&inst| !matches!(
            function.instruction(inst).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ))
        .expect("a terminated block");
    split_block_at(function, old, first)
}

/// `Block::splitBasicBlock(at)`: the instructions of `old` from `at` on move
/// to a new block, `old` jumps to; the successors' phis name the new one.
pub fn split_block_at(
    function: &mut Function,
    old: BlockId,
    at: InstId,
) -> BlockId {
    let list: Vec<InstId> = function.iter_block(old).collect();
    let first = list.iter().position(|&inst| inst == at).expect("an instruction of the block");
    let void = function.instruction(*list.last().expect("a terminated block")).ty;
    let new = function.create_block(None);
    // Right after `old`, as `splitBasicBlock` lays it out (BasicBlock.cpp:496).
    function.insert_block(new, Some(old)).expect("a new block");
    function.splice(&list[first..], Position::End(new)).expect("a placed run");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(new)], Flags::default(), None);
    function.insert(jump, Position::End(old)).expect("a placed block");
    for succ in function.successors(new).into_iter().collect::<Vec<_>>() {
        for phi in phis(function, succ) {
            let incoming: Vec<_> = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == old { new } else { from }))
                .collect();
            function.set_operands(phi, from_arms(&incoming));
        }
    }
    new
}

/// SimplifyCFG's `addPredecessorToBlock(Succ, NewPred, ExistPred)`
/// ([L1224](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L420)):
/// each phi of `block` gets an input from `new_pred`, the one it has from
/// `existing_pred`.
pub fn predecessor_added(
    function: &mut Function,
    block: BlockId,
    new_pred: BlockId,
    existing_pred: BlockId,
) {
    for phi in phis(function, block) {
        let mut incoming = arms(function, phi);
        let value = incoming
            .iter()
            .find(|&&(_, from)| from == existing_pred)
            .expect("an input from the existing predecessor")
            .0;
        incoming.push((value, new_pred));
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// `SplitBlockPredecessors(BB, Preds)`
/// ([BasicBlockUtils.cpp:1400](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1400),
/// body `SplitBlockPredecessorsImpl`): a new block that `preds` jump to
/// instead of `bb` and that jumps to `bb`. Each phi of `bb` gets one input
/// from the new block: the value the `preds` agree on, else a phi in the new
/// block gathering theirs (`UpdatePHINodes`, L1241). No analyses to keep, no
/// landing pads; `preds` is not empty.
pub fn split_block_predecessors(
    function: &mut Function,
    bb: BlockId,
    preds: &[BlockId],
) -> BlockId {
    split_block_predecessors_of(function, bb, preds, false)
}

/// `SplitBlockPredecessors(BB, Preds, ..., PreserveLCSSA)` of a loop's exit
/// block (`HasLoopExit`): each phi of `bb` gets a phi in the new block even
/// where the `preds` agree on its value, so the value leaves the loop through
/// a phi.
pub fn split_block_predecessors_of(
    function: &mut Function,
    bb: BlockId,
    preds: &[BlockId],
    has_loop_exit: bool,
) -> BlockId {
    assert!(!preds.is_empty(), "predecessors to split off");
    // `SplitBlockPredecessorsImpl`: a landing pad is split by its own routine.
    if starts_with_landing_pad(function, bb) {
        return split_landing_pad_predecessors(function, bb, preds, "", ".split-lp", has_loop_exit)[0];
    }
    let void = function.instruction(function.terminator(bb).expect("a terminated block")).ty;
    let new = function.create_block(None);
    // Right before `bb`, as the pin lays it out (BasicBlockUtils.cpp:1331).
    function.insert_block_before(new, bb).expect("a new block");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(bb)], Flags::default(), None);
    function.insert(jump, Position::End(new)).expect("a placed block");
    for &pred in preds {
        let term = function.terminator(pred).expect("a terminated predecessor");
        retarget(function, term, bb, new);
    }
    update_phi_nodes(function, bb, new, preds, jump, has_loop_exit);
    new
}

/// `UpdatePHINodes`
/// ([BasicBlockUtils.cpp:1241](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1241)):
/// each phi of `bb` reads `new` for the inputs of `preds`, the value they agree
/// on, else a phi in `new` (before `jump`) gathering theirs.
fn update_phi_nodes(
    function: &mut Function,
    bb: BlockId,
    new: BlockId,
    preds: &[BlockId],
    jump: InstId,
    has_loop_exit: bool,
) {
    for phi in phis(function, bb) {
        let mut incoming = arms(function, phi);
        let coming: Vec<Operand> = incoming.iter().filter(|(_, from)| preds.contains(from)).map(|&(v, _)| v).collect();
        let common = coming.first().filter(|first| !has_loop_exit && coming.iter().all(|one| one == *first)).copied();
        let value = match common {
            Some(value) => value,
            None => {
                let ty = function.instruction(phi).ty;
                let gathered: Vec<_> = incoming.iter().filter(|(_, from)| preds.contains(from)).copied().collect();
                let gather = function.create_instruction(
                    Opcode::Phi,
                    ty,
                    from_arms(&gathered),
                    Flags::default(),
                    Some(&format!(
                        "{}.ph",
                        function
                            .value(function.instruction(phi).result.expect("a phi's value"))
                            .name
                            .clone()
                            .unwrap_or_default()
                    )),
                );
                function.insert(gather, Position::Before(jump)).expect("a placed phi");
                Operand::Value(function.instruction(gather).result.expect("a phi's value"))
            }
        };
        incoming.retain(|(_, from)| !preds.contains(from));
        incoming.push((value, new));
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// `SplitLandingPadPredecessors`
/// ([BasicBlockUtils.cpp:1418](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1418)):
/// `bb` starts with a landing pad, which only an unwind edge may reach, so it
/// cannot take a bridge. The edges of `preds` go to a new block `bb{suffix1}`
/// and all the others to `bb{suffix2}`, each with a copy of the landing pad
/// and a jump to `bb`; `bb` keeps its phis and reads the pads through a phi
/// where both blocks exist. The new blocks, the second only where edges
/// remain; `preds` is not empty.
pub fn split_landing_pad_predecessors(
    function: &mut Function,
    bb: BlockId,
    preds: &[BlockId],
    suffix1: &str,
    suffix2: &str,
    has_loop_exit: bool,
) -> Vec<BlockId> {
    let void = function.instruction(function.terminator(bb).expect("a terminated block")).ty;
    let mut made = Vec::new();
    // Right before `bb`, the unwind edges of `preds` first and then the rest.
    let bridge = |function: &mut Function, edges: &[BlockId]| {
        let new = function.create_block(None);
        function.insert_block_before(new, bb).expect("a new block");
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(bb)], Flags::default(), None);
        function.insert(jump, Position::End(new)).expect("a placed block");
        for &pred in edges {
            let term = function.terminator(pred).expect("a terminated predecessor");
            retarget(function, term, bb, new);
        }
        update_phi_nodes(function, bb, new, edges, jump, has_loop_exit);
        new
    };
    let first = bridge(function, preds);
    made.push(first);
    let rest: Vec<BlockId> = function.predecessors(bb).into_iter().filter(|&one| one != first).collect();
    if !rest.is_empty() {
        made.push(bridge(function, &rest));
    }
    let pad =
        function.iter_block(bb).find(|&one| function.instruction(one).opcode != Opcode::Phi).expect("the landing pad");
    let (ty, result) = (function.instruction(pad).ty, function.instruction(pad).result.expect("a pad's value"));
    let mut copies = Vec::new();
    for (&new, suffix) in made.iter().zip([suffix1, suffix2]) {
        let copy = function.clone_instruction(pad);
        let jump = function.terminator(new).expect("a jump");
        function.insert(copy, Position::Before(jump)).expect("a placed pad");
        let value = function.instruction(copy).result.expect("a pad's value");
        function.set_name(value, &format!("lpad{suffix}"));
        copies.push((Operand::Value(value), new));
    }
    let merged = if let [(_, _), (_, _)] = copies[..] {
        if function.users(result).is_empty() {
            None
        } else {
            let gather =
                function.create_instruction(Opcode::Phi, ty, from_arms(&copies), Flags::default(), Some("lpad.phi"));
            function.insert(gather, Position::Before(pad)).expect("a placed phi");
            Some(Operand::Value(function.instruction(gather).result.expect("a phi's value")))
        }
    } else {
        Some(copies[0].0)
    };
    if let Some(with) = merged {
        function.replace_all_uses_with(result, with);
    }
    function.erase(pad).expect("a pad without users");
    made
}

/// The edge's own phi uses: a phi in one arm of a `br i1` that takes the
/// branch's own condition from the branch's block reads that arm's constant:
/// the edge it is read on is the one the condition decides (GVN's
/// `propagateEquality` over the edge's own phi uses). Loop rotation and `lcssa`
/// make these (`phi i1 [%c, %latch]` in the exit, for a test after the loop),
/// and no GVN runs after them; the branch on the phi then decides itself.
/// Whether any use changed.
pub fn conditions_decided(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut found = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else { continue };
        let branch = function.instruction(last);
        let [Operand::Value(condition), Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else {
            continue;
        };
        if branch.opcode != Opcode::Br || taken == other {
            continue;
        }
        for (target, holds) in [(taken, 1), (other, 0)] {
            for one in function.users(condition) {
                let user = function.instruction(one.user);
                if user.opcode == Opcode::Phi
                    && function.parent(one.user) == Some(target)
                    && user.operands[one.index as usize + 1] == Operand::Block(block)
                {
                    found.push((*one, condition, holds));
                }
            }
        }
    }
    for &(one, condition, holds) in &found {
        let constant = context.int(function.value(condition).ty, holds);
        function.set_operand(one.user, one.index as usize, Operand::Constant(constant));
    }
    !found.is_empty()
}

/// `terminator`'s edges to `target` now go to `to`.
pub(crate) fn retarget(
    function: &mut Function,
    terminator: InstId,
    target: BlockId,
    to: BlockId,
) {
    for (at, operand) in function.instruction(terminator).operands.clone().into_iter().enumerate() {
        if operand == Operand::Block(target) {
            function.set_operand(terminator, at, Operand::Block(to));
        }
    }
}

/// Whether the first instruction of `block` after its phis is a `landingpad`:
/// a copy of it would be a second pad, and the one module handler
/// (`ehprepare`) cannot choose between two. Jump threading does not copy such
/// a block (J10 of `jumpthreading.rs`, DF3 of `dfajumpthreading.rs`).
pub(crate) fn starts_with_landing_pad(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .find(|&one| function.instruction(one).opcode != Opcode::Phi)
        .is_some_and(|one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
}

/// Whether `block` begins with two phis.
pub(crate) fn has_two_phis(
    function: &Function,
    block: BlockId,
) -> bool {
    function.iter_block(block).take(2).filter(|&one| function.instruction(one).opcode == Opcode::Phi).count() == 2
}

/// `block`'s phis, in order.
pub(crate) fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge takes over the edge, and the target's phi names it.
    #[test]
    fn test_a_split_edge_carries_the_phi_input() {
        let text = "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %r
}
";
        let mut module = llrm_mir::parse::module(text).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let (b1, b2, b3) = (function.layout()[0], function.layout()[1], function.layout()[2]);
        assert!(explicit(function, b1, b2) && !explicit(function, b1, b3));
        assert!(split(function, b2, b3, vec![]).is_err(), "not a conditional edge");
        split(function, b1, b3, vec![]).expect("splits");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        assert_eq!(
            llrm_mir::print::module(&module),
            "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %0

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %0 ], [ 2, %b2 ]
  ret i16 %r

0:
  br label %b3
}
"
        );
    }

    /// The new block of `SplitBlockPredecessors` stands right before the block
    /// it splits (BasicBlockUtils.cpp:1331). It stood at the end of the
    /// function, and JumpThreading, which visits blocks in layout order,
    /// load-PRE'd x_list's list-sum loop into two pointer phis the pin does
    /// not make.
    #[test]
    fn test_a_split_block_predecessors_block_stands_before_the_block() {
        let text = "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br label %b4

b3:
  br label %b4

b4:
  ret i16 0
}
";
        let mut module = llrm_mir::parse::module(text).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let (b2, b3, b4) = (function.layout()[1], function.layout()[2], function.layout()[3]);
        let new = split_block_predecessors(function, b4, &[b2, b3]);
        let layout = function.layout();
        assert_eq!(
            layout.iter().position(|&one| one == new),
            layout.iter().position(|&one| one == b4).map(|at| at - 1)
        );
    }

    /// The block of `SplitCriticalEdge` stands right after the block the edge
    /// leaves (BreakCriticalEdges.cpp:186), not at the end of the function.
    #[test]
    fn test_a_split_critical_edge_block_stands_after_its_source() {
        let text = "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br label %b3

b3:
  ret i16 0
}
";
        let mut module = llrm_mir::parse::module(text).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let (b1, b3) = (function.layout()[0], function.layout()[2]);
        let new = split_critical_edge(function, b1, b3).expect("the edge is critical");
        assert_eq!(function.layout().iter().position(|&one| one == new), Some(1));
    }

    /// `splitBasicBlock` puts the tail right after the block it splits
    /// (BasicBlock.cpp:496); `SplitEdge` on a successor with one predecessor
    /// puts the new block before that successor (BasicBlock.cpp:530).
    #[test]
    fn test_a_split_block_stands_after_its_block_and_a_split_edge_where_the_pin_puts_it() {
        let text = "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  %x = add i16 1, 2
  br label %b4

b3:
  br label %b4

b4:
  ret i16 0
}
";
        let mut module = llrm_mir::parse::module(text).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let (b1, b2, b3) = (function.layout()[0], function.layout()[1], function.layout()[2]);
        let at = |function: &Function, block| function.layout().iter().position(|&one| one == block);
        let add = function.iter_block(b2).next().expect("the add");
        let tail = split_block_at(function, b2, add);
        assert_eq!(at(function, tail), Some(2), "right after b2");
        let before_b2 = split_edge(function, b1, b2);
        assert_eq!(at(function, before_b2), Some(at(function, b2).unwrap() - 1), "b2 has one predecessor: before it");
        let b4 = function
            .layout()
            .iter()
            .copied()
            .find(|&one| function.block(one).name.as_deref() == Some("b4"))
            .expect("b4");
        let after_b3 = split_edge(function, b3, b4);
        assert_eq!(
            at(function, after_b3),
            Some(at(function, b3).unwrap() + 1),
            "b3 has one successor, b4 two predecessors: after b3"
        );
    }

    use llrm_mir::interpret::{Val, run};
    use llrm_mir::module::Module;

    fn parsed(text: &str) -> Module {
        llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
    }

    fn named(
        function: &Function,
        name: &str,
    ) -> BlockId {
        function
            .layout()
            .iter()
            .copied()
            .find(|&one| function.block(one).name.as_deref() == Some(name))
            .expect("a block")
    }

    fn returned(
        module: &Module,
        arguments: Vec<Val>,
    ) -> Val {
        run(module, "f", arguments, 10_000).expect("runs")
    }

    fn int(
        bits: u128,
        width: u32,
    ) -> Val {
        Val::Int { bits, width }
    }

    /// A counted loop whose latch branches back conditionally.
    const COUNTED: &str = "define i16 @f(i16 %n) {
entry:
  br label %head

head:
  %i = phi i16 [ 0, %entry ], [ %next, %head ]
  %s = phi i16 [ 1, %entry ], [ %doubled, %head ]
  %next = add i16 %i, 1
  %doubled = shl i16 %s, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %doubled
}
";

    #[test]
    fn splitting_a_back_edge_keeps_what_the_loop_computes() {
        let before = parsed(COUNTED);
        let mut module = before.clone();
        let (_, function) = module.function_mut("f").expect("@f");
        let head = named(function, "head");
        split(function, head, head, vec![]).expect("a conditional back edge");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        for n in [0, 1, 2, 5] {
            assert_eq!(returned(&module, vec![int(n, 16)]), returned(&before, vec![int(n, 16)]), "{n}");
        }
    }

    #[test]
    fn instructions_placed_on_an_edge_run_only_when_it_is_taken() {
        let text = "@g = global i16 0

define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r

spare:
  store i16 7, ptr @g
  ret i16 0
}
";
        let mut module = parsed(text);
        let (_, function) = module.function_mut("f").expect("@f");
        // An unreachable block lends the store's operands.
        let spare = function.block(named(function, "spare")).first().expect("an instruction");
        let model = function.instruction(spare).clone();
        let store = function.create_instruction(model.opcode, model.ty, model.operands, model.flags, None);
        let (b1, b3) = (named(function, "b1"), named(function, "b3"));
        split(function, b1, b3, vec![store]).expect("splits");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        assert_eq!(returned(&module, vec![int(0, 1)]), int(7, 16));
        assert_eq!(returned(&module, vec![int(1, 1)]), int(0, 16));
    }

    #[test]
    fn an_unconditional_jump_or_a_switch_edge_is_not_split() {
        let mut module = parsed(
            "define void @f(i16 %s) {
b1:
  switch i16 %s, label %b2 [ i16 1, label %b3 ]

b2:
  br label %b3

b3:
  ret void
}
",
        );
        let (_, function) = module.function_mut("f").expect("@f");
        let before = function.clone();
        let [b1, b2, b3] = ["b1", "b2", "b3"].map(|name| named(function, name));
        assert!(!conditional(function, b1, b3) && !conditional(function, b2, b3));
        assert!(split(function, b1, b3, vec![]).is_err());
        assert!(split(function, b2, b3, vec![]).is_err());
        assert_eq!(*function, before);
    }

    const JOIN: &str = "define i16 @f(i1 %c, i1 %d, i16 %a) {
entry:
  br i1 %c, label %p1, label %m

p1:
  br i1 %d, label %p2, label %m

p2:
  br label %m

m:
  %r = phi i16 [ 1, %entry ], [ %V, %p1 ], [ 2, %p2 ]
  ret i16 %r
}
";

    /// Predecessors that agree on a phi's value give the new block no phi: the
    /// phi of `m` reads the shared value from the new block.
    #[test]
    fn test_split_block_predecessors_with_one_shared_value_makes_no_phi() {
        let mut module = llrm_mir::parse::module(&JOIN.replace("%V", "1")).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let [entry, p1, m] = ["entry", "p1", "m"].map(|name| named(function, name));
        let new = split_block_predecessors(function, m, &[entry, p1]);
        assert_eq!(function.layout()[3], new, "right before m");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        let text = llrm_mir::print::module(&module);
        assert!(text.contains("%r = phi i16 [ 2, %p2 ], [ 1, %0 ]"), "{text}");
    }

    /// Predecessors that disagree get a phi in the new block, which `m`'s phi
    /// reads.
    #[test]
    fn test_split_block_predecessors_with_different_values_gathers_a_phi() {
        let mut module = llrm_mir::parse::module(&JOIN.replace("%V", "%a")).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let [entry, p1, m] = ["entry", "p1", "m"].map(|name| named(function, name));
        split_block_predecessors(function, m, &[entry, p1]);
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        let text = llrm_mir::print::module(&module);
        assert!(text.contains("phi i16 [ 1, %entry ], [ %a, %p1 ]"), "{text}");
        assert!(text.contains("%r = phi i16 [ 2, %p2 ], [ %r.ph, %0 ]"), "{text}");
    }

    /// `split_block` moves everything after the phis, and a successor's phi
    /// that named the old block names the new one; `split_edge` renames only
    /// that edge's input.
    #[test]
    fn test_split_block_and_split_edge_keep_the_successors_phis_true() {
        let mut module = llrm_mir::parse::module(&JOIN.replace("%V", "%a")).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let [p2, m] = ["p2", "m"].map(|name| named(function, name));
        let tail = split_block(function, p2);
        assert_eq!(llrm_mir::print::module(&module).matches("[ 2, %").count(), 1);
        let (_, function) = module.function_mut("f").expect("@f");
        split_edge(function, tail, m);
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        let text = llrm_mir::print::module(&module);
        assert!(text.contains("[ 2, %1 ]") && !text.contains("%p2 ]"), "{text}");
    }
}
