//! `VirtRegMap` and the register units of the target's file: which physical
//! register each virtual register is assigned, and what a register is made of.
//! Translated from llvm/lib/CodeGen/VirtRegMap.cpp (the assignment half),
//! llvm/include/llvm/CodeGen/VirtRegMap.h and the `regunits` of
//! `MCRegisterInfo`, at d1106deb71cc.
//!
//! # How it works
//! A register unit is the smallest piece of a register two registers can
//! share: here one byte lane of a root (`al` and `ah` are two units of `eax`,
//! `ax` is both, `eax` all four). Two registers interfere exactly when they
//! share a unit, so the live interval unions are kept per unit and an
//! assignment of a value to a register is an entry in each of its units.
//!
//! ```text
//!   unit    eax.0  eax.1  eax.2  eax.3
//!   al       x
//!   ah              x
//!   ax       x      x
//!   eax      x      x      x      x
//! ```
//!
//! DEVIATE (no number): TableGen's units for x86 are not the byte lanes (AX has
//! two units, EAX a third for its high half); the lane model is the target
//! description's own (`Info::lanes`), and a byte-granular unit gives the same
//! overlaps. A segment register or a stack register is a root of one unit.
//!
//! # Example
//! `units(ax)` is `[eax.0, eax.1]`; `ah` and `al` share none.
//!
//! # Used by
//! `matrix`.

use std::collections::HashMap;

use llrm_lir::registers::{RegId, Regs};

/// A register unit: `root number * 4 + byte lane` (`MCRegUnit`).
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
        let lanes = self.regs.lanes(reg) as u32;
        Units { root: root * 4, lanes: lanes & 0xf }
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
