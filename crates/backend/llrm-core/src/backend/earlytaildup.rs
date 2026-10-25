//! Which blocks early tail duplication may copy, and into which predecessors:
//! `shouldTailDuplicate`, `isSimpleBB`, `canCompletelyDuplicateBB` and
//! `canTailDuplicate` of
//! [TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp)
//! with `PreRegAlloc` set and `LayoutMode` clear.
//!
//! # How it works
//! A block is a candidate when it cannot fall through, is no single-block loop,
//! holds at most two instructions counting its jump (one at `-Os`; twenty if it
//! ends in an indirect jump), and none of them is a call, a return or not
//! duplicable. A block with more than 16 predecessors and more than 16
//! successors is out when it or a successor has a phi. Past those, an indirect
//! jump or a block of one jump is taken as it is; any other block only if every
//! predecessor goes to it alone, by a jump with no condition
//! (`canCompletelyDuplicateBB`).
//!
//! ```text
//!   b1: ... jmp b3        b1: ... <b3's instructions> jmp b4      (b3 has two
//!   b2: ... jmp b3   ==>  b2: ... <b3's instructions> jmp b4       instructions
//!   b3: x; jmp b4         b3 is gone when nothing else reaches it)  or fewer)
//! ```
//!
//! DEVIATE ET1 (owner w-1, exit: step 2, `tailDuplicate`): only the decisions
//! are translated here; nothing calls them yet.
//!
//! DEVIATE ET2 (no number): a block's instructions are not duplicable here if
//! they carry a group, a barrier or data (the placement's version adds a call's
//! record and a symbol, which only exist after allocation); the pin's
//! `isNotDuplicable` and `isConvergent` have no counterpart. LIR has no
//! `INLINEASM_BR`, EH pads or subregister phi operands, so those tests are not
//! made.
//!
//! # Example
//! The unit tests: a join of two arms with one `mov` is a candidate;
//! the same block with a call, or with two, is not.
//!
//! # Used by
//! Step 2 of `docs/working/early-taildup-plan.md`.

use crate::model::ir::Operation;
use crate::model::lir::{Block, LirBody};

/// `TailDuplicateSize`.
const SIZE: usize = 2;
/// `TailDupIndirectBranchSize`.
const INDIRECT_SIZE: usize = 20;
/// `TailDupPredSize` and `TailDupSuccSize`.
const EDGES: usize = 16;

/// The block's instructions that are neither meta nor a no-op.
fn real(block: &Block) -> Vec<&crate::model::lir::Insn> {
    block
        .insns
        .iter()
        .filter(|one| {
            !one.is_meta()
                && !one
                    .what
                    .as_ref()
                    .is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"))
        })
        .map(|one| &**one)
        .collect()
}

fn operation(one: &crate::model::lir::Insn) -> Option<Operation> {
    one.what.as_ref().map(|what| what.op)
}

fn indirect(one: &crate::model::lir::Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch) && what.indirect)
}

/// `canFallThrough`: the block's last instruction is not a jump with no
/// condition or a return.
fn can_fall_through(block: &Block) -> bool {
    match real(block).last() {
        Some(last) => !matches!(operation(last), Some(Operation::Return | Operation::Jump)),
        None => true,
    }
}

/// Distinct successors of `block`.
fn successors(block: &Block) -> Vec<i64> {
    crate::model::lir::distinct(&block.succ)
}

/// The blocks of `body` that go to `at`.
fn predecessors(
    body: &LirBody,
    at: i64,
) -> Vec<&Block> {
    body.blocks.iter().filter(|block| block.succ.contains(&at)).collect()
}

/// `isSimpleBB`: one successor, a predecessor, and nothing but a jump.
pub fn is_simple(
    body: &LirBody,
    tail: &Block,
) -> bool {
    if successors(tail).len() != 1 || predecessors(body, tail.at).is_empty() {
        return false;
    }
    match real(tail).first() {
        None => true,
        Some(first) => operation(first) == Some(Operation::Jump),
    }
}

/// `canTailDuplicate`: `pred` has `tail` as its only successor and reaches it
/// by a jump with no condition.
pub fn can_tail_duplicate(pred: &Block) -> bool {
    successors(pred).len() <= 1 && !real(pred).last().is_some_and(|one| indirect(one))
}

/// `canCompletelyDuplicateBB`: every predecessor of `tail` can take a copy.
pub fn can_completely_duplicate(
    body: &LirBody,
    tail: &Block,
) -> bool {
    predecessors(body, tail.at).iter().all(|pred| can_tail_duplicate(pred))
}

/// `shouldTailDuplicate`, before register allocation; `size` is `-Os`.
pub fn should_tail_duplicate(
    body: &LirBody,
    tail: &Block,
    simple: bool,
    size: bool,
) -> bool {
    if can_fall_through(tail) || tail.succ.contains(&tail.at) {
        return false;
    }
    let mut most = if size { 1 } else { SIZE };
    let code = real(tail);
    let indirect_end = code.last().is_some_and(|last| indirect(last));
    if indirect_end {
        most = INDIRECT_SIZE;
    }
    let mut count = 0;
    for one in &code {
        let op = operation(one);
        if one.group.is_some() || matches!(op, Some(Operation::Barrier | Operation::Data)) {
            return false;
        }
        if matches!(op, Some(Operation::Return | Operation::Call)) {
            return false;
        }
        count += 1;
        if count > most {
            return false;
        }
    }
    let after = successors(tail);
    if predecessors(body, tail.at).len() > EDGES && after.len() > EDGES {
        let phis = !tail.phis.is_empty()
            || after.iter().any(|to| body.blocks.iter().any(|block| block.at == *to && !block.phis.is_empty()));
        if phis {
            return false;
        }
    }
    if indirect_end || simple {
        return true;
    }
    can_completely_duplicate(body, tail)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Imm, Loc, Semantics};
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    fn work(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(crate::model::ir::Held { value, width: 4 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
    }

    fn of(
        name: &str,
        op: Operation,
        target: Option<i64>,
        indirect: bool,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), target, indirect, ..Semantics::new(op) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn jump(to: i64) -> Arc<Insn> {
        of("jmp", Operation::Jump, Some(to), false)
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new("t", 0, blocks.into_iter().map(Block::from).collect(), IndexMap::default(), IndexMap::default())
    }

    /// Two arms jump to a join of the given instructions, which jumps on to 9.
    fn join(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut tail = insns;
        tail.push(jump(9));
        body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, tail, &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ])
    }

    fn at(
        body: &LirBody,
        block: i64,
    ) -> &Block {
        body.blocks.iter().find(|one| one.at == block).unwrap()
    }

    fn should(
        body: &LirBody,
        block: i64,
        size: bool,
    ) -> bool {
        let tail = at(body, block);
        should_tail_duplicate(body, tail, is_simple(body, tail), size)
    }

    /// A join of two arms is a candidate with one instruction and its jump
    /// (two, as the pin counts); one more is not.
    #[test]
    fn test_a_join_of_an_instruction_and_a_jump_is_copied_and_one_more_is_not() {
        assert!(should(&join(vec![work(3)]), 3, false));
        assert!(!should(&join(vec![work(3), work(4)]), 3, false));
    }

    /// At `-Os` the jump alone is the most: the copy saves one jump against it.
    #[test]
    fn test_at_os_a_jump_alone_is_the_most() {
        assert!(should(&join(vec![]), 3, true));
        assert!(!should(&join(vec![work(3)]), 3, true));
    }

    /// A call is never copied before allocation: it is a barrier to it, and a
    /// copy of one adds spills. (A return too: it expands after.)
    #[test]
    fn test_a_call_is_not_copied_before_allocation() {
        let call = of("call", Operation::Call, Some(7), false);
        assert!(!should(&join(vec![call]), 3, false));
        assert!(should(&join(vec![work(3)]), 3, false), "the same block of a move is");
    }

    /// A block of one jump (`isSimpleBB`) is copied whatever its predecessors
    /// do; any other waits for `canCompletelyDuplicateBB`.
    #[test]
    fn test_a_predecessor_with_a_condition_stops_a_block_that_is_not_simple() {
        let shape = |pred_exits: &[i64]| {
            body(vec![
                block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
                block(1, vec![work(1), jump(3)], pred_exits),
                block(2, vec![work(2), jump(3)], &[3]),
                block(3, vec![work(3), jump(9)], &[9]),
                block(9, vec![of("ret", Operation::Return, None, false)], &[]),
            ])
        };
        assert!(should(&shape(&[3]), 3, false));
        assert!(!should(&shape(&[3, 9]), 3, false), "predecessor 1 has two successors");
        let simple = body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(3)], &[3, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ]);
        assert!(should(&simple, 3, false), "a block of one jump goes into a predecessor that branches");
    }

    /// A block that falls out of its end, or loops on itself, is left.
    #[test]
    fn test_a_self_loop_and_a_fall_through_are_not_copied() {
        let looped = body(vec![
            block(0, vec![jump(3)], &[3]),
            block(3, vec![work(3), of("jne", Operation::Branch, Some(3), false), jump(9)], &[3, 9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ]);
        assert!(!should(&looped, 3, false));
        let falls = body(vec![block(0, vec![jump(3)], &[3]), block(3, vec![work(3)], &[9]), block(9, vec![], &[])]);
        assert!(!should(&falls, 3, false));
    }

    /// An indirect jump is copied up to twenty instructions, which undoes the
    /// merging of dispatch blocks (an interpreter's switch).
    #[test]
    fn test_a_block_ending_in_an_indirect_jump_may_hold_twenty_instructions() {
        let dispatch = |count: usize| {
            let mut insns: Vec<Arc<Insn>> = (0..count as u32).map(work).collect();
            insns.push(of("jmp", Operation::Jump, None, true));
            body(vec![
                block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
                block(1, vec![work(100), jump(3)], &[3]),
                block(2, vec![work(101), jump(3)], &[3]),
                block(3, insns, &[1, 2]),
            ])
        };
        assert!(should(&dispatch(19), 3, false));
        assert!(!should(&dispatch(21), 3, false));
    }
}
