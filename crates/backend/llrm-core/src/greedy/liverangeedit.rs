//! `LiveRangeEdit`: the edit a spill or a split makes of a live range, and the
//! registers it creates. Translated from llvm/include/llvm/CodeGen/LiveRangeEdit.h
//! and llvm/lib/CodeGen/LiveRangeEdit.cpp (`createFrom`, `eraseVirtReg`,
//! the `Delegate` callbacks) at d1106deb71cc.
//!
//! # How it works
//! An edit is made for one parent register. `create_from` numbers a new
//! register that names the parent as its original in the `VirtRegMap`
//! (`MRI.createVirtualRegister` and `VRM.setIsSplitFromReg`), tells the
//! delegate (`LRE_DidCloneVirtReg`), and remembers it; the spiller or the
//! splitter then fills it with instructions. `erase_virt_reg` is the opposite:
//! the register has no operand left, so its interval goes (`LRE_CanEraseVirtReg`
//! asks the delegate first).
//!
//! DEVIATE D-E1: `eliminateDeadDefs` and `anyRematerializable` are not here.
//! The fact: a spill's products here are a reload, the instruction itself and a
//! store, none dead; what is made again (constants, frame addresses, loads of a
//! frame cell) is decided by `backend::spiller`'s plan, which carries the m16
//! forms. Exit: the spiller's plan is translated as `TargetInstrInfo::
//! isTriviallyReMaterializable` rules (issue to be opened with step 2).
//!
//! # Example
//! `create_from(7)` gives `%12`, whose original is `%7`; `new_regs()` is `[12]`.
//!
//! # Used by
//! `inlinespiller`.

use super::virtregmap::VirtRegMap;
use crate::liveness::liveintervals::LiveIntervals;

/// `LiveRangeEdit::Delegate`: what the allocator wants told as ranges are made
/// and erased.
pub trait Delegate {
    /// `LRE_DidCloneVirtReg`: `new` was made from `old`.
    fn did_clone_virt_reg(
        &mut self,
        _new: u32,
        _old: u32,
    ) {
    }

    /// `LRE_CanEraseVirtReg`: whether the allocator allows `reg` to go.
    fn can_erase_virt_reg(
        &mut self,
        _reg: u32,
    ) -> bool {
        true
    }
}

/// No one to tell.
pub struct NoDelegate;

impl Delegate for NoDelegate {}

/// `LiveRangeEdit`.
pub struct LiveRangeEdit<'d> {
    parent: u32,
    new_regs: Vec<u32>,
    delegate: &'d mut dyn Delegate,
}

impl<'d> LiveRangeEdit<'d> {
    pub fn new(
        parent: u32,
        delegate: &'d mut dyn Delegate,
    ) -> Self {
        Self { parent, new_regs: Vec::new(), delegate }
    }

    /// `getParent`.
    pub fn parent(&self) -> u32 {
        self.parent
    }

    /// `regs()`: the registers created so far.
    pub fn new_regs(&self) -> &[u32] {
        &self.new_regs
    }

    /// `createFrom(OldReg)` for a register a component numbered itself (the
    /// spiller numbers its values before the instructions are written):
    /// `new` is made known to the map and the delegate.
    pub fn create_from(
        &mut self,
        lis: &mut LiveIntervals,
        vrm: &mut VirtRegMap,
        new: u32,
        old: u32,
    ) {
        lis.reserve_virtual_register(new);
        vrm.set_is_split_from_reg(new, old);
        self.delegate.did_clone_virt_reg(new, old);
        self.new_regs.push(new);
    }

    /// `eraseVirtReg`: `reg` has no operand left, its interval goes. Returns
    /// whether it went (the delegate may keep it).
    pub fn erase_virt_reg(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
    ) -> bool {
        assert!(lis.reg_nodbg_empty(reg), "v{reg} is erased while an instruction names it");
        if self.delegate.can_erase_virt_reg(reg) {
            lis.remove_interval(reg);
            true
        } else {
            false
        }
    }
}
