//! The x87 registers FP0..FP6 as the values the stackifier tracks.
//!
//! # How it works
//! Under RFP the allocator leaves each x87 operand in `FP0`..`FP6`
//! (`Loc::Reg`), as `X86FloatingPoint` finds it. The stackifier's identity is a
//! value number, so each register is one value for the whole function: every
//! write and read of `FP1` names the same number, and the values at both ends
//! of an edge agree.
//!
//! DEVIATE 1: LLVM reads the registers; the stackifier's identity is a value
//! number (`fpstack` DEVIATE 1), here the register's own.
//!
//! # Runs
//! At the start of `fpstack::converted`; a body with no x87 register operand
//! is returned as it is.
//!
//! # Example
//! `fp1 := fld [a]; fp1 := fmul fp1, fp0` becomes
//! `b1 := fld [a]; b1 := fmul b1, b0`, `b1` being `fp1`'s value.
//!
//! # Needs
//! [`llrm_lir::registers::Regs::rfp_registers`].

use std::collections::VecDeque;

use llrm_lir::registers::Regs;

use crate::model::ir::{Held, Loc, Reg, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{HashSet, IndexMap};

/// Which of `FP0`..`FP6` `place` is.
pub(super) fn slot_of(
    regs: Regs,
    place: &Loc,
) -> Option<usize> {
    let Loc::Reg(Reg { register, width: 10 }) = place else {
        return None;
    };
    regs.rfp_registers().iter().position(|one| one == register)
}

/// The slots `what` reads and the slots it writes, as bit masks.
pub(super) fn touched(
    regs: Regs,
    what: &Semantics,
) -> (u8, u8) {
    let mask =
        |places: &[Loc]| places.iter().filter_map(|place| slot_of(regs, place)).fold(0u8, |bits, at| bits | 1 << at);
    (mask(&what.sources), mask(&what.dests))
}

/// For each block the slots live out of it: read before written somewhere
/// ahead.
pub(super) fn live_out(
    body: &LirBody,
    regs: Regs,
) -> IndexMap<i64, u8> {
    let mut gen_kill: IndexMap<i64, (u8, u8)> = IndexMap::default();
    for block in &body.blocks {
        let (mut read, mut written) = (0u8, 0u8);
        for what in block.insns.iter().filter_map(|one| one.what.as_ref()) {
            let (uses, defs) = touched(regs, what);
            read |= uses & !written;
            written |= defs;
        }
        gen_kill.insert(block.at, (read, written));
    }
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for successor in block.succ.iter().filter(|one| gen_kill.contains_key(*one)) {
            preds.entry(*successor).or_default().push(block.at);
        }
    }
    let mut out: IndexMap<i64, u8> = body.blocks.iter().map(|block| (block.at, 0u8)).collect();
    let mut work: VecDeque<i64> = body.blocks.iter().map(|block| block.at).collect();
    let mut queued: HashSet<i64> = work.iter().copied().collect();
    while let Some(at) = work.pop_front() {
        queued.remove(&at);
        let (read, written) = gen_kill[&at];
        let live_in = read | (out[&at] & !written);
        for pred in preds.get(&at).into_iter().flatten() {
            let grown = out[pred] | live_in;
            if grown != out[pred] {
                out.insert(*pred, grown);
                if queued.insert(*pred) {
                    work.push_back(*pred);
                }
            }
        }
    }
    out
}

/// `body` with its x87 register operands as values, or `None` if it has none.
pub(super) fn valued(body: &LirBody) -> Option<LirBody> {
    let regs = body.regs();
    let named = |one: &Insn| {
        one.what
            .as_ref()
            .is_some_and(|what| what.sources.iter().chain(&what.dests).any(|place| slot_of(regs, place).is_some()))
    };
    if !body.iter_insns().any(|one| named(one)) {
        return None;
    }
    let fresh = body
        .blocks
        .iter()
        .flat_map(|block| {
            let phis = block
                .phis
                .iter()
                .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)));
            block.insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied()).chain(phis)
        })
        .max()
        .map_or(0, |most| most + 1);
    let border: Vec<u32> = (0..regs.rfp_registers().len() as u32).map(|at| fresh + at).collect();
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| {
                    if !named(one) {
                        return std::sync::Arc::clone(one);
                    }
                    let what = one.what.as_ref().expect("named");
                    let mut made = (**one).clone();
                    let sources: Vec<Loc> = what
                        .sources
                        .iter()
                        .map(|place| match slot_of(regs, place) {
                            Some(at) => {
                                let value = border[at];
                                made.uses.push(value);
                                made.widths.push((value, 10));
                                Loc::Held(Held { value, width: 10 })
                            }
                            None => place.clone(),
                        })
                        .collect();
                    let dests: Vec<Loc> = what
                        .dests
                        .iter()
                        .map(|place| match slot_of(regs, place) {
                            Some(at) => {
                                let value = border[at];
                                made.defines.push(value);
                                made.widths.push((value, 10));
                                Loc::Held(Held { value, width: 10 })
                            }
                            None => place.clone(),
                        })
                        .collect();
                    made.what = Some(Semantics { sources, dests, ..what.clone() });
                    std::sync::Arc::new(made)
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    Some(body.with_blocks(blocks))
}

#[cfg(test)]
#[path = "fpregs_tests.rs"]
mod tests;
