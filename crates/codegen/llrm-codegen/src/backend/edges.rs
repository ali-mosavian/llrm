//! Breaks one edge of a body: a block of its own that jumps on.
//!
//! # How it works
//! `split` makes a block holding a jump to the edge's target, points the
//! source's branch (or its fall-through, by a jump after the branch) at it,
//! and makes the target's phis read it where they read the source. The
//! block's label is above every physical offset, in a namespace per body
//! entry, as the blocks `phielim` makes for copies on edges are.
//!
//! The block is laid out right after the source, as the pin's
//! `SplitCriticalEdge` places it, and `without_jumps_to_next` drops the jump
//! each of the two ends in to the block now next, or flips the branch that
//! goes to it (`updateTerminator`).
//!
//! # Example
//! The edge `b1 -> b4` of `b1: je b4; b2: ...; b4:` becomes `b1 -> e -> b4`
//! with `b1: jne b2; e: jmp b4; b2: ...`, and `b4`'s phi that read `b1` reads
//! `e`.
//!
//! # Used by
//! `machinesink`, and `phielim` for the blocks it makes.

use std::sync::Arc;

use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Block, Insn, LirBlock, LirBody};

/// `blocks` with the end of each block `touched` names settled for the order:
/// a last jump to the block laid out next is dropped, and in `jcc next; jmp m`
/// the branch takes the other condition to `m`.
///
/// A split puts a block between two that were next to each other, so the
/// jumps it adds or retargets are explicit until the order is final; this is
/// the pin's `MachineBasicBlock::updateTerminator`.
pub fn without_jumps_to_next(
    blocks: &mut [Block],
    touched: &dyn Fn(i64) -> bool,
) {
    for index in 0..blocks.len().saturating_sub(1) {
        if !touched(blocks[index].at) {
            continue;
        }
        let next = blocks[index + 1].at;
        let insns = &blocks[index].insns;
        let mut real = insns.iter().rev().filter(|one| crate::backend::masm::prints(one));
        let (Some(jump), before) = (real.next().cloned(), real.next().cloned()) else { continue };
        let Some(what) = jump.what.as_ref().filter(|what| what.op == Operation::Jump && !what.indirect) else {
            continue;
        };
        let Some(onward) = what.target else { continue };
        let mut flips = false;
        let insns = if onward == next {
            lir::without(insns, |one| Arc::ptr_eq(one, &jump), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
        } else if let Some(branch) = before.filter(|one| {
            one.what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Branch && !what.indirect && what.target == Some(next))
        }) && let Some(opposite) = branch
            .what
            .as_ref()
            .and_then(|what| what.name.as_deref().and_then(|name| crate::backend::layout::_OPPOSITE.get(name)))
        {
            let what = branch.what.as_ref().expect("a branch");
            let flipped = Arc::new(Insn {
                what: Some(Semantics { name: Some((*opposite).into()), target: Some(onward), ..what.clone() }),
                ..(*branch).clone()
            });
            let flipped_in: Vec<Arc<Insn>> = insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, &branch) { Arc::clone(&flipped) } else { Arc::clone(one) })
                .collect();
            flips = true;
            lir::without(&flipped_in, |one| Arc::ptr_eq(one, &jump), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
        } else {
            continue;
        };
        let mut block = blocks[index].with_insns(insns);
        if flips {
            // The first successor is the branch's target.
            block.succ = std::iter::once(onward).chain(block.succ.iter().copied().filter(|&at| at != onward)).collect();
        }
        blocks[index] = block;
    }
}

/// `body` with the edge `from -> to` through a block of its own, and that
/// block's label; None where `from` has no such edge or has two to `to`.
pub fn split(
    body: &LirBody,
    from: i64,
    to: i64,
) -> Option<(LirBody, i64)> {
    let source = body.blocks.iter().find(|block| block.at == from)?;
    if source.succ.iter().filter(|next| **next == to).count() != 1 {
        return None;
    }
    let highest = body.blocks.iter().map(|block| block.at).max()?;
    let at = ((body.entry + 1) << 32).max(highest) + 1;
    let beside = source.insns.last()?;
    let jump = Semantics { name: Some("jmp".into()), target: Some(to), ..Semantics::new(Operation::Jump) };
    let mut made = Insn::new(at, Some((at, at)), Some(jump), vec![], vec![]);
    made.call = beside.call.clone();
    let made = Arc::new(made);
    let landing = |next: i64| if next == to { at } else { next };
    let made = LirBlock {
        at,
        insns: vec![made].into(),
        succ: vec![to].into(),
        phis: vec![].into(),
        cold: false,
        align: 0,
        max_skip: 0,
    };
    let mut blocks: Vec<Block> = Vec::with_capacity(body.blocks.len() + 1);
    for block in &body.blocks {
        if block.at == from {
            let mut insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .map(|one| {
                    let Some(moved) = one.what.as_ref().and_then(|what| what.remapped(landing)) else {
                        return Arc::clone(one);
                    };
                    let mut out = (**one).clone();
                    out.what = Some(moved);
                    Arc::new(out)
                })
                .collect();
            // A conditional branch falls through to its other successor, and
            // the new block now sits between: the fall-through is a jump.
            if let Some(last) = block.insns.last()
                && let Some(what) = last.what.as_ref().filter(|what| what.op == Operation::Branch)
                && let Some(fall) = source.succ.iter().copied().find(|&next| Some(next) != what.target)
            {
                let mut leave = Insn::new(
                    last.at,
                    Some((last.at, last.at)),
                    Some(Semantics {
                        name: Some("jmp".into()),
                        target: Some(landing(fall)),
                        ..Semantics::new(Operation::Jump)
                    }),
                    vec![],
                    vec![],
                );
                leave.call = last.call.clone();
                insns.push(Arc::new(leave));
            }
            let mut out = block.with_insns(insns);
            out.succ = block.succ.iter().map(|&next| landing(next)).collect::<Vec<_>>().into();
            blocks.push(out);
            blocks.push(made.clone().into());
        } else if block.at == to {
            let mut out = block.clone();
            for phi in out.phis.iter_mut() {
                for (pred, _) in &mut phi.incoming {
                    if *pred == from {
                        *pred = at;
                    }
                }
            }
            blocks.push(out);
        } else {
            blocks.push(block.clone());
        }
    }
    without_jumps_to_next(&mut blocks, &|one| one == from || one == at);
    let mut out = body.clone();
    out.odds.rerouted(from, &source.succ, to, &[(at, 1.0)]);
    out.blocks = blocks;
    Some((out, at))
}
