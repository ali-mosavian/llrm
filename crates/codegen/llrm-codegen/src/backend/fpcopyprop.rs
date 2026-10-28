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
//! A float rounded through a cell and stored on, `fstp [t]; fld [t]; fstp [h]`
//! with both cells the same width, rounds once into `h` as the first store
//! does: the first store is dropped and `h` takes its source, where nothing
//! reads `t` before a store rewrites it. pow_stack_depth rounded each of its
//! six doubles twice (m16 -Os 1644 B against main's 1462).
//!
//! Stored back into `t`, the reload and second store go and the first stays.
//!
//! ```text
//!   [t] := fstp fp0             [h] := fstp fp0
//!   fp1 := fld [t]       =>
//!   [h] := fstp fp1
//! ```
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
//! At the start of `FloatAlloc`, under RFP.
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

use crate::backend::floatregions::clears_stack;
use crate::backend::fpregs::{live_out, slot_of, touched};
use crate::model::ir::{Loc, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBody};

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
#[derive(Clone)]
struct Copies(Vec<Option<u8>>);

impl Copies {
    fn new(regs: Regs) -> Self {
        Self(vec![None; regs.rfp_registers().len()])
    }
}

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
                Loc::Reg(Reg { register: regs.rfp_registers()[from as usize], width: 10 })
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
    let mut copies = Copies::new(regs);
    insns
        .iter()
        .map(|one| {
            if clears_stack(regs, one) || !one.clobbers.is_empty() {
                copies = Copies::new(regs);
                return Arc::clone(one);
            }
            let Some(what) = &one.what else {
                return Arc::clone(one);
            };
            if one.group.is_some() {
                let (_, defs) = touched(regs, what);
                (0..copies.0.len()).filter(|at| defs >> at & 1 == 1).for_each(|at| copies.written(at));
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
            (0..copies.0.len()).filter(|at| defs >> at & 1 == 1).for_each(|at| copies.written(at));
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

/// The frame bytes `place` names, where it is a plain frame cell.
fn frame_bytes(place: &Loc) -> Option<(i64, i64)> {
    let Loc::Mem(mem) = place else { return None };
    let addr = mem.addr?;
    (addr.space == Space::Frame && mem.base.is_none() && mem.index.is_none())
        .then(|| (addr.disp + mem.offset, addr.disp + mem.offset + i64::from(mem.width)))
}

/// `one` as a float store of a register to a frame cell: `(cell, register)`.
fn stored(
    regs: Regs,
    one: &Insn,
) -> Option<((i64, i64), usize)> {
    let what = one.what.as_ref()?;
    let (Operation::FloatStore, Some("fstp"), [cell], [source]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    else {
        return None;
    };
    Some((frame_bytes(cell)?, slot_of(regs, source)?))
}

/// `one` as a float load of a frame cell into a register: `(cell, register)`.
fn loaded(
    regs: Regs,
    one: &Insn,
) -> Option<((i64, i64), usize)> {
    let what = one.what.as_ref()?;
    let (Operation::FloatLoad, Some("fld"), [dest], [cell]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    else {
        return None;
    };
    Some((frame_bytes(cell)?, slot_of(regs, dest)?))
}

/// Whether the bytes `cell` are rewritten before anything in `rest` reads
/// them, within the block.
fn rewritten(
    rest: &[Arc<Insn>],
    cell: (i64, i64),
) -> bool {
    for one in rest {
        let Some(what) = &one.what else { continue };
        let overlaps = |place: &Loc| frame_bytes(place).is_some_and(|(low, high)| low < cell.1 && cell.0 < high);
        if what.sources.iter().any(overlaps) {
            return false;
        }
        if what.dests.iter().any(|place| frame_bytes(place).is_some_and(|(low, high)| low <= cell.0 && cell.1 <= high))
        {
            return true;
        }
        if what.dests.iter().any(overlaps) {
            return false;
        }
    }
    false
}

/// `insns` with each rounding through a dead cell stored on once.
fn without_double_roundings(
    regs: Regs,
    insns: &[Arc<Insn>],
) -> Option<Vec<Arc<Insn>>> {
    let mut made: Vec<Arc<Insn>> = insns.to_vec();
    let mut changed = false;
    for at in 0..insns.len().saturating_sub(2) {
        let (Some((cell, held)), Some((again, reread)), Some((home, last))) =
            (stored(regs, &insns[at]), loaded(regs, &insns[at + 1]), stored(regs, &insns[at + 2]))
        else {
            continue;
        };
        let width = |one: &Insn, source: bool| -> Option<u32> {
            let what = one.what.as_ref()?;
            match if source { &what.sources[0] } else { &what.dests[0] } {
                Loc::Mem(mem) => Some(mem.width),
                _ => None,
            }
        };
        if cell != again
            || reread != last
            || width(&insns[at], false) != width(&insns[at + 2], false)
            || width(&insns[at], false) != width(&insns[at + 1], true)
            || insns[at + 1].volatile()
        {
            continue;
        }
        // Back into the same cell, the reload and second store change nothing.
        if home == cell {
            made[at + 1] = lir::anchor(Arc::clone(&insns[at + 1]));
            made[at + 2] = lir::anchor(Arc::clone(&insns[at + 2]));
            changed = true;
            continue;
        }
        if !rewritten(&insns[at + 3..], cell) {
            continue;
        }
        let mut fused = (*insns[at + 2]).clone();
        let what = fused.what.as_mut().expect("a store");
        what.sources = vec![Loc::Reg(Reg { register: regs.rfp_registers()[held], width: 10 })];
        made[at + 2] = Arc::new(fused);
        made[at] = lir::anchor(Arc::clone(&insns[at]));
        made[at + 1] = lir::anchor(Arc::clone(&insns[at + 1]));
        changed = true;
    }
    changed.then_some(made)
}

/// `body` with the x87 register copies propagated and the double roundings
/// fused, or `None` if none changed.
pub(super) fn propagated(body: &LirBody) -> Option<LirBody> {
    let regs = body.regs();
    let copies = body.iter_insns().any(|one| copy_of(regs, one).is_some());
    let out = if copies { Some(live_out(body, regs)) } else { None };
    let mut changed = copies;
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns = if let Some(out) = &out {
                let forwarded = forward(regs, &block.insns);
                without_dead_copies(regs, forwarded, out[&block.at])
            } else {
                block.insns.to_vec()
            };
            if let Some(fused) = without_double_roundings(regs, &insns) {
                insns = fused;
                changed = true;
            }
            block.with_insns(insns)
        })
        .collect();
    changed.then(|| body.with_blocks(blocks))
}

#[cfg(test)]
#[path = "fpcopyprop_tests.rs"]
mod tests;
