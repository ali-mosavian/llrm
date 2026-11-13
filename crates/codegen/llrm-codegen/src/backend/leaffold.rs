//! A constant a leaf block makes for a join, made in the block that jumps to
//! it.
//!
//! # How it works
//! A block of one constant and a jump (`v := mov 3; jmp J`) that one branch
//! reaches costs its jump at -Os. The branching block makes the constant itself
//! and the branch goes to `J`; the leaf is gone and `J`'s phi takes `v` from
//! the branching block. A chain of tests of a sparse switch ends in one such
//! leaf per case; gcc loads the result ahead of its compare, as this does.
//!
//! ```text
//!   p:  cmp x, 7                    p:  v := mov 3
//!       je l                            cmp x, 7
//!   l:  v := mov 3                      je j
//!       jmp j                       j:  r = phi [p: v, ...]
//!   j:  r = phi [l: v, ...]
//! ```
//!
//! # Deviation
//! DEVIATE LEAF-FOLD-OS (owner m32, exit 2026-11-20). The pin keeps the leaf:
//! `PHIElimination.cpp` splits an edge only for a value live out past the phis
//! (L841, L858, L879), and clang -Os emits `mov $c, %edi; jmp .LBB0_19` per
//! case. `Target::leaf_fold_at_size` turns it on (m32); only at -Os, where the
//! jump counts and the move on the path that does not branch does not.
//!
//! DEVIATE LEAF-FOLD-TRIAL (owner m32, exit 2026-11-24): the fold changes the
//! registers after it, so a module it folded is assembled again without it
//! (`driver::trial`) and the smaller object is kept.
//!
//! # Runs
//! Phase 1 of the machine phases, after `SharedConstants`, before `SsaSpill`.

use std::sync::Arc;

use crate::backend::phielim::{_constant_made_in, _nothing};
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct LeafFold {
    pub size: bool,
}

thread_local! {
    static FOLDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a fold was made since the last call (the trial's note).
pub fn take_folded() -> bool {
    FOLDED.with(|one| one.replace(false))
}

/// `body` run with the fold left off: the trial's second compile.
pub fn without<T>(body: impl FnOnce() -> T) -> T {
    OFF.with(|one| one.set(true));
    let made = body();
    OFF.with(|one| one.set(false));
    made
}

impl LIRTransform for LeafFold {
    fn class_name(&self) -> &'static str {
        "LeafFold"
    }

    fn name(&self) -> &str {
        "leaffold"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if !self.size || OFF.with(|one| one.get()) {
            return Ok(body);
        }
        let made = folded(&body);
        if made.blocks.len() != body.blocks.len() {
            FOLDED.with(|one| one.set(true));
        }
        Ok(made)
    }
}

/// The instructions of the block that do something.
fn real(block: &LirBlock) -> Vec<&Arc<Insn>> {
    block.insns.iter().filter(|one| one.what.is_some()).collect()
}

fn ends_with(
    block: &LirBlock,
    ops: &[Operation],
) -> Option<usize> {
    let last = block.insns.iter().rposition(|one| one.what.as_ref().is_some_and(|what| ops.contains(&what.op)))?;
    Some(last)
}

/// `body` with every leaf that makes a constant for a join folded into the
/// block that branches to it.
pub fn folded(body: &LirBody) -> LirBody {
    let at_of: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(place, block)| (block.at, place)).collect();
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in crate::model::lir::distinct(&block.succ) {
            preds.entry(to).or_default().push(block.at);
        }
    }
    let floating = crate::backend::regclass::floating(body);
    // leaf -> (branching block, join, value, the move)
    let mut chosen: IndexMap<i64, (i64, i64, u32, Semantics)> = IndexMap::default();
    let mut used: IndexMap<i64, ()> = IndexMap::default();
    for join in &body.blocks {
        for phi in join.phis.iter() {
            for &(from, value) in &phi.incoming {
                let Some(&place) = at_of.get(&from) else { continue };
                let leaf = &body.blocks[place];
                if chosen.contains_key(&from) || from == body.entry || from == join.at || !leaf.phis.is_empty() {
                    continue;
                }
                let [only] = preds.get(&from).map_or(&[][..], Vec::as_slice) else { continue };
                let pred = &body.blocks[at_of[only]];
                let shape = real(leaf);
                let ([made, jump], true) = (&shape[..], leaf.succ.len() == 1) else { continue };
                if jump.what.as_ref().map(|what| (what.op, what.target)) != Some((Operation::Jump, Some(join.at)))
                    || !made.defines.contains(&value)
                    || floating.contains(&value)
                {
                    continue;
                }
                let Some(what) = _constant_made_in(leaf, value) else { continue };
                if !matches!(
                    what.sources.first(),
                    Some(Loc::Imm(imm)) if imm.address.is_none()
                ) {
                    continue;
                }
                let reached = real(pred)
                    .iter()
                    .filter_map(|one| one.what.as_ref())
                    .any(|last| matches!(last.op, Operation::Jump | Operation::Branch) && last.target == Some(from));
                // One fold per branching block; none where it already reaches
                // the join (the phi would take two values from it).
                if !reached
                    || *only == join.at
                    || pred.succ.contains(&join.at)
                    || used.contains_key(only)
                    || pred.phis.iter().any(|one| one.result == value)
                {
                    continue;
                }
                used.insert(*only, ());
                chosen.insert(from, (*only, join.at, value, what));
            }
        }
    }
    if chosen.is_empty() {
        return body.clone();
    }
    let by_pred: IndexMap<i64, (&i64, &(i64, i64, u32, Semantics))> =
        chosen.iter().map(|(leaf, one)| (one.0, (leaf, one))).collect();
    let mut blocks: Vec<Block> = Vec::new();
    for old in &body.blocks {
        if chosen.contains_key(&old.at) {
            continue;
        }
        if let Some((leaf, (_, join, value, what))) = by_pred.get(&old.at).map(|(leaf, one)| (**leaf, *one)) {
            let mut new = LirBlock::clone(old);
            let mut insns: Vec<Arc<Insn>> = old
                .insns
                .iter()
                .map(|one| match one.what.as_ref() {
                    Some(w) if matches!(w.op, Operation::Jump | Operation::Branch) && w.target == Some(leaf) => {
                        let mut made = (**one).clone();
                        made.what = Some(Semantics { target: Some(*join), ..w.clone() });
                        Arc::new(made)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            let last = old.insns.last();
            let at = last.map_or(old.at, |last| last.at);
            let edge = _nothing(last.map(|last| &**last), at);
            let mov = Arc::new(Insn::new(
                at,
                Some(edge),
                Some(Semantics { target: None, ..what.clone() }),
                vec![*value],
                Vec::new(),
            ));
            // Ahead of the compare the branch reads: a `mov r, 0` between them
            // would keep the flags from being a `xor`'s to clobber.
            let mut cut =
                ends_with(old, &[Operation::Jump, Operation::Branch, Operation::Return]).map_or(insns.len(), |_| {
                    let mut cut = insns.len();
                    while cut > 0
                        && insns[cut - 1]
                            .what
                            .as_ref()
                            .is_some_and(|w| matches!(w.op, Operation::Jump | Operation::Branch | Operation::Return))
                    {
                        cut -= 1;
                    }
                    cut
                });
            while cut > 0 && insns[cut - 1].what.as_ref().is_some_and(|w| w.op == Operation::Compare) {
                cut -= 1;
            }
            insns.insert(cut, mov);
            new.insns = insns.into();
            let mut succ: Vec<i64> = Vec::new();
            for next in old.succ.iter().map(|next| if *next == leaf { *join } else { *next }) {
                if !succ.contains(&next) {
                    succ.push(next);
                }
            }
            new.succ = succ.into();
            // a branching block can also be the join of another folded leaf
            new.phis = retargeted(old, &chosen);
            blocks.push(new.into());
        } else if old.phis.iter().any(|phi| phi.incoming.iter().any(|(from, _)| chosen.contains_key(from))) {
            let mut new = LirBlock::clone(old);
            new.phis = retargeted(old, &chosen);
            blocks.push(new.into());
        } else {
            blocks.push(old.clone());
        }
    }
    let mut made = body.with_blocks(blocks);
    for (leaf, (from, join, ..)) in &chosen {
        if let Some(odds) = made.odds.taken.shift_remove(&(*from, *leaf)) {
            made.odds.taken.insert((*from, *join), odds);
        }
        made.odds.taken.retain(|(one, _), _| one != leaf);
    }
    made
}

/// `block`'s phis with each incoming edge from a folded leaf read from the
/// branching block that took its place.
fn retargeted(
    block: &LirBlock,
    chosen: &IndexMap<i64, (i64, i64, u32, Semantics)>,
) -> crate::model::lir::Shared<Phi> {
    block
        .phis
        .iter()
        .map(|phi| Phi {
            result: phi.result,
            incoming: phi
                .incoming
                .iter()
                .map(|&(from, value)| (chosen.get(&from).map_or(from, |one| one.0), value))
                .collect(),
        })
        .collect::<Vec<_>>()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Imm};

    fn of(
        name: &str,
        op: Operation,
        target: Option<i64>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), target, ..Semantics::new(op) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn make(
        value: u32,
        imm: i64,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value, width: 4, offset: 0 })],
            sources: vec![Loc::Imm(Imm { value: imm, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, insns) }
    }

    fn at(
        body: &LirBody,
        block: i64,
    ) -> Option<&Block> {
        body.blocks.iter().find(|one| one.at == block)
    }

    fn names(block: &Block) -> Vec<String> {
        block.insns.iter().filter_map(|one| Some(one.what.as_ref()?.name.clone()?.to_string())).collect()
    }

    /// Two tests of a switch, each with a leaf making its result for the join
    /// 9; `exit` is the join's own ending.
    fn chain(first: Arc<Insn>) -> LirBody {
        let join = LirBlock {
            phis: vec![Phi { result: 9, incoming: vec![(1, 11), (2, 12), (3, 13)] }].into(),
            ..block(9, vec![of("ret", Operation::Return, None)], &[])
        };
        LirBody::new(
            "t",
            0,
            [
                block(
                    0,
                    vec![of("cmp", Operation::Compare, None), of("je", Operation::Branch, Some(1)), first],
                    &[1, 4],
                ),
                block(1, vec![make(11, 3), of("jmp", Operation::Jump, Some(9))], &[9]),
                block(
                    4,
                    vec![
                        of("cmp", Operation::Compare, None),
                        of("je", Operation::Branch, Some(2)),
                        of("jmp", Operation::Jump, Some(3)),
                    ],
                    &[2, 3],
                ),
                block(2, vec![make(12, 5), of("jmp", Operation::Jump, Some(9))], &[9]),
                block(3, vec![make(13, 0), of("jmp", Operation::Jump, Some(9))], &[9]),
                join,
            ]
            .into_iter()
            .map(Block::from)
            .collect(),
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    /// -Os kept a `mov; jmp` leaf per case of a sparse switch (x_switch_sparse
    /// m32: 2 B of jump a case): the constant is made ahead of the compare and
    /// the branch goes to the join.
    #[test]
    fn test_a_leaf_of_a_constant_and_a_jump_is_made_in_the_branching_block() {
        let body = chain(of("jmp", Operation::Jump, Some(4)));
        let done = folded(&body);
        assert!(at(&done, 1).is_none() && at(&done, 2).is_none(), "the leaves are gone");
        assert_eq!(names(at(&done, 0).unwrap()), ["mov", "cmp", "je", "jmp"]);
        let branch =
            at(&done, 0).unwrap().insns.iter().find_map(|one| one.what.as_ref().filter(|w| w.op == Operation::Branch));
        assert_eq!(branch.and_then(|w| w.target), Some(9));
        let phi = &at(&done, 9).unwrap().phis[0];
        assert!(phi.incoming.contains(&(0, 11)) && phi.incoming.contains(&(4, 12)), "{:?}", phi.incoming);
        assert!(at(&done, 3).is_some() && phi.incoming.contains(&(3, 13)), "one fold a branching block");
    }

    /// The same for a block that jumps to its leaf, not branches.
    #[test]
    fn test_a_jump_to_the_leaf_goes_to_the_join_too() {
        let join = LirBlock {
            phis: vec![Phi { result: 9, incoming: vec![(1, 11), (5, 14)] }].into(),
            ..block(9, vec![of("ret", Operation::Return, None)], &[])
        };
        let body = LirBody::new(
            "t",
            0,
            [
                block(0, vec![of("jmp", Operation::Jump, Some(1))], &[1]),
                block(1, vec![make(11, 7), of("jmp", Operation::Jump, Some(9))], &[9]),
                block(5, vec![make(14, 8), of("jmp", Operation::Jump, Some(9))], &[9]),
                join,
            ]
            .into_iter()
            .map(Block::from)
            .collect(),
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = folded(&body);
        assert!(at(&done, 1).is_none());
        assert_eq!(at(&done, 0).unwrap().succ.iter().copied().collect::<Vec<_>>(), [9]);
        assert!(at(&done, 9).unwrap().phis[0].incoming.contains(&(0, 11)));
    }

    /// A leaf with a second predecessor stays (the constant would be made on
    /// a path that does not take it).
    #[test]
    fn test_a_leaf_with_two_predecessors_stays() {
        let mut body = chain(of("jmp", Operation::Jump, Some(4)));
        let mut blocks = body.blocks.clone();
        let extra = LirBlock::clone(&blocks[2]);
        let mut extra = extra;
        extra.succ = vec![2, 3, 1].into();
        blocks[2] = extra.into();
        body = body.with_blocks(blocks);
        assert!(at(&folded(&body), 1).is_some());
    }

    /// A block that branches to one leaf and is the join of another keeps its
    /// phis' predecessors true. The first fold left the join's phi reading from
    /// the folded-away leaf: -Oz panicked in `machinesink.rs:575` on
    /// bench/nbody_fixed and stft_fixed (a phi's `from` not in the body).
    #[test]
    fn test_a_block_that_is_a_branching_block_and_a_join_keeps_its_phis_predecessors() {
        let middle = LirBlock {
            phis: vec![Phi { result: 9, incoming: vec![(1, 11), (2, 12)] }].into(),
            ..block(
                5,
                vec![
                    of("cmp", Operation::Compare, None),
                    of("je", Operation::Branch, Some(6)),
                    of("jmp", Operation::Jump, Some(8)),
                ],
                &[6, 8],
            )
        };
        let last = LirBlock {
            phis: vec![Phi { result: 15, incoming: vec![(6, 14), (8, 9)] }].into(),
            ..block(7, vec![of("ret", Operation::Return, None)], &[])
        };
        let body = LirBody::new(
            "t",
            0,
            [
                block(
                    0,
                    vec![
                        of("cmp", Operation::Compare, None),
                        of("je", Operation::Branch, Some(1)),
                        of("jmp", Operation::Jump, Some(2)),
                    ],
                    &[1, 2],
                ),
                block(1, vec![make(11, 3), of("jmp", Operation::Jump, Some(5))], &[5]),
                block(2, vec![make(12, 4), of("jmp", Operation::Jump, Some(5))], &[5]),
                middle,
                block(6, vec![make(14, 8), of("jmp", Operation::Jump, Some(7))], &[7]),
                block(8, vec![of("jmp", Operation::Jump, Some(7))], &[7]),
                last,
            ]
            .into_iter()
            .map(Block::from)
            .collect(),
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = folded(&body);
        assert!(at(&done, 1).is_none() && at(&done, 6).is_none(), "both leaves fold");
        for join in &done.blocks {
            for phi in join.phis.iter() {
                for (from, _) in &phi.incoming {
                    let pred = at(&done, *from)
                        .unwrap_or_else(|| panic!("phi of {} reads from the gone block {from}", join.at));
                    assert!(pred.succ.contains(&join.at), "{from} is no predecessor of {}", join.at);
                }
            }
        }
    }
}
