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
//! `tail_duplicate` is `tailDuplicate` for a tail with no phi whose values
//! nothing outside it reads: each predecessor that goes to it alone loses its
//! jump to it and takes a copy of its instructions (values the tail defines are
//! new in each copy), the successors' phis read from the predecessor what they
//! read from the tail, the tail's edge odds move to the predecessor, and the
//! tail goes when nothing reaches it.
//!
//! DEVIATE ET1 (owner w-1, exit: step 3): a tail with a phi, or whose values
//! are read outside it, is left (`processPHI`, `updateSuccessorsPHIs`, the SSA
//! update). Nothing calls `tail_duplicate` yet.
//!
//! DEVIATE ET3 (no number): LIR blocks end in explicit jumps, so the pin's
//! predecessor that falls through into the tail is the one that jumps to it
//! from the block just before it; it is left for last and takes the copy only
//! when it is the tail's one predecessor left (the pin's merge into the
//! previous block), which has the same result.
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

use std::collections::HashMap;
use std::sync::Arc;

use crate::model::ir::Operation;
use crate::model::lir::{Block, Insn, LirBlock, LirBody};

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

/// The values `tail` defines are read by an instruction or a phi of another
/// block.
fn read_outside(
    body: &LirBody,
    tail: &Block,
) -> bool {
    let defined: std::collections::HashSet<u32> =
        tail.insns.iter().flat_map(|one| one.defines.iter().copied()).collect();
    !defined.is_empty()
        && body.blocks.iter().filter(|block| block.at != tail.at).any(|block| {
            block.insns.iter().any(|one| one.uses.iter().chain(&one.undefs).any(|value| defined.contains(value)))
                || block.phis.iter().any(|phi| phi.incoming.iter().any(|(_, value)| defined.contains(value)))
        })
}

/// `duplicateSimpleBB`: a block of one jump is no code to copy: every
/// predecessor that can go to its successor directly does, whatever it ends in.
/// A predecessor that already goes to the successor, which has phis, is left
/// (`bothUsedInPHI`).
fn duplicate_simple(
    body: &LirBody,
    at: usize,
) -> Option<LirBody> {
    let tail = &body.blocks[at];
    let [to] = successors(tail)[..] else { return None };
    let target = body.blocks.iter().find(|block| block.at == to)?;
    let preds: Vec<usize> = (0..body.blocks.len())
        .filter(|one| *one != at && body.blocks[*one].succ.contains(&tail.at) && !indirect_end(&body.blocks[*one]))
        .filter(|one| target.phis.is_empty() || !body.blocks[*one].succ.contains(&to))
        .collect();
    if preds.is_empty() {
        return None;
    }
    let remains = body
        .blocks
        .iter()
        .enumerate()
        .any(|(one, block)| one != at && block.succ.contains(&tail.at) && !preds.contains(&one))
        || tail.at == body.entry;
    let mut blocks: Vec<Block> = Vec::new();
    for (place, old) in body.blocks.iter().enumerate() {
        if place == at && !remains {
            continue;
        }
        if preds.contains(&place) {
            let mut new = LirBlock::clone(old);
            new.insns = old
                .insns
                .iter()
                .map(|one| match one.what.as_ref() {
                    Some(what)
                        if matches!(what.op, Operation::Jump | Operation::Branch) && what.target == Some(tail.at) =>
                    {
                        let mut made = (**one).clone();
                        made.what = Some(crate::model::ir::Semantics { target: Some(to), ..what.clone() });
                        Arc::new(made)
                    }
                    _ => Arc::clone(one),
                })
                .collect::<Vec<_>>()
                .into();
            let mut succ: Vec<i64> = Vec::new();
            for next in old.succ.iter().map(|next| if *next == tail.at { to } else { *next }) {
                if !succ.contains(&next) {
                    succ.push(next);
                }
            }
            new.succ = succ.into();
            blocks.push(new.into());
        } else if old.at == to {
            let mut new = LirBlock::clone(old);
            new.phis = old
                .phis
                .iter()
                .map(|phi| {
                    let mut phi = phi.clone();
                    let from_tail: Vec<u32> =
                        phi.incoming.iter().filter(|(pred, _)| *pred == tail.at).map(|(_, value)| *value).collect();
                    for &pred in &preds {
                        for value in &from_tail {
                            phi.incoming.push((body.blocks[pred].at, *value));
                        }
                    }
                    if !remains {
                        phi.incoming.retain(|(pred, _)| *pred != tail.at);
                    }
                    phi
                })
                .collect::<Vec<_>>()
                .into();
            blocks.push(new.into());
        } else {
            blocks.push(old.clone());
        }
    }
    let mut made = body.with_blocks(blocks);
    for &pred in &preds {
        let from = body.blocks[pred].at;
        if let Some(odds) = made.odds.taken.shift_remove(&(from, tail.at)) {
            made.odds.taken.insert((from, to), odds);
        }
    }
    if !remains {
        made.odds.taken.retain(|(from, _), _| *from != tail.at);
    }
    Some(made)
}

/// The block ends in an indirect jump.
fn indirect_end(block: &Block) -> bool {
    real(block).last().is_some_and(|last| indirect(last))
}

/// `body` with the block at `tail` copied into its predecessors, or None where
/// the pin would not, or where this step does not reach (a phi, a value read
/// outside). `size` is `-Os`.
pub fn tail_duplicate(
    body: &LirBody,
    tail: i64,
    size: bool,
) -> Option<LirBody> {
    let at = body.blocks.iter().position(|block| block.at == tail)?;
    let block = &body.blocks[at];
    if !block.phis.is_empty() || read_outside(body, block) {
        return None;
    }
    let simple = is_simple(body, block);
    if !should_tail_duplicate(body, block, simple, size) {
        return None;
    }
    if simple {
        return duplicate_simple(body, at);
    }
    let preds: Vec<usize> = (0..body.blocks.len()).filter(|one| body.blocks[*one].succ.contains(&tail)).collect();
    // The block just before the tail in the layout that jumps to it is the one
    // the pin sees fall through.
    let falls = |pred: usize| {
        pred + 1 == at
            && real(&body.blocks[pred]).last().is_some_and(|last| {
                operation(last) == Some(Operation::Jump)
                    && last.what.as_ref().is_some_and(|what| what.target == Some(tail))
            })
    };
    let mut chosen: Vec<usize> = preds
        .iter()
        .copied()
        .filter(|pred| *pred != at && can_tail_duplicate(&body.blocks[*pred]) && !falls(*pred))
        .collect();
    let left: Vec<usize> = preds.iter().copied().filter(|pred| !chosen.contains(pred)).collect();
    if let [only] = left[..]
        && falls(only)
        && can_tail_duplicate(&body.blocks[only])
    {
        chosen.push(only);
    }
    if chosen.is_empty() {
        return None;
    }
    let remains = preds.iter().any(|pred| !chosen.contains(pred)) || tail == body.entry;
    let mut next = body.next_value();
    let mut blocks: Vec<Block> = Vec::new();
    let mut made: HashMap<usize, Vec<Arc<Insn>>> = HashMap::new();
    for &pred in &chosen {
        let mut fresh: HashMap<u32, u32> = HashMap::new();
        for one in block.insns.iter() {
            for value in &one.defines {
                fresh
                    .entry(*value)
                    .or_insert_with(
                        || {
                            next += 1;
                            next - 1
                        },
                    );
            }
        }
        let host = &body.blocks[pred];
        let mut insns: Vec<Arc<Insn>> = host.insns.to_vec();
        let jump = insns.iter().rposition(|one| operation(one) == Some(Operation::Jump)).expect("a jump to the tail");
        let beside = Arc::clone(&insns[jump]);
        insns.remove(jump);
        for one in block.insns.iter() {
            let copy = one.renamed(&|value| fresh.get(&value).copied().unwrap_or(value));
            insns.push(super::jumps::_unowned(&copy, &beside));
        }
        made.insert(pred, insns);
    }
    for (place, old) in body.blocks.iter().enumerate() {
        if place == at && !remains {
            continue;
        }
        if let Some(insns) = made.remove(&place) {
            let mut new = LirBlock::clone(old);
            new.insns = insns.into();
            new.succ = block.succ.clone();
            blocks.push(new.into());
        } else if old.succ.iter().any(|to| block.succ.contains(to)) || !old.phis.is_empty() {
            let mut new = LirBlock::clone(old);
            if block.succ.contains(&old.at) {
                let phis: Vec<_> = old
                    .phis
                    .iter()
                    .map(|phi| {
                        let mut phi = phi.clone();
                        let from_tail: Vec<u32> =
                            phi.incoming.iter().filter(|(pred, _)| *pred == tail).map(|(_, value)| *value).collect();
                        for &pred in &chosen {
                            let at_pred = body.blocks[pred].at;
                            for value in &from_tail {
                                phi.incoming.push((at_pred, *value));
                            }
                        }
                        if !remains {
                            phi.incoming.retain(|(pred, _)| *pred != tail);
                        }
                        phi
                    })
                    .collect();
                new.phis = phis.into();
            }
            blocks.push(new.into());
        } else {
            blocks.push(old.clone());
        }
    }
    let mut made_body = body.with_blocks(blocks);
    for &pred in &chosen {
        let from = body.blocks[pred].at;
        for to in block.succ.iter() {
            if let Some(odds) = body.odds.taken.get(&(tail, *to)).copied() {
                made_body.odds.taken.insert((from, *to), odds);
            }
        }
        made_body.odds.taken.shift_remove(&(from, tail));
    }
    if !remains {
        made_body.odds.taken.retain(|(from, _), _| *from != tail);
    }
    Some(made_body)
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

    fn insn_names(block: &Block) -> Vec<String> {
        block.insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// The join of two arms goes into both and is gone: each arm ends with the
    /// join's instruction and its jump, the join's value is new in each copy.
    #[test]
    fn test_a_join_is_copied_into_each_arm_and_removed() {
        let shape = join(vec![work(3)]);
        let done = tail_duplicate(&shape, 3, false).expect("duplicated");
        assert!(done.blocks.iter().all(|block| block.at != 3), "the join is gone");
        for arm in [1, 2] {
            let block = at(&done, arm);
            assert_eq!(insn_names(block), ["mov", "mov", "jmp"], "arm {arm}");
            assert_eq!(block.succ.iter().copied().collect::<Vec<_>>(), [9]);
        }
        let values: Vec<u32> =
            [1, 2].iter().flat_map(|arm| at(&done, *arm).insns.iter().flat_map(|one| one.defines.clone())).collect();
        assert_eq!(values.len(), 4);
        assert!(values[1] != 3 && values[3] != 3 && values[1] != values[3], "new values per copy: {values:?}");
    }

    /// A block of one jump (`duplicateSimpleBB`) is no copy: each predecessor
    /// goes to its successor, a conditional one by its branch.
    #[test]
    fn test_a_block_of_one_jump_is_bypassed_by_every_predecessor() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(3), false), jump(2)], &[3, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let mut shape = body(blocks);
        shape.odds.taken.insert((0, 3), 777);
        let done = tail_duplicate(&shape, 3, false).expect("bypassed");
        assert!(done.blocks.iter().all(|block| block.at != 3), "the jump block is gone");
        assert_eq!(at(&done, 0).succ.iter().copied().collect::<Vec<_>>(), [9, 2]);
        assert_eq!(at(&done, 2).succ.iter().copied().collect::<Vec<_>>(), [9]);
        let branch = at(&done, 0).insns.iter().find_map(|one| one.what.as_ref().filter(|w| w.op == Operation::Branch));
        assert_eq!(branch.and_then(|w| w.target), Some(9));
        assert_eq!(done.odds.taken.get(&(0, 9)), Some(&777));
    }

    /// A predecessor that cannot take a copy (two successors) stops a block
    /// that is not one jump: `canCompletelyDuplicateBB`.
    #[test]
    fn test_one_predecessor_that_cannot_take_a_copy_keeps_the_join_whole() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(1, vec![work(1), of("jne", Operation::Branch, Some(3), false), jump(9)], &[3, 9]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        assert!(tail_duplicate(&body(blocks), 3, false).is_none());
    }

    /// The successor's phi reads from each arm what it read from the join.
    #[test]
    fn test_a_successors_phi_reads_from_each_copy_what_it_read_from_the_join() {
        use crate::model::lir::Phi;
        let phi = Phi { result: 50, incoming: vec![(3, 7)] };
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            LirBlock { phis: vec![phi].into(), ..block(9, vec![of("ret", Operation::Return, None, false)], &[]) },
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        let mut from: Vec<(i64, u32)> = at(&done, 9).phis[0].incoming.clone();
        from.sort();
        assert_eq!(from, [(1, 7), (2, 7)], "the join is no predecessor any more");
    }

    /// A value the join defines and another block reads is the next step's: the
    /// SSA update.
    #[test]
    fn test_a_value_read_outside_the_join_is_left_for_the_ssa_update() {
        use crate::model::lir::Phi;
        let phi = Phi { result: 50, incoming: vec![(3, 3)] };
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            LirBlock { phis: vec![phi].into(), ..block(9, vec![of("ret", Operation::Return, None, false)], &[]) },
        ];
        assert!(tail_duplicate(&body(blocks), 3, false).is_none());
    }

    /// The join's edge odds are the copy's.
    #[test]
    fn test_the_joins_edge_odds_move_to_each_copy() {
        let mut shape = join(vec![work(3)]);
        shape.odds.taken.insert((3, 9), 1234);
        let done = tail_duplicate(&shape, 3, false).expect("duplicated");
        assert_eq!(done.odds.taken.get(&(1, 9)), Some(&1234));
        assert_eq!(done.odds.taken.get(&(2, 9)), Some(&1234));
        assert!(!done.odds.taken.keys().any(|(from, _)| *from == 3));
    }
}
