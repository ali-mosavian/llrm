//! The prologue and epilogue a frame needs on i486: LLVM's
//! `X86FrameLowering::emitPrologue`
//! ([L1677](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L1677)),
//! `emitEpilogue`
//! ([L2580](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L2580))
//! and `emitSPUpdate`
//! ([L247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L247)),
//! the callee-saved spills of `spillCalleeSavedRegisters` included.
//!
//! # How it works
//! With a frame pointer the frame record is `StackSize - SlotSize` bytes, of
//! which the callee-saved pushes are `callee_saved_size` and the rest,
//! `NumBytes`, is the `sub` (raised to the largest alignment when the stack
//! is realigned). Without one `NumBytes` is `StackSize - callee_saved_size`.
//!
//! The prologue is `push ebp; mov ebp, esp`, the callee-saved pushes in
//! order, `and esp, -align` if realigned, then the stack adjustment. The
//! epilogue is the adjustment undone (`mov esp, ebp` or `lea esp, [ebp -
//! callee_saved_size]` where the stack is realigned or has a variable-sized
//! object, else `add esp, NumBytes`), the callee-saved pops in reverse, and
//! `pop ebp`. A stack adjustment of one slot is a `push eax` (a `pop` of a
//! dead caller-saved register in an epilogue) where one is free.
//!
//! ```text
//!   stack 24, saved esi edi (8), frame pointer:   NumBytes = 24 - 4 - 8 = 12
//!   push ebp; mov ebp,esp; push esi; push edi; sub esp,12
//!   add esp,12; pop edi; pop esi; pop ebp
//! ```
//!
//! DEVIATE FL-1 (no number, owner w-6): Windows and SEH, funclets, the tail
//! call return-address reserve, a Swift async context, the red zone, the
//! stack probe and DWARF CFI are not translated: none applies to i486.
//!
//! DEVIATE FL-2 (no number, owner w-6): `mergeSPUpdates` and `mergeSPAdd`,
//! which fold an adjacent stack update into this one, are not translated: a
//! function of the frame alone sees no neighbour.
//!
//! DEVIATE CG7 (slot table row 80): `masm::parts` calls this for the flat
//! m32 frames built at entry (`saved_first`); every other frame (16-bit, the
//! runtime's, interrupt, inline-code, shrink-wrapped) keeps `parts`' own
//! sequence, and cells are not yet frame indices (FI-1,
//! docs/working/pei-plan.md).
//!
//! # Example
//! The unit test `a_frame_with_saved_registers_pushes_and_pops_them`.
//!
//! # Used by
//! `masm::lowered`, for `saved_first` frames.

/// A register, by whatever number the caller gives it.
pub type Reg = u32;

/// What the frame needs, as `MachineFrameInfo` and `X86MachineFunctionInfo`
/// state it.
#[derive(Clone, Debug)]
pub struct FrameInfo {
    pub has_fp: bool,
    /// `MFI.getStackSize()`, the frame pointer's slot included.
    pub stack_size: u64,
    pub slot_size: u64,
    /// The registers pushed after the frame pointer, in push order.
    pub saved: Vec<Reg>,
    pub frame_pointer: Reg,
    /// `TRI->hasStackRealignment`, with the alignment to realign to.
    pub realign_to: Option<u64>,
    pub has_var_sized_objects: bool,
}

/// One instruction of the sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Op {
    Push(Reg),
    Pop(Reg),
    /// `mov fp, sp`.
    MovFpSp,
    /// `mov sp, fp`.
    MovSpFp,
    /// `lea sp, [fp + offset]`.
    LeaSpFp(i64),
    /// `and sp, -align`.
    AndSp(u64),
    SubSp(u64),
    AddSp(u64),
}

impl FrameInfo {
    fn callee_saved_size(&self) -> u64 {
        self.saved.len() as u64 * self.slot_size
    }

    /// `NumBytes` of `emitPrologue` and `emitEpilogue`.
    pub fn num_bytes(&self) -> u64 {
        if self.has_fp {
            let frame = self.stack_size - self.slot_size;
            match self.realign_to {
                Some(align) => frame.saturating_sub(self.callee_saved_size()).next_multiple_of(align),
                None => frame - self.callee_saved_size(),
            }
        } else {
            self.stack_size - self.callee_saved_size()
        }
    }
}

/// `emitSPUpdate` for `bytes` below (`sub`) or above (`add`) the stack
/// pointer: one slot is a push of `eax` or a pop of a dead register.
fn sp_update(
    out: &mut Vec<Op>,
    bytes: u64,
    sub: bool,
    slot: u64,
    eax: Reg,
    dead: Option<Reg>,
) {
    if bytes == 0 {
        return;
    }
    if bytes == slot {
        match (sub, dead) {
            (true, _) => return out.push(Op::Push(eax)),
            (false, Some(reg)) => return out.push(Op::Pop(reg)),
            (false, None) => {}
        }
    }
    out.push(if sub { Op::SubSp(bytes) } else { Op::AddSp(bytes) });
}

/// `emitPrologue` with the callee-saved pushes; `eax` is the register a
/// one-slot adjustment pushes.
pub fn emit_prologue(
    frame: &FrameInfo,
    eax: Reg,
) -> Vec<Op> {
    let mut out = Vec::new();
    if frame.has_fp {
        out.push(Op::Push(frame.frame_pointer));
        out.push(Op::MovFpSp);
    }
    out.extend(frame.saved.iter().map(|&reg| Op::Push(reg)));
    if let Some(align) = frame.realign_to.filter(|_| frame.has_fp) {
        out.push(Op::AndSp(align));
    }
    sp_update(&mut out, frame.num_bytes(), true, frame.slot_size, eax, None);
    out
}

/// `emitEpilogue` with the callee-saved pops; `dead` is a caller-saved
/// register nothing reads after the epilogue, for a one-slot `pop`.
pub fn emit_epilogue(
    frame: &FrameInfo,
    dead: Option<Reg>,
) -> Vec<Op> {
    let mut out = Vec::new();
    let from_fp = frame.has_fp && (frame.realign_to.is_some() || frame.has_var_sized_objects);
    if from_fp {
        let saved = frame.callee_saved_size();
        out.push(if saved == 0 { Op::MovSpFp } else { Op::LeaSpFp(-(saved as i64)) });
    } else {
        sp_update(&mut out, frame.num_bytes(), false, frame.slot_size, 0, dead);
    }
    out.extend(frame.saved.iter().rev().map(|&reg| Op::Pop(reg)));
    if frame.has_fp {
        out.push(Op::Pop(frame.frame_pointer));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{FrameInfo, Op, emit_epilogue, emit_prologue};

    const EAX: u32 = 0;
    const EBX: u32 = 3;
    const ESI: u32 = 6;
    const EDI: u32 = 7;
    const EBP: u32 = 5;
    const ECX: u32 = 1;

    fn frame() -> FrameInfo {
        FrameInfo {
            has_fp: true,
            stack_size: 24,
            slot_size: 4,
            saved: vec![ESI, EDI],
            frame_pointer: EBP,
            realign_to: None,
            has_var_sized_objects: false,
        }
    }

    /// The header's example: the frame record is 24 - 4, 8 of it pushes.
    #[test]
    fn a_frame_with_saved_registers_pushes_and_pops_them() {
        assert_eq!(
            emit_prologue(&frame(), EAX),
            [Op::Push(EBP), Op::MovFpSp, Op::Push(ESI), Op::Push(EDI), Op::SubSp(12)]
        );
        assert_eq!(emit_epilogue(&frame(), None), [Op::AddSp(12), Op::Pop(EDI), Op::Pop(ESI), Op::Pop(EBP)]);
    }

    /// With no frame pointer `NumBytes` is the whole stack less the pushes.
    #[test]
    fn a_frame_without_a_frame_pointer_adjusts_by_the_stack_size() {
        let leaf = FrameInfo { has_fp: false, stack_size: 20, saved: vec![EBX], ..frame() };
        assert_eq!(emit_prologue(&leaf, EAX), [Op::Push(EBX), Op::SubSp(16)]);
        assert_eq!(emit_epilogue(&leaf, None), [Op::AddSp(16), Op::Pop(EBX)]);
    }

    /// A one-slot adjustment is a `push eax`; undone it is a `pop` of a dead
    /// register, or an `add` where none is free.
    #[test]
    fn a_one_slot_adjustment_is_a_push_and_a_pop() {
        let small = FrameInfo { has_fp: false, stack_size: 4, saved: vec![], ..frame() };
        assert_eq!(emit_prologue(&small, EAX), [Op::Push(EAX)]);
        assert_eq!(emit_epilogue(&small, Some(ECX)), [Op::Pop(ECX)]);
        assert_eq!(emit_epilogue(&small, None), [Op::AddSp(4)]);
    }

    /// A realigned stack ands the pointer after the pushes, raises `NumBytes`
    /// to the alignment and restores the pointer from the frame pointer's
    /// frame: `lea` past the saved registers, `mov` where there are none.
    #[test]
    fn a_realigned_stack_is_restored_from_the_frame_pointer() {
        let aligned = FrameInfo { realign_to: Some(16), stack_size: 28, ..frame() };
        // 28 - 4 - 8 = 16, already aligned.
        assert_eq!(
            emit_prologue(&aligned, EAX),
            [Op::Push(EBP), Op::MovFpSp, Op::Push(ESI), Op::Push(EDI), Op::AndSp(16), Op::SubSp(16)]
        );
        assert_eq!(emit_epilogue(&aligned, None), [Op::LeaSpFp(-8), Op::Pop(EDI), Op::Pop(ESI), Op::Pop(EBP)]);
        let bare = FrameInfo { saved: vec![], realign_to: Some(16), stack_size: 24, ..frame() };
        // 24 - 4 = 20, up to 32.
        assert_eq!(emit_prologue(&bare, EAX), [Op::Push(EBP), Op::MovFpSp, Op::AndSp(16), Op::SubSp(32)]);
        assert_eq!(emit_epilogue(&bare, None), [Op::MovSpFp, Op::Pop(EBP)]);
    }

    /// A variable-sized object (an alloca) moves the stack pointer in the
    /// body: the epilogue restores it from the frame pointer.
    #[test]
    fn a_variable_sized_object_restores_the_stack_from_the_frame_pointer() {
        let dynamic = FrameInfo { has_var_sized_objects: true, ..frame() };
        assert_eq!(emit_epilogue(&dynamic, None)[0], Op::LeaSpFp(-8));
    }
}
