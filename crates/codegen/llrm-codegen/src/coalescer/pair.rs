//! `CoalescerPair`: a copy's two values, and which is merged into which.
//! Translated from RegisterCoalescer.cpp `CoalescerPair::setRegisters`, `flip`
//! and `isCoalescable` at d1106deb71cc, without physical registers (see the
//! module header, DEVIATE C2).
//!
//! # How it works
//! `set_registers` reads a plain move of one value to another. Of one width,
//! `src_reg` is merged into `dst_reg` and `flip` swaps them. Of two widths
//! (`EXTRACT_SUBREG`'s truncating copy, an any-extend), the pair is partial as
//! the pin's is for a sub-register copy: `src_reg` is the narrow value, merged
//! into the wide `dst_reg` ("prefer SrcReg to be a sub-register of DstReg"),
//! and the pair is never flipped.
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

    /// `getMatchingSuperRegClass(WideRC, NarrowRC, SubIdx)` is not null: the
    /// registers `wide` may take have, as their low part, registers `narrow`
    /// may take. A target whose registers have no such views answers no.
    fn can_share_sub(
        &self,
        _narrow: u32,
        _wide: u32,
    ) -> bool {
        false
    }

    /// `narrow` for a partial pair: `narrow_value` was merged into `wide`,
    /// which may now take only the registers whose low part `narrow_value`
    /// could.
    fn narrow_sub(
        &self,
        _wide: u32,
        _narrow_value: u32,
    ) {
    }

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

    /// `JoinGlobalCopies` (`STI.enableJoinGlobalCopies()`, which is
    /// `enableMachineScheduler()`: x86 and AArch64 true, WebAssembly and M68k
    /// false): local copies wait for their loop depth's pass and terminal
    /// copies go last. False: one top-down list per block, terminals last.
    fn joins_global_copies(&self) -> bool {
        true
    }

    /// DEVIATE C13: whether a copy between widths (`CP.isPartial()`) is joined.
    /// The pin always joins it (`TRI->shouldCoalesce` is the only veto,
    /// RegisterCoalescer.cpp#L2080); a target whose profitability for these
    /// joins is not yet known says false. Exit: m68k profitability rule
    /// (11-08).
    fn joins_sub_registers(&self) -> bool {
        true
    }

    /// `canJoinPhys` (RegisterCoalescer.cpp#L1993) joins a copy with a
    /// physical register only when it is reserved. A pinned value stands for a
    /// register that is not (an argument register): joined, a value takes the
    /// pin's unsplittable range. False: such a copy is left to the allocator.
    /// True keeps the join (the targets that have not stated otherwise).
    fn joins_pinned_copies(&self) -> bool {
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

/// The (destination, source) of a plain move between two values of any
/// widths (`isMoveInstr`).
pub fn move_copy(insn: &Insn) -> Option<(Held, Held)> {
    let what = insn.what.as_ref()?;
    if what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1 {
        return None;
    }
    match (&what.dests[0], &what.sources[0]) {
        (Loc::Held(dst), Loc::Held(src)) => Some((*dst, *src)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct CoalescerPair {
    src_reg: u32,
    dst_reg: u32,
    flipped: bool,
    partial: bool,
}

impl CoalescerPair {
    /// `setRegisters`: the pair of a copy, or None where it is not a move
    /// the coalescer may join.
    pub fn set_registers(
        copy: &Insn,
        env: &dyn CoalescerEnv,
    ) -> Option<Self> {
        let (dst, src) = move_copy(copy)?;
        if dst.width == src.width {
            if !env.can_share(src.value, dst.value) {
                return None;
            }
            return Some(Self { src_reg: src.value, dst_reg: dst.value, flipped: false, partial: false });
        }
        // The narrow value is merged into the wide one, whichever way the
        // copy goes.
        let truncates = dst.width < src.width;
        let (narrow, wide) = if truncates { (dst, src) } else { (src, dst) };
        if narrow.value == wide.value || !env.can_share_sub(narrow.value, wide.value) {
            return None;
        }
        Some(Self { src_reg: narrow.value, dst_reg: wide.value, flipped: truncates, partial: true })
    }

    /// `isPartial`: one value is a sub-register of the other.
    pub fn is_partial(&self) -> bool {
        self.partial
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
        assert!(!self.partial, "a partial pair keeps the wide value");
        std::mem::swap(&mut self.src_reg, &mut self.dst_reg);
        self.flipped = !self.flipped;
    }

    /// `isCoalescable`: whether `insn` is a copy between this pair's values.
    pub fn is_coalescable(
        &self,
        insn: &Insn,
    ) -> bool {
        let Some((dst, src)) = move_copy(insn) else { return false };
        if self.partial != (dst.width != src.width) {
            return false;
        }
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
