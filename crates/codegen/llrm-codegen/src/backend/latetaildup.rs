//! The stand-alone tail duplication after register allocation, before block
//! placement: `tailDuplicateBlocks` with `PreRegAlloc` clear and `LayoutMode`
//! clear, of
//! [TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L274).
//!
//! # How it works
//! A block is a tail when it cannot fall through, is no single-block loop and
//! holds at most two instructions counting its jump (one at `-Os`)
//! ([`shouldTailDuplicate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L562)).
//! After allocation nothing else is asked of it: there are no phis to
//! update, no value to rename (the registers are the machine's), and
//! `canCompletelyDuplicateBB` is not consulted. Each predecessor that goes to
//! it by a jump with no condition takes its instructions in place of the jump;
//! the tail goes when nothing reaches it. A block of one jump is bypassed
//! (`duplicateSimpleBB`).
//!
//! ```text
//!   b1: ... jmp b3        b1: ... <b3's instructions> jmp b4
//!   b2: ... jmp b3   ==>  b2: ... <b3's instructions> jmp b4
//!   b3: x; jmp b4         (b3 goes when nothing else reaches it)
//! ```
//!
//! DEVIATE TL1 (owner greedy-subreg, exit 2026-11-27): a tail with a call, a
//! call's record, a group, a symbol, bytes spread over others, an indirect jump
//! or a sibling call is not copied. The pin copies a call or a return after
//! allocation; here a copy keeps no callee (the callee is named by the
//! instruction's id) and claims no source bytes of its own (placement's
//! `copied` refuses the same).
//!
//! DEVIATE TL3 (owner greedy-subreg, exit 2026-11-20): the pin's return block
//! already holds the frame's teardown, so a framed function's is longer than
//! the bound and is never copied; here the teardown is emitted after this pass,
//! so a tail that returns counts the callee-saved registers the body writes.
//! Without it queens m64 -O2 was +24 B (copies the tail merge joined again).
//!
//! DEVIATE TL2 (no number): `Preds` is rebuilt after each tail copied, so a
//! function with many copied tails costs one pass over its blocks each; the pin
//! updates the CFG in place.
//!
//! # Example
//! The unit tests: a join of two arms holding one `mov` is copied into both
//! and removed; the same join holding a call is kept.
//!
//! # Used by
//! `flow::machine_with`, directly before block placement, when the target
//! asks for it.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::earlytaildup::{
    Preds, can_fall_through, can_tail_duplicate, duplicate_simple, forgotten, is_simple_in, operation, real,
};
use crate::model::ir::Operation;
use crate::model::lir::{self, Block, Insn, LirBlock, LirBody};

/// `TailDuplicateSize`.
const SIZE: usize = 2;

/// The tail's instructions as they would be copied, or none where it may not
/// be: [`shouldTailDuplicate`] after allocation, with this crate's refusals
/// (TL1).
fn copied(
    tail: &Block,
    size: bool,
    epilogue: usize,
) -> Option<Vec<Arc<Insn>>> {
    if can_fall_through(tail) || tail.succ.contains(&tail.at) || !tail.phis.is_empty() {
        return None;
    }
    if tail
        .insns
        .iter()
        .any(|one| one.call.is_some() || one.group.is_some() || one.symbol == Some(true) || !one.spread.is_empty())
    {
        return None;
    }
    let most = if size { 1 } else { SIZE };
    let work = super::jumps::_real(tail);
    let plain = work
        .iter()
        .all(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| !matches!(
                        what.op,
                        Operation::Barrier | Operation::Data | Operation::Call
                    )
                        && !what.is_sibling_call()
                        && !(matches!(what.op, Operation::Jump | Operation::Branch) && what.indirect),
                ),
        );
    // TL3: the pin's return block holds the frame's teardown (PEI ran before
    // this pass); here the pops are emitted later, so a tail that returns
    // counts the registers the function saves.
    let returns = work.last().is_some_and(|one| operation(one) == Some(Operation::Return));
    let length = work.len() + if returns { epilogue } else { 0 };
    (plain && length <= most).then_some(work)
}

/// `tailDuplicate` of `body.blocks[at]`: its instructions in place of the jump
/// to it in each predecessor that can take them. None where nothing changes.
fn tail_duplicate(
    graph: &Preds,
    body: &LirBody,
    at: usize,
    size: bool,
    epilogue: usize,
) -> Option<LirBody> {
    let tail = &body.blocks[at];
    let work = copied(tail, size, epilogue)?;
    if is_simple_in(graph, tail) {
        return duplicate_simple(body, at);
    }
    let preds: Vec<usize> = graph.places(tail.at).to_vec();
    // The block just before the tail that jumps to it is the one the pin sees
    // fall through; it takes the copy only when it is the last one left.
    let falls = |pred: usize| {
        pred + 1 == at
            && real(&body.blocks[pred]).next_back().is_none_or(|last| {
                operation(last) == Some(Operation::Jump)
                    && last.what.as_ref().is_some_and(|what| what.target == Some(tail.at))
            })
    };
    let mut chosen: Vec<usize> = preds
        .iter()
        .copied()
        .filter(|&pred| pred != at && can_tail_duplicate(&body.blocks[pred]) && !falls(pred))
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
    let remains = preds.iter().any(|pred| !chosen.contains(pred)) || tail.at == body.entry;
    let mut blocks: Vec<Block> = Vec::new();
    for (place, old) in body.blocks.iter().enumerate() {
        if place == at && !remains {
            continue;
        }
        if chosen.contains(&place) {
            // A predecessor with no instruction at all (it only falls into the tail) is
            // given a stand-in at its own place to anchor the copy's addresses.
            let jump = old
                .insns
                .iter()
                .rev()
                .find(|one| {
                    operation(one) == Some(Operation::Jump)
                        && one.what.as_ref().is_some_and(|what| what.target == Some(tail.at))
                })
                .cloned()
                .or_else(|| old.insns.last().cloned())
                .unwrap_or_else(|| Arc::new(Insn::new(old.at, Some((old.at, old.at)), None, Vec::new(), Vec::new())));
            // A predecessor that falls into the tail has no jump to drop: its last
            // instruction stays (it was lost, `add eax, 32` of __clzdi2).
            let jumps = operation(&jump) == Some(Operation::Jump)
                && jump.what.as_ref().is_some_and(|what| what.target == Some(tail.at));
            let mut insns: Vec<Arc<Insn>> = old
                .insns
                .iter()
                .map(|one| if jumps && Arc::ptr_eq(one, &jump) { lir::bytes_only(one) } else { Arc::clone(one) })
                .collect();
            insns.extend(work.iter().map(|one| super::jumps::_unowned(one, &jump)));
            let mut new = LirBlock::clone(old);
            new.insns = insns.into();
            new.succ = tail.succ.clone();
            blocks.push(new.into());
        } else {
            blocks.push(old.clone());
        }
    }
    let mut made = body.with_blocks(blocks);
    for &pred in &chosen {
        let from = body.blocks[pred].at;
        for to in tail.succ.iter() {
            if let Some(odds) = body.odds.taken.get(&(tail.at, *to)).copied() {
                made.odds.taken.insert((from, *to), odds);
            }
        }
        made.odds.taken.shift_remove(&(from, tail.at));
    }
    if !remains {
        made.odds.taken.retain(|(from, _), _| *from != tail.at);
    } else {
        made.frequencies = forgotten(&made, tail.at);
    }
    Some(made)
}

/// `tailDuplicateBlocks`: every block but the entry, once, in order.
pub fn duplicated(
    mut body: LirBody,
    size: bool,
    saved: &[RegId],
    free: bool,
    pointer: RegId,
) -> LirBody {
    let epilogue = saved_count(&body, saved, free, pointer);
    let mut place = 1;
    let mut graph = Preds::of(&body);
    while place < body.blocks.len() {
        let at = body.blocks[place].at;
        // Most blocks fail the cheap tests: no scan of the body for them.
        if copied(&body.blocks[place], size, epilogue).is_some()
            && let Some(made) = tail_duplicate(&graph, &body, place, size, epilogue)
        {
            body = made;
            graph = Preds::of(&body);
            // The tail may be gone: what stands at `place` is the next block.
            if body.blocks.get(place).is_none_or(|block| block.at != at) {
                continue;
            }
        }
        place += 1;
    }
    body
}

/// How many callee-saved registers the body writes: the pops its returns will
/// take (`masm::saved_of`, without the interrupt and runtime-entry cases). A
pub(crate) fn saved_count(
    body: &LirBody,
    saved: &[RegId],
    free: bool,
    pointer: RegId,
) -> usize {
    if let Some(kept) = &body.frame_saved {
        return kept.len();
    }
    let found = body.facts.0.get::<super::masm::BodyRoots>(body);
    let mut roots = if free { found.of_values(body.regs(), pointer) } else { found.plain.clone() };
    roots.extend(found.disturbed.iter());
    saved.iter().filter(|whole| roots.contains(whole)).count()
}

/// The phase: tail duplication after allocation.
pub struct TailDuplicate {
    pub size: bool,
    /// The callee-saved registers (whole), whether the frame register is free,
    /// and the frame pointer: what `masm::saved_of` reads.
    pub saved: Vec<RegId>,
    pub free: bool,
    pub pointer: RegId,
}

impl crate::model::passes::LIRTransform for TailDuplicate {
    fn class_name(&self) -> &'static str {
        "TailDuplicate"
    }

    fn name(&self) -> &str {
        "taildup"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(duplicated(body, self.size, &self.saved, self.free, self.pointer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Imm, Loc, Semantics};
    use crate::support::hash::IndexMap;

    fn work(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(crate::model::ir::Held { value, width: 4, offset: 0 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
    }

    fn of(
        name: &str,
        op: Operation,
        target: Option<i64>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), target, ..Semantics::new(op) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn jump(to: i64) -> Arc<Insn> {
        of("jmp", Operation::Jump, Some(to))
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

    /// Two arms jump to a join of `insns` that jumps on to 9.
    fn join(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut tail = insns;
        tail.push(jump(9));
        body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2)), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(9, vec![work(8), work(9), of("ret", Operation::Return, None)], &[]),
            block(3, tail, &[9]),
        ])
    }

    /// `duplicated` for a function that saves nothing.
    fn dup(
        body: LirBody,
        size: bool,
    ) -> LirBody {
        duplicated(body, size, &[], false, RegId::default())
    }

    fn names(block: &Block) -> Vec<String> {
        block
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().filter(|name| !name.is_empty()).map(str::to_owned))
            .collect()
    }

    fn at(
        body: &LirBody,
        block: i64,
    ) -> &Block {
        body.blocks.iter().find(|one| one.at == block).unwrap()
    }

    /// sfm16_float_cmp -Os (5 B per object on the pin): the join of two arms,
    /// one move and a jump, stayed a block both arms jumped to. After
    /// allocation it is copied verbatim into each arm (the registers are the
    /// machine's: no value is renamed) and the join is gone.
    #[test]
    fn test_a_join_is_copied_verbatim_into_each_arm_and_removed() {
        let done = dup(join(vec![work(3)]), false);
        assert!(done.blocks.iter().all(|block| block.at != 3), "the join is gone");
        for arm in [1, 2] {
            assert_eq!(names(at(&done, arm)), ["mov", "mov", "jmp"], "arm {arm}");
            assert_eq!(at(&done, arm).succ.iter().copied().collect::<Vec<_>>(), [9]);
        }
        let copies: Vec<Vec<u32>> = [1, 2]
            .iter()
            .map(|arm| at(&done, *arm).insns.iter().flat_map(|one| one.defines.clone()).collect())
            .collect();
        assert_eq!(copies, [vec![1, 3], vec![2, 3]], "the copy defines what the tail defined");
    }

    /// A tail of two instructions is too long; at `-Os` one is (the jump alone
    /// is the most).
    #[test]
    fn test_the_size_bound_is_two_instructions_and_one_at_os() {
        assert!(!dup(join(vec![work(3), work(4)]), false).blocks.iter().all(|b| b.at != 3));
        assert!(dup(join(vec![]), true).blocks.iter().all(|b| b.at != 3));
        assert!(!dup(join(vec![work(3)]), true).blocks.iter().all(|b| b.at != 3));
    }

    /// queens `place` (m64 -O2, +24 B): a return block of a function that saves
    /// registers has the pops in it in the pin, so it is too long to copy;
    /// copied here, tail merging joined the copies again. A frameless
    /// function's `ret` is copied.
    #[test]
    fn test_a_tail_that_returns_counts_the_pops_it_will_take() {
        let shape = || {
            body(vec![
                block(0, vec![of("jne", Operation::Branch, Some(2)), jump(1)], &[1, 2]),
                block(1, vec![work(1), jump(3)], &[3]),
                block(2, vec![work(2), jump(3)], &[3]),
                block(9, vec![work(8), work(9), of("ret", Operation::Return, None)], &[]),
                block(3, vec![work(3), of("ret", Operation::Return, None)], &[]),
            ])
        };
        assert!(dup(shape(), false).blocks.iter().all(|block| block.at != 3), "frameless: copied");
        let mut framed = shape();
        framed.frame_saved = Some(vec![RegId::default(), RegId::default()]);
        assert!(duplicated(framed, false, &[], false, RegId::default()).blocks.iter().any(|block| block.at == 3));
    }

    /// A call is not copied (TL1): a copy keeps no callee.
    #[test]
    fn test_a_tail_with_a_call_is_kept() {
        let kept = dup(join(vec![of("call", Operation::Call, Some(7))]), false);
        assert!(kept.blocks.iter().any(|block| block.at == 3));
    }

    /// A predecessor that ends in a condition cannot take a copy; the others
    /// do, and the tail stays for it (after allocation there is no
    /// `canCompletelyDuplicateBB`).
    #[test]
    fn test_a_predecessor_with_a_condition_leaves_the_tail_for_it() {
        let shape = body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(3)), jump(1)], &[1, 3]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(9, vec![work(8), work(9), of("ret", Operation::Return, None)], &[]),
            block(3, vec![work(3), jump(9)], &[9]),
        ]);
        let done = dup(shape, false);
        assert_eq!(names(at(&done, 1)), ["mov", "mov", "jmp"]);
        assert!(done.blocks.iter().any(|block| block.at == 3), "block 0 still reaches the tail");
    }
    /// sfm16_float_i2f `-m64 -O1` (any of six softfloat helpers together): a predecessor with no
    /// instruction that only falls into the tail was chosen to take the copy and the compile
    /// panicked with "a block with an instruction". It takes the copy like the other arm.
    #[test]
    fn test_an_empty_predecessor_that_falls_into_the_tail_takes_the_copy() {
        let shape = body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2)), jump(1)], &[1, 2]),
            block(1, Vec::new(), &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(9, vec![work(8), work(9), of("ret", Operation::Return, None)], &[]),
        ]);
        let done = dup(shape, false);
        assert_eq!(names(at(&done, 1)), ["mov", "jmp"]);
        assert_eq!(names(at(&done, 2)), ["mov", "mov", "jmp"]);
    }
}
