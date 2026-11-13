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
//! DEVIATE M68K-STACK-ALIGN: the shipped build rounds the frame to 4, the m68k
//! Linux stack boundary gcc keeps, not the pin's 8, which only pads a frame (a
//! function that pushes two words and calls paid `lea (-4,%sp),%sp` and an
//! `addq.l #4`). Owner: m68k. Review: 2026-12-20. Row and price:
//! `docs/working/m68k-stack-align-row.md`.
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

/// The shipped build's stack alignment: m68k Linux, AmigaOS and TOS ask for
/// no more than 4.
pub const SHIPPED_STACK_ALIGN: i64 = 4;

/// An object aligned beyond this realigns the stack. Up to it the frame keeps
/// what it can (a `double` slot at the shipped 4): realigning for 8 grew
/// nbody 42 B and bitcount_u64 40 B at -O1.
pub const REALIGN_ABOVE: i64 = 8;

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
    /// The stack alignment; 0 is the pin's.
    pub stack_align: i64,
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
    /// `BuildStackAlignAND`: `move.l %sp, %d0; and.l #-align, %d0; move.l %d0,
    /// %sp`
    AlignSp(i64),
    /// `move.l reg, -(%sp)` for one register, else `movem.l regs, -(%sp)`
    Push { registers: Vec<RegId> },
    /// `move.l (%sp)+, reg` for one register, else `movem.l (%sp)+, regs`
    Pop { registers: Vec<RegId> },
    /// One `move.l reg, -(%sp)` a register, in the order that lays them out as
    /// `movem` does (gcc's form; dearer in bytes from three registers, cheaper
    /// in clocks from the 68020 on).
    PushEach { registers: Vec<RegId> },
    /// One `move.l (%sp)+, reg` a register, undoing [`FrameOp::PushEach`].
    PopEach { registers: Vec<RegId> },
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
    /// The frame realigns the stack pointer (`hasStackRealignment`): to
    /// `max_align`.
    realigned: bool,
    max_align: i64,
    /// `objects[first_local..first_save]` are the locals: with a realigned
    /// stack they are addressed from the aligned `%sp`, the rest from `%a6`.
    first_local: usize,
    first_save: usize,
}

/// `hasFPImpl`.
fn has_fp(function: &Function) -> bool {
    function.frame_pointer_all || function.has_var_sized || function.frame_address_taken
}

/// Lays the frame out as `PEIImpl::calculateFrameObjectOffsets` does with
/// M68k's hooks: with a frame pointer the saved `%a6` is a fixed object just
/// below the return address (`assignCalleeSavedSpillSlots`).
pub fn layout(function: &Function) -> Layout {
    let max_align = function.objects.iter().map(|one| one.align).max().unwrap_or(1);
    let stack_align = if function.stack_align == 0 { STACK_ALIGN } else { function.stack_align };
    // `hasFPImpl`: a realigned stack needs the frame pointer; the locals are
    // addressed from the stack pointer and the arguments from the frame pointer
    let has_fp = has_fp(function) || max_align > stack_align.max(REALIGN_ABOVE);
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
    let stack_align = if function.stack_align == 0 { STACK_ALIGN } else { function.stack_align };
    let realigned = max_align > stack_align.max(REALIGN_ABOVE);
    let frame = Frame {
        local_area: SLOT,
        max_align,
        stack_align,
        transient_align: 1,
        adjusts_stack: function.adjusts_stack,
        has_var_sized: function.has_var_sized,
        realigned,
        reserved_call_frame: !function.has_var_sized,
        max_call_frame_size: function.max_call_frame,
    };
    let stack_size = calculate_frame_object_offsets(&mut objects, fixed, frame);
    let lowest_save = (first_save..objects.len()).min_by_key(|&index| objects[index].offset);
    Layout { stack_size, has_fp, objects, saved, lowest_save, realigned, max_align, first_local: fixed, first_save }
}

impl Layout {
    /// `getFrameIndexReference`: the register and displacement of object
    /// `index`, as numbered in [`Function::objects`] (the frame pointer's slot,
    /// when there is one, is not counted).
    pub fn address(
        &self,
        index: usize,
    ) -> (Base, i64) {
        // the frame pointer's slot sits between the fixed objects and the
        // locals
        let index = if self.has_fp && index >= self.first_local - 1 { index + 1 } else { index };
        self.address_of(index)
    }

    /// [`Layout::address`] of an object by its place in [`Layout::objects`].
    fn address_of(
        &self,
        index: usize,
    ) -> (Base, i64) {
        let offset = self.objects[index].offset + SLOT;
        let local = (self.first_local..self.first_save).contains(&index);
        // with a realigned stack the locals are `StackSize` above the aligned
        // stack pointer; the fixed objects and the saved registers are below
        // the frame pointer as ever
        if self.has_fp && !(self.realigned && local) {
            (Base::Fp, offset + SLOT)
        } else {
            (Base::Sp, offset + self.stack_size)
        }
    }

    /// The bytes the prologue takes off the stack pointer or `link` reserves:
    /// the stack size less the saved frame pointer. `CalleeSavedFrameSize`,
    /// which the pin subtracts, is 0 here: no register is pushed.
    fn adjustment(&self) -> i64 {
        self.stack_size - if self.has_fp { SLOT } else { 0 }
    }

    /// Where the `movem` base is: the lowest saved slot's address.
    fn save_base(&self) -> Option<(Base, i64)> {
        self.lowest_save.map(|index| self.address_of(index))
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
        // `emitPrologue`: the stack is realigned after the registers are saved
        if self.realigned {
            ops.push(FrameOp::AlignSp(self.max_align));
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

    /// [`Layout::push_prologue`] with a `move.l` for each register.
    pub fn push_each_prologue(&self) -> Option<Vec<FrameOp>> {
        let mut ops = self.push_prologue()?;
        ops[0] = FrameOp::PushEach { registers: self.saved.clone() };
        Some(ops)
    }

    /// The epilogue that undoes [`Layout::push_each_prologue`].
    pub fn push_each_epilogue(&self) -> Option<Vec<FrameOp>> {
        let mut ops = self.push_epilogue()?;
        let last = ops.len() - 1;
        ops[last] = FrameOp::PopEach { registers: self.saved.clone() };
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
