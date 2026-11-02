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
    /// DEVIATE CO1 (see `Coalescer::coalesce_carried`): whether the copies of a
    /// value live across a loop's back edge are joined first.
    fn joins_carried_first(&self) -> bool {
        false
    }

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

    /// `getMatchingSuperRegClass(WideRC, NarrowRC, SubIdx)` for the
    /// sub-register at byte `offset` of the wide value (`SubIdx` other than
    /// the low one): the registers `wide` may take have, at that offset,
    /// registers `narrow` may take. A target without such a view answers
    /// no, and a copy to or from a lane above the low one is left alone.
    fn can_share_lane(
        &self,
        _narrow: u32,
        _wide: u32,
        _offset: u32,
    ) -> bool {
        false
    }

    /// `narrow_sub` for the lane at byte `offset` of `wide`.
    fn narrow_lane(
        &self,
        _wide: u32,
        _narrow_value: u32,
        _offset: u32,
    ) {
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

    /// Whether the env keeps a summary of what names each value, so the
    /// intervals record the instructions put in and taken out for it
    /// (DEVIATE C14).
    fn tracks_uses(&self) -> bool {
        false
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
    /// (2026-11-15).
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

    /// Whether making `def` again beats the copy `source -> destination` it
    /// would replace: the target's own price (`isAsCheapAsAMove` against a
    /// move of that register pair). The pin's `reMaterializeDef` asks none, so
    /// a target without one remats whenever it can.
    fn remat_wins(
        &self,
        _def: &Insn,
        _source: u32,
        _destination: u32,
    ) -> bool {
        true
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

/// A copy between two whole values of one width. A lane (an offset above the
/// low byte) makes a pair partial whatever the widths.
fn is_whole(
    dst: Held,
    src: Held,
) -> bool {
    dst.width == src.width && dst.offset == 0 && src.offset == 0
}

#[derive(Clone, Debug)]
pub struct CoalescerPair {
    src_reg: u32,
    dst_reg: u32,
    flipped: bool,
    partial: bool,
    /// The pin's `SrcIdx`/`DstIdx` of a partial pair: the byte of `dst_reg`
    /// that `src_reg` is. Zero for every pair of the low part.
    lane_offset: u32,
    /// The bytes of `src_reg` (`lane_offset` .. `+ lane_width` of `dst_reg`).
    lane_width: u32,
}

impl CoalescerPair {
    /// `setRegisters`: the pair of a copy, or None where it is not a move
    /// the coalescer may join.
    pub fn set_registers(
        copy: &Insn,
        env: &dyn CoalescerEnv,
    ) -> Option<Self> {
        let (dst, src) = move_copy(copy)?;
        if is_whole(dst, src) {
            if !env.can_share(src.value, dst.value) {
                return None;
            }
            return Some(Self {
                src_reg: src.value,
                dst_reg: dst.value,
                flipped: false,
                partial: false,
                lane_offset: 0,
                lane_width: src.width,
            });
        }
        // The narrow value is merged into the wide one, whichever way the
        // copy goes.
        let truncates = dst.width < src.width || (dst.width == src.width && src.offset != 0);
        let (narrow, wide) = if truncates { (dst, src) } else { (src, dst) };
        if narrow.offset != 0 || narrow.value == wide.value || (wide.offset == 0 && wide.width == narrow.width) {
            return None;
        }
        let shareable = match wide.offset {
            0 => env.can_share_sub(narrow.value, wide.value),
            offset => env.can_share_lane(narrow.value, wide.value, offset),
        };
        if !shareable {
            return None;
        }
        Some(Self {
            src_reg: narrow.value,
            dst_reg: wide.value,
            flipped: truncates,
            partial: true,
            lane_offset: wide.offset,
            lane_width: narrow.width,
        })
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

    /// The byte of `dst_reg` that `src_reg` is (`DstIdx`).
    pub fn lane_offset(&self) -> u32 {
        self.lane_offset
    }

    /// The bytes of `dst_reg` that `src_reg` is (`DstIdx` as a lane mask).
    pub fn lanes(&self) -> llrm_lir::valuelanes::ValueLanes {
        llrm_lir::valuelanes::ValueLanes::at(self.lane_offset, self.lane_width)
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
        if self.partial == is_whole(dst, src) {
            return false;
        }
        if self.partial && dst.offset.max(src.offset) != self.lane_offset {
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
