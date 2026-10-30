//! `LiveRangeEdit`: the edit a spill or a split makes of a live range, and the
//! registers it creates. Translated from
//! llvm/include/llvm/CodeGen/LiveRangeEdit.h and llvm/lib/CodeGen/
//! LiveRangeEdit.cpp (`createFrom`, `eraseVirtReg`, the `Delegate` callbacks)
//! at d1106deb71cc.
//!
//! # How it works
//! An edit is made for one parent register. `create_from` numbers a new
//! register that names the parent as its original in the `VirtRegMap`
//! (`MRI.createVirtualRegister` and `VRM.setIsSplitFromReg`), tells the
//! delegate (`LRE_DidCloneVirtReg`), and remembers it; the spiller or the
//! splitter then fills it with instructions. `erase_virt_reg` is the opposite:
//! the register has no operand left, so its interval goes
//! (`LRE_CanEraseVirtReg` asks the delegate first).
//!
//! DEVIATE D-E1: `eliminateDeadDefs` is in the spiller and the splitter
//! (`InlineSpiller::eliminate_dead_defs`, `SplitEditor::delete_remat_victims`),
//! not here, and erases what LLVM's `isInstructionTriviallyDead` would for the
//! products of a spill or split: plain copies and the definitions
//! `greedy/remat.rs` makes again. A spill's other products (the reload, the
//! instruction itself, the store) are never dead, so no case differs. Exit: the
//! general `isInstructionTriviallyDead` test, when a pass leaves such a def.
//!
//! # Example
//! `create_from(7)` gives `%12`, whose original is `%7`; `new_regs()` is
//! `[12]`.
//!
//! # Used by
//! `inlinespiller`.

use super::virtregmap::VirtRegMap;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;

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

    /// `getReg`: the register being edited.
    pub fn get_reg(&self) -> u32 {
        self.parent
    }

    /// `size`.
    pub fn size(&self) -> usize {
        self.new_regs.len()
    }

    /// `empty`.
    pub fn empty(&self) -> bool {
        self.new_regs.is_empty()
    }

    /// `get(idx)`: the `idx`th register created.
    pub fn get(
        &self,
        idx: usize,
    ) -> u32 {
        self.new_regs[idx]
    }

    /// `createEmptyIntervalFrom(getReg())`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeEdit.cpp#L43>):
    /// a new register, split from the parent, with an empty interval that is
    /// unspillable when the parent is.
    pub fn create_empty_interval(
        &mut self,
        lis: &mut LiveIntervals,
        vrm: &mut VirtRegMap,
    ) -> u32 {
        let new = lis.create_virtual_register();
        self.create_from(lis, vrm, new, self.parent);
        let mut li = LiveInterval::new(new, 0.0);
        if lis.has_interval(self.parent) && !lis.get_interval(self.parent).is_spillable() {
            li.mark_not_spillable();
        }
        lis.insert_interval(li);
        new
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
