//! Copy propagation over the x87 registers FP0..FP6: LLVM's
//! `MachineCopyPropagation` for the RFP class.
//!
//! # How it works
//! Registers are chosen and the stackifier has not run. Each block is walked
//! forward with the copies `FPd := FPs` still valid (neither side written
//! since): a read of `FPd` reads `FPs`, and a copy that repeats a valid one is
//! dropped. A backward walk then drops a copy whose destination nothing reads
//! before it is written or the block ends, and that is not live out.
//!
//! ```text
//!   fp3 := fp1                  fp3 := fp1      <- dead when fp3 is not
//!   fstp [m], fp3        =>     fstp [m], fp1      read again
//! ```
//!
//! A removed copy becomes an anchor (`lir::anchor`), as in `machinedce`.
//!
//! DEVIATE 1: `machinecp` tracks the byte lanes of the general registers from
//! the decoded effects of an instruction. An x87 operation here has no
//! encoding yet, so its reads and writes are its operands; this is a second
//! tracker of seven units. Exit: the stackifier follows registers and the
//! tracker is `machinecp`'s with the RFP class as more units.
//!
//! DEVIATE 2: a copy in a group (a phi's parallel copy) is neither rewritten
//! nor removed; its writes end the copies they touch.
//!
//! # Runs
//! At the start of `FloatAlloc`, under `LLRM_GREEDY_RFP`.
//!
//! # Example
//! The listing above: the store reads `fp1`, the copy is dead and gone.
//!
//! # Needs
//! [`crate::backend::fpregs`] for the registers live out of a block.
//!
//! # See also
//! LLVM [`MachineCopyPropagation`]
//! (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp)

use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::allocate::RFP;
use crate::backend::floatregions::clears_stack;
use crate::backend::fpregs::{live_out, slot_of, touched};
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBody};

const SLOTS: usize = RFP.len();

/// `one` as a copy between two x87 registers: `(destination, source)`.
fn copy_of(
    regs: Regs,
    one: &Insn,
) -> Option<(usize, usize)> {
    let what = one.what.as_ref()?;
    let (Operation::Move, Some("mov"), [dest], [source]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    else {
        return None;
    };
    let plain = one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.group.is_none();
    let (dest, source) = (slot_of(regs, dest)?, slot_of(regs, source)?);
    (plain && dest != source).then_some((dest, source))
}

/// The copies valid at a point: for each register, the one it is a copy of.
#[derive(Clone, Copy, Default)]
struct Copies([Option<u8>; SLOTS]);

impl Copies {
    /// `slot` is written: it is no copy, and no copy of it holds.
    fn written(
        &mut self,
        slot: usize,
    ) {
        self.0[slot] = None;
        for held in &mut self.0 {
            if *held == Some(slot as u8) {
                *held = None;
            }
        }
    }
}

/// `what` with each x87 source read through the copy that holds.
fn forwarded(
    regs: Regs,
    what: &Semantics,
    copies: &Copies,
) -> Option<Semantics> {
    let mut changed = false;
    let sources = what
        .sources
        .iter()
        .map(|place| match slot_of(regs, place).and_then(|at| copies.0[at]) {
            Some(from) => {
                changed = true;
                Loc::Reg(Reg { register: RFP[from as usize], width: 10 })
            }
            None => place.clone(),
        })
        .collect();
    changed.then(|| Semantics { sources, ..what.clone() })
}

/// One block's instructions with the copies propagated forward.
fn forward(
    regs: Regs,
    insns: &[Arc<Insn>],
) -> Vec<Arc<Insn>> {
    let mut copies = Copies::default();
    insns
        .iter()
        .map(|one| {
            if clears_stack(regs, one) || !one.clobbers.is_empty() {
                copies = Copies::default();
                return Arc::clone(one);
            }
            let Some(what) = &one.what else {
                return Arc::clone(one);
            };
            if one.group.is_some() {
                let (_, defs) = touched(regs, what);
                (0..SLOTS).filter(|at| defs >> at & 1 == 1).for_each(|at| copies.written(at));
                return Arc::clone(one);
            }
            let mut made = Arc::clone(one);
            if let Some(what) = forwarded(regs, what, &copies) {
                let mut changed = (**one).clone();
                changed.what = Some(what);
                made = Arc::new(changed);
            }
            if let Some((dest, source)) = copy_of(regs, &made) {
                if copies.0[dest] == Some(source as u8) {
                    return lir::anchor(made);
                }
                copies.written(dest);
                copies.0[dest] = Some(source as u8);
                return made;
            }
            let (_, defs) = touched(regs, made.what.as_ref().expect("semantics"));
            (0..SLOTS).filter(|at| defs >> at & 1 == 1).for_each(|at| copies.written(at));
            made
        })
        .collect()
}

/// `insns` without the copies nothing reads, given the registers live out.
fn without_dead_copies(
    regs: Regs,
    insns: Vec<Arc<Insn>>,
    live_out: u8,
) -> Vec<Arc<Insn>> {
    let mut live = live_out;
    let mut kept: Vec<Arc<Insn>> = insns
        .into_iter()
        .rev()
        .map(|one| {
            let Some(what) = &one.what else {
                return one;
            };
            if clears_stack(regs, &one) {
                // Nothing is on the stack across it.
                live = 0;
            }
            if let Some((dest, _)) = copy_of(regs, &one)
                && live >> dest & 1 == 0
            {
                return lir::anchor(one);
            }
            let (uses, defs) = touched(regs, what);
            live = live & !defs | uses;
            one
        })
        .collect();
    kept.reverse();
    kept
}

/// `body` with the x87 register copies propagated, or `None` if none changed.
pub(super) fn propagated(body: &LirBody) -> Option<LirBody> {
    let regs = body.regs();
    if !body.insns().iter().any(|one| copy_of(regs, one).is_some()) {
        return None;
    }
    let out = live_out(body, regs);
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let forwarded = forward(regs, &block.insns);
            block.with_insns(without_dead_copies(regs, forwarded, out[&block.at]))
        })
        .collect();
    Some(body.with_blocks(blocks))
}

#[cfg(test)]
#[path = "fpcopyprop_tests.rs"]
mod tests;
