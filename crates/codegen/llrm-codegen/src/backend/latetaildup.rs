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
) -> Option<Vec<Arc<Insn>>> {
    if can_fall_through(tail) || tail.succ.contains(&tail.at) || !tail.phis.is_empty() {
        return None;
    }
    if tail.insns.iter().any(|one| {
        one.call.is_some() || one.group.is_some() || one.symbol == Some(true) || !one.spread.is_empty()
    }) {
        return None;
    }
    let most = if size { 1 } else { SIZE };
    let work = super::jumps::_real(tail);
    let plain = work.iter().all(|one| {
        one.what.as_ref().is_some_and(|what| {
            !matches!(what.op, Operation::Barrier | Operation::Data | Operation::Call)
                && !what.is_sibling_call()
                && !(matches!(what.op, Operation::Jump | Operation::Branch) && what.indirect)
        })
    });
    (plain && work.len() <= most).then_some(work)
}

/// `tailDuplicate` of `body.blocks[at]`: its instructions in place of the jump
/// to it in each predecessor that can take them. None where nothing changes.
fn tail_duplicate(
    graph: &Preds,
    body: &LirBody,
    at: usize,
    size: bool,
) -> Option<LirBody> {
    let tail = &body.blocks[at];
    let work = copied(tail, size)?;
    if is_simple_in(graph, tail) {
        return duplicate_simple(body, at);
    }
    let preds: Vec<usize> = graph.places(tail.at).to_vec();
    // The block just before the tail that jumps to it is the one the pin sees
    // fall through; it takes the copy only when it is the last one left.
    let falls = |pred: usize| {
        pred + 1 == at
            && real(&body.blocks[pred]).next_back().is_none_or(|last| {
                operation(last) == Some(Operation::Jump) && last.what.as_ref().is_some_and(|what| what.target == Some(tail.at))
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
                .expect("a block with an instruction");
            let mut insns: Vec<Arc<Insn>> = old
                .insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, &jump) { lir::bytes_only(one) } else { Arc::clone(one) })
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
) -> LirBody {
    let mut place = 1;
    let mut graph = Preds::of(&body);
    while place < body.blocks.len() {
        let at = body.blocks[place].at;
        // Most blocks fail the cheap tests: no scan of the body for them.
        if copied(&body.blocks[place], size).is_some()
            && let Some(made) = tail_duplicate(&graph, &body, place, size)
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

/// The phase: tail duplication after allocation.
pub struct TailDuplicate {
    pub size: bool,
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
        Ok(duplicated(body, self.size))
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
        let done = duplicated(join(vec![work(3)]), false);
        assert!(done.blocks.iter().all(|block| block.at != 3), "the join is gone");
        for arm in [1, 2] {
            assert_eq!(names(at(&done, arm)), ["mov", "mov", "jmp"], "arm {arm}");
            assert_eq!(at(&done, arm).succ.iter().copied().collect::<Vec<_>>(), [9]);
        }
        let copies: Vec<Vec<u32>> =
            [1, 2].iter().map(|arm| at(&done, *arm).insns.iter().flat_map(|one| one.defines.clone()).collect()).collect();
        assert_eq!(copies, [vec![1, 3], vec![2, 3]], "the copy defines what the tail defined");
    }

    /// A tail of two instructions is too long; at `-Os` one is (the jump alone
    /// is the most).
    #[test]
    fn test_the_size_bound_is_two_instructions_and_one_at_os() {
        assert!(!duplicated(join(vec![work(3), work(4)]), false).blocks.iter().all(|b| b.at != 3));
        assert!(duplicated(join(vec![]), true).blocks.iter().all(|b| b.at != 3));
        assert!(!duplicated(join(vec![work(3)]), true).blocks.iter().all(|b| b.at != 3));
    }

    /// A call is not copied (TL1): a copy keeps no callee.
    #[test]
    fn test_a_tail_with_a_call_is_kept() {
        let kept = duplicated(join(vec![of("call", Operation::Call, Some(7))]), false);
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
        let done = duplicated(shape, false);
        assert_eq!(names(at(&done, 1)), ["mov", "mov", "jmp"]);
        assert!(done.blocks.iter().any(|block| block.at == 3), "block 0 still reaches the tail");
    }
}
