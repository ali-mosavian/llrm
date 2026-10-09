//! LLVM's PostRAMachineSinking: a copy whose result only one successor reads moves into that successor.
//!
//! `mov edi, edx` ahead of `test eax, eax / jne body` is dead on the path that returns at once, where nothing
//! reads EDI (a saved register's incoming value is on the stack): the copy belongs to the block that reads it.
//! Shrink wrapping can then place the prologue after the test, as gcc and LLVM do. Sound while the successor
//! is entered only from here, nothing between the copy and the branch reads its result or writes its source,
//! and no other successor has the result live.

use std::sync::Arc;

use crate::support::hash::IndexMap;

use crate::backend::liveness::{_before, _effects, _universe};
use crate::backend::peephole::{id, Lanes, _lanes};
use crate::backend::copysink::{copy_of, touches};
use crate::model::lir::{self, Insn, LirBlock, LirBody};

/// The lanes live on entry to each block.
fn live_in(body: &LirBody) -> IndexMap<i64, Lanes> {
    let universe = _universe();
    let mut into: IndexMap<i64, Lanes> = body.blocks.iter().map(|block| (block.at, Lanes::new())).collect();
    // Decoded once: the fixed point reads each block several times.
    let decoded: IndexMap<i64, _> = body.blocks.iter().map(|block| (block.at, _effects(body.bits, block))).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for block in body.blocks.iter().rev() {
            let after: Lanes = block.succ.iter().filter_map(|to| into.get(to)).flat_map(|lanes| lanes.iter().copied()).collect();
            let before = _before(&decoded[&block.at], after, &universe);
            if before != into[&block.at] {
                into.insert(block.at, before);
                changing = true;
            }
        }
    }
    into
}

/// `body` with each copy moved into the one successor that reads it. A copy that reached a block that itself branches may go on,
/// so the blocks that received one are asked again; liveness is the pass's whole cost, and is worked out only for a body that has a
/// copy in a block to ask.
pub fn sunk(body: &LirBody) -> LirBody {
    let mut body = body.clone();
    let mut asked: Option<Vec<i64>> = None;
    for _ in 0..8 {
        match round(&body, asked.as_deref()) {
            Some((next, received)) => {
                body = next;
                asked = Some(received);
            }
            None => break,
        }
    }
    body
}

fn round(body: &LirBody, asked: Option<&[i64]>) -> Option<(LirBody, Vec<i64>)> {
    let ask = |block: &LirBlock| block.succ.len() > 1 && asked.is_none_or(|asked| asked.contains(&block.at));
    // Liveness is the cost: ask for it only where a block that branches ends in a copy-able prefix.
    if !body.blocks.iter().any(|block| ask(block) && block.insns.iter().any(|one| copy_of(one).is_some())) {
        return None;
    }
    let live = live_in(body);
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut preds: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            *preds.entry(*to).or_insert(0) += 1;
        }
    }
    let mut moved: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut removed: crate::support::hash::HashSet<usize> = crate::support::hash::HashSet::default();
    for block in &body.blocks {
        if !ask(block) {
            continue;
        }
        for (index, one) in block.insns.iter().enumerate().rev() {
            let Some((dest, source)) = copy_of(one) else { continue };
            let (written, read) = (_lanes(dest.register), _lanes(source.register));
            if written.is_empty() || read.is_empty() || !written.is_disjoint(&read) {
                continue;
            }
            let readers: Vec<i64> = block.succ.iter().copied().filter(|to| live.get(to).is_some_and(|lanes| !written.is_disjoint(lanes))).collect();
            let [target] = readers[..] else { continue };
            if target == block.at || !at_of.contains_key(&target) || preds.get(&target) != Some(&1) || block.succ.iter().filter(|to| **to == target).count() != 1 {
                continue;
            }
            let both = written.or(&read);
            if block.insns[index + 1..].iter().any(|other| !removed.contains(&id(other)) && (touches(body.bits, other, &both, false) || touches(body.bits, other, &written, true))) {
                continue;
            }
            removed.insert(id(one));
            // Bottom-up, so each earlier copy lands ahead of the later ones already there.
            moved.entry(target).or_default().insert(0, Arc::clone(one));
        }
    }
    if removed.is_empty() {
        return None;
    }
    let blocks: Vec<LirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns = block.insns.clone();
            if insns.iter().any(|one| removed.contains(&id(one))) {
                insns = lir::without(&insns, |one| removed.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
            }
            if let Some(moved) = moved.get(&block.at) {
                insns = moved.iter().cloned().chain(insns).collect();
            }
            block.with_insns(insns)
        })
        .collect();
    Some((body.with_blocks(blocks), moved.keys().copied().collect()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register::{self, AX, BX, DI, DX};

    use super::sunk;
    use crate::model::ir::{Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn r(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn insn(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at)), Some(Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }), vec![], vec![]))
    }

    fn copy(at: i64, dest: Register, source: Register) -> Arc<Insn> {
        insn(at, Operation::Move, "mov", vec![r(dest)], vec![r(source)], None)
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    /// `mov di, dx; cmp ax, bx; jl 9`, the loop at 9 reading DI, the exit at 5 not: the shape of a function that leaves at once or
    /// works with a copy of its argument.
    fn shape(exit_reads: bool, body_has_two_predecessors: bool) -> LirBody {
        let cmp = insn(2, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None);
        let branch = insn(3, Operation::Branch, "jl", vec![], vec![], Some(9));
        // A generated return reads only what it requires and the epilogue: DI is not among them.
        let ret = Arc::new(Insn { reads_complete: true, ..Arc::unwrap_or_clone(insn(5, Operation::Return, "ret", vec![], vec![], None)) });
        let use_di = |at| insn(at, Operation::Binary, "add", vec![r(AX)], vec![r(AX), r(DI)], None);
        let mut blocks = vec![
            block(1, vec![copy(1, DI, DX), cmp, branch], vec![9, 5]),
            block(5, if exit_reads { vec![use_di(5), ret] } else { vec![ret] }, vec![]),
        ];
        let tail = if body_has_two_predecessors { vec![use_di(9), insn(10, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None), insn(11, Operation::Branch, "jl", vec![], vec![], Some(9)), ] } else { vec![use_di(9), insn(10, Operation::Jump, "jmp", vec![], vec![], Some(5))] };
        blocks.push(block(9, tail, if body_has_two_predecessors { vec![9, 5] } else { vec![5] }));
        LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default())
    }

    fn copies_in(body: &LirBody, at: i64) -> usize {
        body.blocks.iter().find(|block| block.at == at).unwrap().insns.iter().filter(|one| one.what.as_ref().unwrap().name.as_deref() == Some("mov")).count()
    }

    /// The copy of a saved register's argument ran on the path that returns at once, where nothing reads it, and kept the prologue
    /// from moving past the test (hanoi, fib: -7% instructions).
    #[test]
    fn test_a_copy_only_one_successor_reads_moves_into_it() {
        let body = shape(false, false);
        let after = sunk(&body);
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (0, 1));
    }

    #[test]
    fn test_a_copy_both_successors_read_stays() {
        let after = sunk(&shape(true, false));
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (1, 0));
    }

    #[test]
    fn test_a_copy_does_not_move_into_a_block_with_another_predecessor() {
        let after = sunk(&shape(false, true));
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (1, 0));
    }
}
