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

use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
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

    /// `TRI->shouldCoalesce(CopyMI, SrcRC, DstRC, NewRC, LIS)`: whether the
    /// join of `src` and `dst` across `copy` goes ahead.
    fn should_coalesce(
        &self,
        _lis: &LiveIntervals,
        _copy: InstrKey,
        _src: u32,
        _dst: u32,
    ) -> bool {
        true
    }

    /// `MRI->setRegClass(CP.getDstReg(), CP.getNewRC())`: `src` was merged into
    /// `dst`, which may now take only the registers both could.
    fn narrow(
        &self,
        _dst: u32,
        _src: u32,
    ) {
    }

    /// A value a join must leave standing, whatever the interval sizes: a
    /// value pinned to a register of its own, whose range the allocator keeps
    /// other values out of. Merged into a virtual value it would be an
    /// ordinary value with one register to choose from, and lose to the values
    /// that took the register first.
    fn keeps(
        &self,
        _value: u32,
    ) -> bool {
        false
    }

    /// The pinned values on registers that overlap `value`'s (a pair and its
    /// bytes): `value` holds the units they hold, so a join that makes it
    /// longer must keep clear of their ranges (`checkRegUnitInterference`).
    fn aliases(
        &self,
        _value: u32,
    ) -> Vec<u32> {
        Vec::new()
    }

    /// A build for size: a copy of a constant is made again only where that
    /// leaves no second copy of the constant (DEVIATE C10).
    fn tuned_for_size(&self) -> bool {
        false
    }

    /// `MachineInstr::isTerminator`: the instruction ends its block.
    fn is_terminator(
        &self,
        insn: &Insn,
    ) -> bool {
        insn.what
            .as_ref()
            .is_some_and(|what| matches!(
                what.op,
                Operation::Jump | Operation::Branch | Operation::Return
            ))
    }

    /// `TII->isAsCheapAsAMove(DefMI) && TII->isReMaterializable(DefMI) &&
    /// DefMI->isSafeToMove()` with one def: a move of an immediate, or an
    /// address (`LEA32r`, of registers too), as `isTriviallyReMaterializable`
    /// says; the caller asks whether the registers it reads still hold
    /// (`allUsesAvailableAt`).
    fn rematerializable(
        &self,
        insn: &Insn,
    ) -> bool {
        insn.spread.is_empty()
            && insn.call.is_none()
            && crate::greedy::remat::is_trivially_re_materializable(insn, false)
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
