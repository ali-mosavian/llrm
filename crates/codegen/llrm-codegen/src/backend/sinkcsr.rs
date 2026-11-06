//! A copy into a callee-saved register, sunk out of a branching block.
//!
//! # How it works
//! A branching block outside every loop ends in a conditional branch to two
//! blocks. A `mov csr, reg|imm` before it, whose register only one of the two
//! reads, is moved to that one: to its start where the block has no other
//! predecessor, else to a new block on the edge. The block that returns early
//! then no longer touches the register, and the frame and the save of it can
//! wait for the path that needs them (`shrinkwrap`).
//!
//! ```text
//!   b0: mov esi, 1             b0: test ecx, ecx
//!       test ecx, ecx              je b9
//!       je b9               ==>  b1: mov esi, 1 ...
//!   b1: ...                  b9 (esi dead on this edge)
//! ```
//!
//! # Deviation
//! DEVIATE REC-5B (owner m32, exit 2026-11-20). The pin never moves such a copy:
//! `ShrinkWrap` only places the save, and its entry keeps the copy of an
//! argument into a callee-saved register, so `rectwo`'s base case pays three
//! `push`/`pop`. gcc does move them (`prepare_shrink_wrap`, gcc 15.2
//! `shrink-wrap.cc`; the idea only, this is written from the rule above).
//! `Target::sink_csr_copies` turns it on (m32).
//!
//! # Runs
//! After `ParallelCopy`, before `Prologue`.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::loops;
use crate::backend::liveness;
use crate::backend::machinedce;
use crate::backend::peephole::{_lanes, Lanes};
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashSet;

pub struct SinkCsrCopies {
    pub enabled: bool,
    /// The callee-saved registers, each by its full register.
    pub saved: BTreeSet<RegId>,
}

impl LIRTransform for SinkCsrCopies {
    fn class_name(&self) -> &'static str {
        "SinkCsrCopies"
    }

    fn name(&self) -> &str {
        "sinkcsr"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(if self.enabled && !self.saved.is_empty() { sunk(body, &self.saved) } else { body })
    }
}

/// `body` with every copy into a callee-saved register that one successor of
/// its branching block alone needs moved there.
pub fn sunk(
    mut body: LirBody,
    saved: &BTreeSet<RegId>,
) -> LirBody {
    // Each move can make another possible; the blocks are few and the copies
    // fewer, and every round removes one.
    for _ in 0..body.blocks.len().max(1) {
        match sunk_once(&body, saved) {
            Some(next) => body = next,
            None => break,
        }
    }
    body
}

fn real_branch(insn: &Insn) -> Option<(Operation, Option<i64>)> {
    insn.what.as_ref().map(|what| (what.op, what.target))
}

/// One copy moved, or None where none can be.
fn sunk_once(
    body: &LirBody,
    saved: &BTreeSet<RegId>,
) -> Option<LirBody> {
    let regs = body.regs();
    let bits = body.bits;
    let mut csr = Lanes::new();
    for &whole in saved {
        csr = csr.or(&_lanes(regs, whole));
    }
    let (live_into, _, _) = liveness::live_into(body);
    let in_loop: HashSet<i64> =
        loops::loops(&body.blocks, Some(body.entry)).iter().flat_map(|one| one.body.to_set()).collect();
    let mut preds: crate::support::hash::IndexMap<i64, Vec<i64>> = Default::default();
    for block in &body.blocks {
        for to in crate::model::lir::distinct(&block.succ) {
            preds.entry(to).or_default().push(block.at);
        }
    }
    for (place, block) in body.blocks.iter().enumerate() {
        if in_loop.contains(&block.at) || block.succ.len() != 2 || block.succ[0] == block.succ[1] {
            continue;
        }
        // The terminators: from the first branch or jump on.
        let Some(first) = block
            .insns
            .iter()
            .position(|one| matches!(real_branch(one), Some((Operation::Branch | Operation::Jump, _))))
        else {
            continue;
        };
        let (mut reads, mut writes) = (Lanes::new(), Lanes::new());
        let mut known = true;
        for one in &block.insns[first..] {
            match liveness::effect(regs, bits, one) {
                Some(effect) => {
                    reads = reads.or(&effect.all_reads());
                    writes = writes.or(&effect.writes);
                }
                None => known = false,
            }
        }
        if !known {
            continue;
        }
        for index in (0..first).rev() {
            let one = &block.insns[index];
            let Some(effect) = liveness::effect(regs, bits, one) else { break };
            if let Some(dest) = candidate(one, &csr, regs, &effect, &reads, &writes) {
                let into = |to: i64| !live_into[&to].is_disjoint(&dest);
                let [first_to, second_to] = [block.succ[0], block.succ[1]];
                let to = match (into(first_to), into(second_to)) {
                    (true, false) => first_to,
                    (false, true) => second_to,
                    _ => {
                        reads = reads.or(&effect.all_reads());
                        writes = writes.or(&effect.writes);
                        continue;
                    }
                };
                return Some(moved(body, place, index, to, preds.get(&to).map_or(0, Vec::len)));
            }
            reads = reads.or(&effect.all_reads());
            writes = writes.or(&effect.writes);
        }
    }
    None
}

/// The lanes `one` writes if it is a flagless, pure copy of a register or a
/// constant into a callee-saved register that nothing after it in the block
/// reads or writes, and whose source nothing after it writes.
fn candidate(
    one: &Insn,
    csr: &Lanes,
    regs: llrm_lir::registers::Regs,
    effect: &liveness::Effect,
    reads_later: &Lanes,
    writes_later: &Lanes,
) -> Option<Lanes> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Move || !machinedce::pure(regs, one) {
        return None;
    }
    let ([Loc::Reg(dest)], [source]) = (&what.dests[..], &what.sources[..]) else { return None };
    if !matches!(source, Loc::Reg(_) | Loc::Imm(_)) {
        return None;
    }
    let lanes = _lanes(regs, dest.register);
    if lanes.is_empty() || !lanes.is_subset(csr) || effect.writes != lanes {
        return None;
    }
    if !lanes.is_disjoint(reads_later) || !lanes.is_disjoint(writes_later) {
        return None;
    }
    if !effect.reads.is_disjoint(writes_later) {
        return None;
    }
    Some(lanes)
}

/// `body` with the instruction at `index` of the block at `place` moved to
/// `to`: to its start where `preds` is one, else to a new block on the edge.
fn moved(
    body: &LirBody,
    place: usize,
    index: usize,
    to: i64,
    preds: usize,
) -> LirBody {
    let mut blocks: Vec<Block> = body.blocks.clone();
    let insn = Arc::clone(&blocks[place].insns[index]);
    let mut from = LirBlock::clone(&blocks[place]);
    let mut kept: Vec<Arc<Insn>> = from.insns.to_vec();
    kept.remove(index);
    if preds <= 1 {
        from.insns = kept.into();
        blocks[place] = from.into();
        let target = blocks.iter().position(|one| one.at == to).expect("a successor is a block");
        let mut into = LirBlock::clone(&blocks[target]);
        let mut insns: Vec<Arc<Insn>> = into.insns.to_vec();
        insns.insert(0, insn);
        into.insns = insns.into();
        blocks[target] = into.into();
    } else {
        let fresh = body.blocks.iter().map(|one| one.at).max().unwrap_or(0) + 1;
        let retarget = |one: &Arc<Insn>| -> Arc<Insn> {
            match one.what.as_ref() {
                Some(what) if what.target == Some(to) && matches!(what.op, Operation::Branch | Operation::Jump) => {
                    let mut changed = (**one).clone();
                    changed.what = Some(crate::model::ir::Semantics { target: Some(fresh), ..what.clone() });
                    Arc::new(changed)
                }
                _ => Arc::clone(one),
            }
        };
        let kept: Vec<Arc<Insn>> = kept.iter().map(retarget).collect();
        from.insns = kept.into();
        from.succ = from.succ.iter().map(|&at| if at == to { fresh } else { at }).collect::<Vec<_>>().into();
        let jump = Arc::new(Insn::new(
            fresh,
            None,
            Some(crate::model::ir::Semantics {
                name: Some("jmp".into()),
                target: Some(to),
                ..crate::model::ir::Semantics::new(Operation::Jump)
            }),
            Vec::new(),
            Vec::new(),
        ));
        let edge = LirBlock {
            at: fresh,
            insns: vec![insn, jump].into(),
            succ: vec![to].into(),
            phis: Vec::new().into(),
            cold: false,
            align: 0,
            max_skip: 0,
        };
        blocks[place] = from.into();
        blocks.insert(place + 1, edge.into());
    }
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Imm, Reg, Semantics};
    use crate::support::hash::IndexMap;

    fn insn(
        at: i64,
        what: Semantics,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![]))
    }

    fn reg(register: RegId) -> Loc {
        Loc::Reg(crate::model::ir::Reg { register, width: 4 })
    }

    fn load(
        at: i64,
        register: RegId,
        value: i64,
    ) -> Arc<Insn> {
        insn(
            at,
            Semantics {
                name: Some("mov".into()),
                dests: vec![reg(register)],
                sources: vec![Loc::Imm(Imm { value, width: 4, address: None })],
                ..Semantics::new(Operation::Move)
            },
        )
    }

    fn copy(
        at: i64,
        to: RegId,
        from: RegId,
    ) -> Arc<Insn> {
        insn(
            at,
            Semantics {
                name: Some("mov".into()),
                dests: vec![reg(to)],
                sources: vec![reg(from)],
                ..Semantics::new(Operation::Move)
            },
        )
    }

    fn je(
        at: i64,
        target: i64,
    ) -> Arc<Insn> {
        insn(at, Semantics { name: Some("je".into()), target: Some(target), ..Semantics::new(Operation::Branch) })
    }

    fn jmp(
        at: i64,
        target: i64,
    ) -> Arc<Insn> {
        insn(at, Semantics { name: Some("jmp".into()), target: Some(target), ..Semantics::new(Operation::Jump) })
    }

    /// A return that reads nothing but what it requires: the lowered one.
    fn ret(at: i64) -> Arc<Insn> {
        let mut one = Insn::new(
            at,
            Some((at, at + 2)),
            Some(Semantics { name: Some("ret".into()), ..Semantics::new(Operation::Return) }),
            vec![],
            vec![],
        );
        one.reads_complete = true;
        Arc::new(one)
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        let blocks = crate::model::lir::shared(blocks);
        LirBody::new("sink", blocks[0].at, blocks, IndexMap::default(), IndexMap::default())
    }

    fn saved() -> BTreeSet<RegId> {
        BTreeSet::from([RegId::EBX])
    }

    fn names(
        block: &LirBlock,
        register: RegId,
    ) -> bool {
        block.insns.iter().filter_map(|one| one.what.as_ref()).any(|what| {
            what.op == Operation::Move && matches!(&what.dests[..], [Loc::Reg(Reg { register: r, .. })] if *r == register)
        })
    }

    fn at(
        body: &LirBody,
        at: i64,
    ) -> &LirBlock {
        body.blocks.iter().find(|one| one.at == at).expect("a block")
    }

    /// rectwo -O2: `mov ebx, 1` before the early-exit branch kept the save of
    /// ebx in the entry (three `push`/`pop` on the base case). With the arm
    /// that reads ebx behind its own predecessor, the copy goes to its start.
    #[test]
    fn a_copy_only_one_arm_reads_leaves_the_branching_block() {
        let done = sunk(
            body(vec![
                block(0, vec![load(0, RegId::EBX, 1), je(2, 20), jmp(4, 10)], vec![20, 10]),
                block(10, vec![copy(10, RegId::EAX, RegId::EBX), ret(12)], vec![]),
                block(20, vec![ret(20)], vec![]),
            ]),
            &saved(),
        );
        assert!(!names(at(&done, 0), RegId::EBX), "the entry still makes ebx");
        assert!(names(at(&done, 10), RegId::EBX), "the arm that reads it makes it");
        assert!(!names(at(&done, 20), RegId::EBX));
    }

    /// Both arms read it: nothing moves.
    #[test]
    fn a_copy_both_arms_read_stays() {
        let before = body(vec![
            block(0, vec![load(0, RegId::EBX, 1), je(2, 20), jmp(4, 10)], vec![20, 10]),
            block(10, vec![copy(10, RegId::EAX, RegId::EBX), ret(12)], vec![]),
            block(20, vec![copy(20, RegId::EAX, RegId::EBX), ret(22)], vec![]),
        ]);
        let done = sunk(before.clone(), &saved());
        assert!(names(at(&done, 0), RegId::EBX));
        assert_eq!(done.blocks.len(), before.blocks.len());
    }

    /// The arm that reads it has another predecessor: the copy goes to a new
    /// block on the edge (rectwo's exit is also the loop's).
    #[test]
    fn a_copy_for_an_arm_with_two_predecessors_gets_an_edge_block() {
        let done = sunk(
            body(vec![
                block(0, vec![load(0, RegId::EBX, 1), je(2, 20), jmp(4, 10)], vec![20, 10]),
                block(10, vec![load(10, RegId::EBX, 0), jmp(12, 20)], vec![20]),
                block(20, vec![copy(20, RegId::EAX, RegId::EBX), ret(22)], vec![]),
            ]),
            &saved(),
        );
        assert!(!names(at(&done, 0), RegId::EBX));
        let edge = done.blocks.iter().find(|one| one.at != 0 && one.at != 10 && one.at != 20).expect("an edge block");
        assert!(names(edge, RegId::EBX) && edge.succ.to_vec() == vec![20]);
        assert!(at(&done, 0).succ.contains(&edge.at) && !at(&done, 0).succ.contains(&20));
    }

    /// A register the target does not save is not moved: it costs no frame.
    #[test]
    fn a_copy_into_a_register_the_callee_does_not_save_stays() {
        let done = sunk(
            body(vec![
                block(0, vec![load(0, RegId::ECX, 1), je(2, 20), jmp(4, 10)], vec![20, 10]),
                block(10, vec![copy(10, RegId::EAX, RegId::ECX), ret(12)], vec![]),
                block(20, vec![ret(20)], vec![]),
            ]),
            &saved(),
        );
        assert!(names(at(&done, 0), RegId::ECX));
    }
}
