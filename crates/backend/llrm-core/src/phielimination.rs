//! `PHIElimination`: phis become copies, each phi through a register of its
//! own. Translated from llvm/lib/CodeGen/PHIElimination.cpp at d1106deb71cc
//! (`SplitPHIEdges`, `EliminatePHINodes`, `LowerPHINode`, `isLiveIn`,
//! `isLiveOutPastPHIs`).
//!
//! # How it works
//! A phi `d = phi [p1: a, p2: b]` becomes `i = copy a` at the end of `p1`
//! (before its terminators), `i = copy b` at the end of `p2`, and `d = copy i`
//! at the top of the phi's block, where `i` is a register made for this phi.
//! Nothing reads `i` but that copy, so the copies on one edge cannot disturb
//! one another (no parallel copy, no lost copy, no swap problem) and the pass
//! needs no price: the coalescer joins `i` with `a`, `b` and `d` where their
//! ranges allow.
//!
//! Before that, critical edges are split where the copy would otherwise stay
//! (`SplitPHIEdges`): an edge `p -> s` whose value is live out of `p` past the
//! phi and is not live into `s` (the interference is then avoidable by a block
//! on the edge), or that leaves a loop, unless it is a back edge or enters the
//! loop from outside. A block whose predecessors all have several successors
//! lowers identical phis through one register.
//!
//! DEVIATE P1 (no number): a phi with an edge from outside the body is left
//! alone, as before. Fact: LLVM's phis list only blocks of their function.
//!
//! DEVIATE P4 (exit: the x87 pair after allocation, plan step 4): a phi of an
//! x87 value is left for `phielim`'s own lowering. Fact: `FloatAssign` and
//! `FloatAlloc` run before the coalescer here, where LLVM's FPS runs after
//! register allocation, so a copy of an x87 value costs an `fld` and an `fstp`
//! that nothing joins away.
//!
//! DEVIATE P2 (no number): `IMPLICIT_DEF` and undef incoming values do not
//! exist in LIR, so `allPhiOperandsUndefined`, `ImpDefs` and `SrcUndef` are not
//! translated.
//!
//! DEVIATE P3 (no number): `MachineLoopInfo::getLoopFor` is the innermost loop
//! of the body's loop forest; `isLiveIn` and `isLiveOutPastPHIs` read the
//! values live at a block's entry (`allocate::live`), where
//! `LiveIntervals::liveAt` reads the interval. A phi's operand is read at the
//! end of its predecessor, so it is not live into the successor by that phi, as
//! in `LiveIntervals`.
//!
//! # Example
//! A loop carrying `i`: `i2 = phi [pre: i0, body: i1]` becomes `n = copy i0`
//! at the end of `pre`, `n = copy i1` at the end of `body` and `i2 = copy n` at
//! the top of the loop.
//!
//! # Used by
//! `backend::phielim::PhiElimination`.

use std::sync::Arc;

use crate::backend::phielim::{self, Split};
use crate::model::ir::Held;
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::support::hash::{HashMap, IndexMap};

/// `SplitPHIEdges`: the critical edges worth a block of their own, as
/// `(predecessor, block)`.
fn edges_to_split(
    body: &LirBody,
    at_of: &IndexMap<i64, &Block>,
) -> Vec<(i64, i64)> {
    let (entering, _) = crate::backend::allocate::live(body);
    let forest = crate::analysis::loops::forest_of(&body.blocks, Some(body.entry));
    let live_in = |value: u32, at: i64| entering.get(&at).is_some_and(|set| set.contains(&value));
    let mut split: Vec<(i64, i64)> = Vec::new();
    for block in &body.blocks {
        if block.phis.is_empty() {
            continue;
        }
        let cur_loop = forest.innermost(block.at);
        let is_loop_header = cur_loop.is_some_and(|one| forest.loops[one].header == block.at);
        for phi in block.phis.iter() {
            for &(pre, reg) in &phi.incoming {
                let Some(pre_block) = at_of.get(&pre) else { continue };
                // Is there a critical edge from PreMBB to MBB?
                if pre_block.succ.len() == 1 {
                    continue;
                }
                // Avoid splitting backedges of loops. It would introduce small
                // out-of-line blocks into the loop which is very bad for code
                // placement.
                if pre == block.at {
                    continue;
                }
                let pre_loop = forest.innermost(pre);
                if is_loop_header && pre_loop == cur_loop {
                    continue;
                }
                // The copy in PreMBB is not a kill when the value is live out
                // for some other reason than a phi use, and may not coalesce.
                let mut should_split = pre_block.succ.iter().any(|&succ| live_in(reg, succ));
                if !should_split {
                    continue;
                }
                // If Reg is not live-in to MBB, it must be live-in to some
                // other PreMBB successor, and splitting avoids the
                // interference. If it is, the interference is inevitable.
                should_split = !live_in(reg, block.at);
                // Check for a loop exiting edge.
                if !should_split && cur_loop != pre_loop {
                    // This edge could be entering a loop, exiting a loop, or
                    // both. Split unless it is entering CurLoop from an outer
                    // loop.
                    should_split = pre_loop.is_some_and(|outer| {
                        !cur_loop.is_some_and(|inner| forest.contains(outer, forest.loops[inner].header))
                    });
                }
                if should_split && !split.contains(&(pre, block.at)) {
                    split.push((pre, block.at));
                }
            }
        }
    }
    split
}

/// `body` with every phi lowered to copies.
pub fn lowered(body: &LirBody) -> Result<LirBody, String> {
    if body.blocks.iter().all(|block| block.phis.is_empty()) {
        return Ok(body.clone());
    }
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let widths = phielim::_widths(body);
    let kept_phis = |body: &LirBody| -> IndexMap<i64, Vec<Phi>> {
        body.blocks.iter().map(|block| (block.at, block.phis.to_vec())).collect()
    };
    let split = edges_to_split(body, &at_of);
    let split_body;
    let body = if split.is_empty() {
        body
    } else {
        let edges: IndexMap<(i64, i64), Split> = split.iter().map(|&edge| (edge, Split::Pairs(Vec::new()))).collect();
        split_body = phielim::_split_edges(
            body,
            &edges,
            &IndexMap::default(),
            &IndexMap::default(),
            &kept_phis(body),
            &widths,
            false,
            &|_, _| false,
        )?;
        &split_body
    };
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut next = crate::backend::splitkit::_next_value(body);
    let mut head: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut tails: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut left: IndexMap<i64, Vec<Phi>> = IndexMap::default();
    let mut preds: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for &to in block.succ.iter() {
            *preds.entry(to).or_insert(0) += 1;
        }
    }
    let floating = crate::backend::ssaspill::floating(body);
    for block in &body.blocks {
        let mut stays = Vec::new();
        // If all incoming edges are critical, identical phis are lowered
        // through one register, so fewer copies are made.
        let all_critical = preds.get(&block.at).copied().unwrap_or(0) >= 2
            && body.blocks.iter().filter(|one| one.succ.contains(&block.at)).all(|one| one.succ.len() >= 2);
        let mut lowered_phis: HashMap<Vec<(i64, u32)>, u32> = HashMap::default();
        for phi in block.phis.iter() {
            if phi.incoming.iter().any(|(pre, _)| !at_of.contains_key(pre)) || floating.contains(&phi.result) {
                stays.push(phi.clone());
                continue;
            }
            let width = widths[&phi.result];
            let key = {
                let mut key = phi.incoming.clone();
                key.sort_unstable();
                key
            };
            let reused = if all_critical { lowered_phis.get(&key).copied() } else { None };
            let incoming = reused.unwrap_or_else(|| {
                let made = next;
                next += 1;
                made
            });
            if all_critical && reused.is_none() {
                lowered_phis.insert(key, incoming);
            }
            // The copy at the top of the block, after any remaining phis.
            head.entry(block.at).or_default().push(phielim::_copy(block, phi.result, incoming, None, width));
            if reused.is_some() {
                continue;
            }
            // The copy at the end of each predecessor, once for each.
            let mut done: Vec<i64> = Vec::new();
            for &(pre, source) in phi.incoming.iter().rev() {
                if done.contains(&pre) {
                    continue;
                }
                done.push(pre);
                tails.entry(pre).or_default().push(phielim::_copy(at_of[&pre], incoming, source, None, width));
            }
        }
        left.insert(block.at, stays);
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let first = block.insns.iter().position(|one| !one.arrival()).unwrap_or(block.insns.len());
        let tail = phielim::_before_the_terminator(block, tails.get(&block.at).map_or(&[][..], Vec::as_slice));
        insns.extend(tail[..first].iter().cloned());
        insns.extend(head.get(&block.at).into_iter().flatten().cloned());
        insns.extend(tail[first..].iter().cloned());
        let stays = &left[&block.at];
        *block = LirBlock { insns: insns.into(), phis: stays.clone().into(), ..(**block).clone() }.into();
    }
    Ok(out)
}

#[allow(dead_code)]
fn _held(
    value: u32,
    width: u32,
) -> Held {
    Held { value, width }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::coalesce::_copy as copy_of;
    use crate::model::ir::{Imm, Loc, Operation, Semantics};

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn def(v: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".to_owned()),
            dests: vec![held(v)],
            sources: vec![Loc::Imm(Imm { value: 7, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![v], vec![]))
    }

    fn read(v: u32) -> Arc<Insn> {
        Arc::new(Insn::new(0, Some((0, 0)), None, vec![], vec![v]))
    }

    fn jump(target: i64) -> Arc<Insn> {
        let what = Semantics { name: Some("jmp".to_owned()), target: Some(target), ..Semantics::new(Operation::Jump) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn branch(target: i64) -> Arc<Insn> {
        let what =
            Semantics { name: Some("jne".to_owned()), target: Some(target), ..Semantics::new(Operation::Branch) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
        phis: Vec<Phi>,
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), phis: phis.into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(
            "lowered",
            0,
            blocks.into_iter().map(Block::from).collect(),
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn copies(block: &Block) -> Vec<(u32, u32)> {
        block.insns.iter().filter_map(|one| copy_of(one)).collect()
    }

    /// Two counters carried round a loop: each phi gets a register of its own,
    /// the copies at the end of the loop write those, and the top of the loop
    /// copies them into the phis' results.
    #[test]
    fn test_independent_phis_are_lowered_through_registers_of_their_own() {
        let phis = vec![
            Phi { result: 10, incoming: vec![(0, 1), (1, 12)] },
            Phi { result: 11, incoming: vec![(0, 2), (1, 13)] },
        ];
        let was = body(vec![
            block(0, vec![def(1), def(2), jump(1)], &[1], vec![]),
            block(1, vec![def(12), def(13), read(10), read(11), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert!(now.blocks.iter().all(|one| one.phis.is_empty()), "no phi is left");
        let (pre, lop) = (copies(&now.blocks[0]), copies(&now.blocks[1]));
        assert_eq!(pre.len(), 2, "one copy for each phi at the end of the entry: {pre:?}");
        assert_eq!(lop.len(), 4, "{lop:?}");
        assert_eq!((lop[0].0, lop[1].0), (10, 11), "the results are copied at the top");
        let (first, second) = (lop[2], lop[3]);
        assert_eq!((first.1, second.1), (12, 13));
        assert!(first.0 != 10 && first.0 != 11 && second.0 != 10 && second.0 != 11, "into registers made for the phis");
        assert_eq!((lop[0].1, lop[1].1), (first.0, second.0), "the top reads what the end wrote");
    }

    /// `a = phi [.., b]`, `b = phi [.., c]`: a hands over `b`, which another
    /// phi of the block rewrites. Each phi is lowered through a register of
    /// its own, as LLVM's are; left to `phielim`'s parallel copy (DEVIATE P5)
    /// they cost idiv_loop_os 14% of its estimate at -O2 and recgcd as much.
    #[test]
    fn test_phis_that_read_each_other_are_lowered_like_any_other() {
        let phis = vec![
            Phi { result: 10, incoming: vec![(0, 1), (1, 11)] },
            Phi { result: 11, incoming: vec![(0, 2), (1, 12)] },
        ];
        let was = body(vec![
            block(0, vec![def(1), def(2), jump(1)], &[1], vec![]),
            block(1, vec![def(12), read(10), read(11), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert!(now.blocks.iter().all(|one| one.phis.is_empty()), "a phi was left");
        assert_eq!(copies(&now.blocks[0]).len(), 2, "one copy per phi on the entry edge");
    }

    /// The edge from a block with two successors to a phi's block is split
    /// when the phi's value is also live into the other successor (the copy
    /// would otherwise interfere with it): a block of only a jump is made.
    #[test]
    fn test_a_critical_edge_is_split_where_the_value_is_live_into_the_other_successor() {
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (3, 2)] }];
        let was = body(vec![
            block(0, vec![def(1), branch(2)], &[2, 1], vec![]),
            block(1, vec![read(1), jump(3)], &[3], vec![]),
            block(2, vec![read(10), read(1)], &[], vec![]),
            block(3, vec![def(2), jump(2)], &[2], vec![]),
        ]);
        // Block 2 is entered from 0 (value 1 live out and into block 1) and
        // from 3.
        let mut was = was;
        was.blocks[2] = Block::from(block(2, vec![read(10)], &[], phis));
        let now = lowered(&was).expect("lowers");
        assert_eq!(now.blocks.len(), 5, "a block was made on the edge 0 -> 2");
        let made = now.blocks.last().expect("the new block");
        assert_eq!(made.succ.to_vec(), vec![2]);
        assert!(!now.blocks[0].succ.contains(&2), "the branch no longer goes to the phi's block");
        assert_eq!(copies(made).len(), 1, "the copy is in the edge block");
    }
}
