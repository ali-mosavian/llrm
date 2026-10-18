//! `VirtRegMap` and the register units of the target's file: which physical
//! register each virtual register is assigned, and what a register is made of.
//! Translated from llvm/lib/CodeGen/VirtRegMap.cpp (the assignment half),
//! llvm/include/llvm/CodeGen/VirtRegMap.h and the `regunits` of
//! `MCRegisterInfo`, at d1106deb71cc.
//!
//! # How it works
//! A register unit is the smallest piece of a register two registers can
//! share. They are X86RegisterInfo.td's, as `llvm-tblgen -gen-register-info`
//! writes them at d1106deb71cc: `al` and `ah` are two units of `eax`, `ax` is
//! both, and `eax` has a third, `HAX`, for its upper half (no register names
//! bytes 2 and 3 apart). Two registers interfere exactly when they share a
//! unit, so the live interval unions are kept per unit and an assignment of a
//! value to a register is an entry in each of its units.
//!
//! ```text
//!   unit    eax.lo  eax.hi  eax.up (HAX)
//!   al        x
//!   ah                x
//!   ax        x       x
//!   eax       x       x       x
//! ```
//!
//! Units per register against LLVM's table: a 32-bit general register 3, a
//! 16-bit one 2, a byte register 1, a segment register 1 (the test
//! `test_the_units_of_a_register_are_the_table_gen_units_of_x86` asserts the
//! table). The lane masks of the target description (`Info::lanes`) say which
//! bytes a register covers; the unit numbering is `root * 4 + {0, 1, 2}`.
//!
//! # Example
//! `units(ax)` is `[eax.lo, eax.hi]`; `ah` and `al` share none.
//!
//! # Used by
//! `matrix`.

use llrm_lir::registers::{RegId, Regs};

use crate::support::hash::HashMap;

/// A register unit: `root number * 4 + {0: low byte, 1: high byte, 2: upper
/// half}` (`MCRegUnit`).
pub type RegUnit = u32;

/// The units of a register (`MCRegUnitIterator`), without a heap allocation.
#[derive(Clone, Copy)]
pub struct Units {
    root: u32,
    lanes: u32,
}

impl Iterator for Units {
    type Item = RegUnit;

    fn next(&mut self) -> Option<RegUnit> {
        if self.lanes == 0 {
            return None;
        }
        let lane = self.lanes.trailing_zeros();
        self.lanes &= self.lanes - 1;
        Some(self.root + lane)
    }
}

impl Units {
    pub fn len(&self) -> usize {
        self.lanes.count_ones() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.lanes == 0
    }
}

/// The target's register file as the allocator asks it (`MCRegisterInfo`).
#[derive(Clone, Copy)]
pub struct RegisterInfo {
    pub regs: Regs,
}

impl RegisterInfo {
    pub fn new(regs: Regs) -> Self {
        Self { regs }
    }

    /// `TRI->regunits(Reg)`: the byte lanes of the register's root it covers.
    pub fn regunits(
        &self,
        reg: RegId,
    ) -> Units {
        let root = self.regs.root(reg).index() as u32;
        // A segment register has no sub-registers: one unit (`DS` of the
        // table).
        let lanes = if self.regs.is_segment(reg) { 1 } else { self.regs.lanes(reg) as u32 & 0xf };
        // Bytes 2 and 3 are never named apart (no `eax` sub-register covers
        // one), so they are one unit, `HAX` of X86RegisterInfo.td.
        assert!(lanes & 0xc == 0 || lanes & 0xc == 0xc, "{reg:?} covers one of the upper bytes only");
        let hi = if lanes & 0xc != 0 { 0b100 } else { 0 };
        Units { root: root * 4, lanes: (lanes & 0b11) | hi }
    }

    /// Whether writing one register can be seen by reading the other.
    pub fn overlaps(
        &self,
        a: RegId,
        b: RegId,
    ) -> bool {
        self.regs.overlaps(a, b)
    }
}

/// `VirtRegMap` (VirtRegMap.h): the physical register of each virtual register.
#[derive(Default)]
pub struct VirtRegMap {
    virt2phys: HashMap<u32, RegId>,
    /// `getOriginal`: the register a split or spilled one came from.
    original: HashMap<u32, u32>,
}

impl VirtRegMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// `hasPhys`.
    pub fn has_phys(
        &self,
        virt: u32,
    ) -> bool {
        self.virt2phys.contains_key(&virt)
    }

    /// `getPhys`.
    pub fn get_phys(
        &self,
        virt: u32,
    ) -> Option<RegId> {
        self.virt2phys.get(&virt).copied()
    }

    /// `assignVirt2Phys`.
    pub fn assign_virt2phys(
        &mut self,
        virt: u32,
        phys: RegId,
    ) {
        assert!(
            !self.virt2phys.contains_key(&virt),
            "attempt to assign physical register to already mapped virtual register"
        );
        self.virt2phys.insert(virt, phys);
    }

    /// `clearVirt`.
    pub fn clear_virt(
        &mut self,
        virt: u32,
    ) {
        assert!(self.virt2phys.remove(&virt).is_some(), "attempt to clear a not assigned virtual register");
    }

    /// `setIsSplitFromReg`.
    pub fn set_is_split_from_reg(
        &mut self,
        virt: u32,
        from: u32,
    ) {
        let original = self.get_original(from);
        self.original.insert(virt, original);
    }

    /// `getOriginal`.
    pub fn get_original(
        &self,
        virt: u32,
    ) -> u32 {
        self.original.get(&virt).copied().unwrap_or(virt)
    }

    pub fn original_map(&self) -> &HashMap<u32, u32> {
        &self.original
    }

    /// `hasPreferredPhys`: whether the register sits in a register it prefers
    /// (its first hint, once assigned).
    pub fn has_preferred_phys(
        &self,
        virt: u32,
        hint: Option<RegId>,
    ) -> bool {
        match (self.get_phys(virt), hint) {
            (Some(phys), Some(hint)) => phys == hint,
            _ => false,
        }
    }
}
