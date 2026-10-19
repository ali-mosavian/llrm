//! `CoalescerPair`: a copy's two values, and which is merged into which.
//! Translated from RegisterCoalescer.cpp `CoalescerPair::setRegisters`, `flip`
//! and `isCoalescable` at d1106deb71cc, without sub-registers and physical
//! registers (see the module header, DEVIATE C1 and C2).
//!
//! # How it works
//! `set_registers` reads a plain move of one value to another of the same
//! width. `src_reg` is merged into `dst_reg`. `flip` swaps them.
//! `is_coalescable` says whether another instruction is a copy between the same
//! two values in either direction: after the join it is an identity.
//!
//! # Example
//! `v2 := mov v1` gives `src_reg` 1 and `dst_reg` 2; `v1 := mov v2` is
//! coalescable too.
//!
//! # Used by
//! `joinvals`, `register_coalescer`.

use crate::model::ir::{Held, Loc, Operation};
use crate::model::lir::Insn;

/// What the target says of two values: `getCommonSubClass(DstRC, SrcRC)` is
/// not null.
pub trait CoalescerEnv {
    fn can_share(
        &self,
        a: u32,
        b: u32,
    ) -> bool;

    /// A build for size: a copy of a constant is made again only where that
    /// leaves no second copy of the constant (DEVIATE C10).
    fn tuned_for_size(&self) -> bool {
        false
    }

    /// `TII->isAsCheapAsAMove(DefMI) && TII->isReMaterializable(DefMI) &&
    /// DefMI->isSafeToMove()` with one def: a move of an immediate that reads
    /// and writes nothing else.
    fn rematerializable(
        &self,
        insn: &Insn,
    ) -> bool {
        let Some(what) = &insn.what else { return false };
        what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && matches!((what.dests.as_slice(), what.sources.as_slice()), ([Loc::Held(_)], [Loc::Imm(_)]))
            && insn.defines.len() == 1
            && insn.uses.is_empty()
            && insn.clobbers.is_empty()
            && insn.requires.is_empty()
            && insn.delivers.is_empty()
            && insn.spread.is_empty()
            && insn.group.is_none()
            && insn.call.is_none()
            && !insn.volatile
    }
}

/// The (destination, source) of a plain move between values of one width
/// (`isMoveInstr` for a full copy).
pub fn full_copy(insn: &Insn) -> Option<(Held, Held)> {
    let what = insn.what.as_ref()?;
    if what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1 {
        return None;
    }
    match (&what.dests[0], &what.sources[0]) {
        (Loc::Held(dst), Loc::Held(src)) if dst.width == src.width => Some((*dst, *src)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct CoalescerPair {
    src_reg: u32,
    dst_reg: u32,
    flipped: bool,
}

impl CoalescerPair {
    /// `setRegisters`: the pair of a copy, or None where it is not a move
    /// the coalescer may join.
    pub fn set_registers(
        copy: &Insn,
        env: &dyn CoalescerEnv,
    ) -> Option<Self> {
        let (dst, src) = full_copy(copy)?;
        if !env.can_share(src.value, dst.value) {
            return None;
        }
        Some(Self { src_reg: src.value, dst_reg: dst.value, flipped: false })
    }

    pub fn src_reg(&self) -> u32 {
        self.src_reg
    }

    pub fn dst_reg(&self) -> u32 {
        self.dst_reg
    }

    pub fn is_flipped(&self) -> bool {
        self.flipped
    }

    /// `flip`: the other value is merged into this one.
    pub fn flip(&mut self) {
        std::mem::swap(&mut self.src_reg, &mut self.dst_reg);
        self.flipped = !self.flipped;
    }

    /// `isCoalescable`: whether `insn` is a copy between this pair's values.
    pub fn is_coalescable(
        &self,
        insn: &Insn,
    ) -> bool {
        let Some((dst, src)) = full_copy(insn) else { return false };
        let (mut src, mut dst) = (src.value, dst.value);
        // Find the value that is SrcReg.
        if dst == self.src_reg {
            std::mem::swap(&mut src, &mut dst);
        } else if src != self.src_reg {
            return false;
        }
        // Now check that Dst matches DstReg.
        dst == self.dst_reg
    }
}
