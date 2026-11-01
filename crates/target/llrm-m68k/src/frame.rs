//! `M68kFrameLowering`: where the frame objects go, the prologue and epilogue,
//! and the address of an object.
//!
//! # How it works
//! The stack grows down and the return address sits at the local area (4
//! bytes). The layout is the generic `calculate_frame_object_offsets`: fixed
//! objects, the saved registers, then the locals, then the outgoing call area
//! the frame reserves, the whole rounded to the stack alignment (8). Without a
//! frame pointer the prologue is `suba.l #size, %sp` and objects are addressed
//! from `%sp`; with one it is `link.w %a6, #-(size - 4)` and objects
//! are addressed from `%a6`. The saved registers go in one `movem.l`, from the
//! lowest of their slots, after the stack adjustment and before the `unlk` or
//! `adda.l` in the epilogue.
//!
//! DEVIATE: not translated are stack realignment and the base pointer,
//! dynamic allocas, tail calls (`TCReturnAddrDelta`), a stack adjustment over
//! 2 GiB, CFI and shrink wrapping. Reason: the slice does not reach them.
//! Exit: M3d.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kFrameLowering.cpp`, pin d1106deb71cc: `hasFPImpl`
//! (L44), `getFrameIndexReference` (L77), `emitPrologue` (L471),
//! `emitEpilogue` (L666), `assignCalleeSavedSpillSlots` (L799),
//! `spillCalleeSavedRegisters` (L828).

use llrm_codegen::backend::frameobjects::{Frame, Object, calculate_frame_object_offsets};
use llrm_lir::registers::RegId;

use crate::abi;
use crate::reg;

/// `M68kSubtarget::getSlotSize`, and `getOffsetOfLocalArea`'s magnitude.
const SLOT: i64 = 4;
/// `M68kSubtarget::stackAlignment`.
const STACK_ALIGN: i64 = 8;

/// What the frame depends on: `MachineFrameInfo` and the function's attributes.
#[derive(Clone, Debug, Default)]
pub struct Function {
    /// Fixed objects (incoming arguments) first, then the locals; an offset is
    /// given for the fixed ones only.
    pub objects: Vec<Object>,
    /// How many of `objects` are fixed.
    pub fixed: usize,
    /// The callee-saved registers the allocator used, in `CSR_STD` order.
    pub saved: Vec<RegId>,
    pub max_call_frame: i64,
    pub adjusts_stack: bool,
    /// `"frame-pointer"="all"`.
    pub frame_pointer_all: bool,
    pub has_var_sized: bool,
    pub frame_address_taken: bool,
}

/// The register an object is addressed through.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Base {
    Sp,
    Fp,
}

/// An instruction of the prologue or epilogue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameOp {
    /// `link.w %a6, #-bytes`
    Link { bytes: i64 },
    /// `unlk %a6`
    Unlink,
    /// `suba.l #bytes, %sp`
    SubSp(i64),
    /// `adda.l #bytes, %sp`
    AddSp(i64),
    /// `movem.l regs, (offset, base)`
    Save { registers: Vec<RegId>, base: Base, offset: i64 },
    /// `movem.l (offset, base), regs`
    Restore { registers: Vec<RegId>, base: Base, offset: i64 },
    /// `move.l reg, -(%sp)` for one register, else `movem.l regs, -(%sp)`
    Push { registers: Vec<RegId> },
    /// `move.l (%sp)+, reg` for one register, else `movem.l (%sp)+, regs`
    Pop { registers: Vec<RegId> },
}

/// The laid-out frame.
#[derive(Clone, Debug)]
pub struct Layout {
    pub stack_size: i64,
    pub has_fp: bool,
    /// `objects` with every offset set (from the incoming stack pointer, less
    /// the return address), the saved-register slots last.
    pub objects: Vec<Object>,
    saved: Vec<RegId>,
    /// The index of the saved-register slot with the lowest address.
    lowest_save: Option<usize>,
}

/// `hasFPImpl`.
fn has_fp(function: &Function) -> bool {
    function.frame_pointer_all || function.has_var_sized || function.frame_address_taken
}

/// Lays the frame out as `PEIImpl::calculateFrameObjectOffsets` does with
/// M68k's hooks: with a frame pointer the saved `%a6` is a fixed object just
/// below the return address (`assignCalleeSavedSpillSlots`).
pub fn layout(function: &Function) -> Layout {
    let has_fp = has_fp(function);
    let mut objects = function.objects.clone();
    let mut fixed = function.fixed;
    if has_fp {
        objects.insert(fixed, Object { size: SLOT, align: SLOT, offset: -2 * SLOT, callee_saved: false, dead: false });
        fixed += 1;
    }
    // the frame pointer is saved by `link`, not by `movem`
    let saved: Vec<RegId> = function.saved.iter().copied().filter(|&one| !(has_fp && one == reg::A6)).collect();
    // The slots are alike and the `movem` uses only the lowest, so the order
    // `calculate_frame_object_offsets` visits them in does not show.
    let first_save = objects.len();
    for _ in &saved {
        objects.push(Object { size: SLOT, align: SLOT, offset: 0, callee_saved: true, dead: false });
    }
    let max_align = objects.iter().map(|one| one.align).max().unwrap_or(1);
    let frame = Frame {
        local_area: SLOT,
        max_align,
        stack_align: STACK_ALIGN,
        transient_align: 1,
        adjusts_stack: function.adjusts_stack,
        has_var_sized: function.has_var_sized,
        realigned: false,
        reserved_call_frame: !function.has_var_sized,
        max_call_frame_size: function.max_call_frame,
    };
    let stack_size = calculate_frame_object_offsets(&mut objects, fixed, frame);
    let lowest_save = (first_save..objects.len()).min_by_key(|&index| objects[index].offset);
    Layout { stack_size, has_fp, objects, saved, lowest_save }
}

impl Layout {
    /// `getFrameIndexReference`: the register and displacement of object
    /// `index` (as numbered in [`Function::objects`], plus the frame pointer's
    /// slot when there is one).
    pub fn address(
        &self,
        index: usize,
    ) -> (Base, i64) {
        let offset = self.objects[index].offset + SLOT;
        if self.has_fp { (Base::Fp, offset + SLOT) } else { (Base::Sp, offset + self.stack_size) }
    }

    /// The bytes the prologue takes off the stack pointer or `link` reserves:
    /// the stack size less the saved frame pointer. `CalleeSavedFrameSize`,
    /// which the pin subtracts, is 0 here: no register is pushed.
    fn adjustment(&self) -> i64 {
        self.stack_size - if self.has_fp { SLOT } else { 0 }
    }

    /// Where the `movem` base is: the lowest saved slot's address.
    fn save_base(&self) -> Option<(Base, i64)> {
        self.lowest_save.map(|index| self.address(index))
    }

    /// `emitPrologue` with `spillCalleeSavedRegisters`: the frame is opened,
    /// then the registers saved.
    pub fn prologue(&self) -> Vec<FrameOp> {
        let mut ops = Vec::new();
        let bytes = self.adjustment();
        if self.has_fp {
            ops.push(FrameOp::Link { bytes });
        } else if bytes != 0 {
            ops.push(FrameOp::SubSp(bytes));
        }
        if let Some((base, offset)) = self.save_base() {
            ops.push(FrameOp::Save { registers: self.saved.clone(), base, offset });
        }
        ops
    }

    /// The prologue with the saved registers pushed before the rest of the
    /// frame is opened, when there is no frame pointer. The saved slots are the
    /// top of the frame, so every object keeps its address.
    ///
    /// DEVIATE from the pin, which opens the frame with `suba.l` and `movem`s
    /// the registers into slots. Reason: the push is shorter and no slower, and
    /// gcc does it. Exit: none; [`Layout::pushes_where_cheaper`] says when.
    pub fn push_prologue(&self) -> Option<Vec<FrameOp>> {
        if self.has_fp || self.saved.is_empty() {
            return None;
        }
        let rest = self.adjustment() - SLOT * self.saved.len() as i64;
        let mut ops = vec![FrameOp::Push { registers: self.saved.clone() }];
        if rest != 0 {
            ops.push(FrameOp::SubSp(rest));
        }
        Some(ops)
    }

    /// The epilogue that undoes [`Layout::push_prologue`].
    pub fn push_epilogue(&self) -> Option<Vec<FrameOp>> {
        if self.has_fp || self.saved.is_empty() {
            return None;
        }
        let rest = self.adjustment() - SLOT * self.saved.len() as i64;
        let mut ops = Vec::new();
        if rest != 0 {
            ops.push(FrameOp::AddSp(rest));
        }
        ops.push(FrameOp::Pop { registers: self.saved.clone() });
        Some(ops)
    }

    /// `emitEpilogue` with `restoreCalleeSavedRegisters`.
    pub fn epilogue(&self) -> Vec<FrameOp> {
        let mut ops = Vec::new();
        if let Some((base, offset)) = self.save_base() {
            ops.push(FrameOp::Restore { registers: self.saved.clone(), base, offset });
        }
        let bytes = self.adjustment();
        if self.has_fp {
            ops.push(FrameOp::Unlink);
        } else if bytes != 0 {
            ops.push(FrameOp::AddSp(bytes));
        }
        ops
    }
}

/// The registers `CSR_STD` keeps, in the order the pin assigns them slots
/// (`calling.toml`'s `preserved`).
pub fn callee_saved() -> Vec<RegId> {
    abi::c().preserved.iter().map(|kept| abi::register(&kept.full)).collect()
}
