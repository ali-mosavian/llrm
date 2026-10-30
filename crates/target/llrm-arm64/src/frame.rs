//! Frame lowering: the frame's layout, the prologue and epilogue, and the
//! frame objects turned into stack-pointer offsets
//! (`AArch64FrameLowering::emitPrologue`, `emitEpilogue`,
//! `eliminateFrameIndex`). The frame, from the stack pointer up: outgoing
//! arguments, frame objects, the callee-saved registers in pairs, and the frame
//! record (`x29`, `x30`) at the top. Offsets are from the stack pointer, which
//! does not move in the body.
//!
//! A frame of only saved registers moves the stack pointer in the first pair's
//! store and last load (`stp [sp, #-N]!`, `ldp [sp], #N`). A `Return` that
//! carries a callee is a sibling call ([`crate::sibcall`]): the epilogue, then
//! `b`.
//!
//! Shrink wrapping: where the prologue and epilogue go is
//! [`crate::shrinkwrap`]'s.

use llrm_object::assembler::Cfi;

use crate::mc::inst::*;
use crate::mc::unwind;
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, MFunc, Op};
use crate::regalloc::Allocation;

/// A block of real instructions.
#[derive(Clone, Debug)]
pub struct Lowered {
    pub label: String,
    pub insts: Vec<Inst>,
}

#[derive(Debug)]
struct Layout {
    size: u64,
    /// The stack pointer moves in the body: objects are addressed from `x29`.
    dynamic: bool,
    /// Each frame object's offset from the stack pointer.
    slots: Vec<u64>,
    /// The register pairs saved, with their offsets.
    saves: Vec<(Save, u64)>,
    frame_record: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
enum Save {
    GprPair(u32, Option<u32>),
    FprPair(u32, Option<u32>),
}

fn layout(
    function: &MFunc,
    allocation: &Allocation,
) -> Layout {
    let outgoing = function.outgoing;
    let mut at = outgoing;
    let mut slots = vec![0; function.slots.len()];
    for (index, slot) in function.slots.iter().enumerate() {
        at = at.next_multiple_of(slot.align.max(1));
        slots[index] = at;
        at += slot.size;
    }
    let locals_end = at.next_multiple_of(16);
    let pair = |list: &[u32]| -> Vec<(u32, Option<u32>)> {
        list.chunks(2).map(|chunk| (chunk[0], chunk.get(1).copied())).collect()
    };
    let mut saves = Vec::new();
    let mut top = locals_end;
    for (first, second) in pair(&allocation.saved_gprs) {
        saves.push((Save::GprPair(first, second), top));
        top += 16;
    }
    for (first, second) in pair(&allocation.saved_fprs) {
        saves.push((Save::FprPair(first, second), top));
        top += 16;
    }
    // A moving stack pointer is addressed from, and put back by, the frame
    // pointer: a leaf with a dynamic alloca too.
    let frame_record = (function.calls || function.dynamic).then(|| {
        let at = top;
        top += 16;
        at
    });
    Layout { size: top, dynamic: function.dynamic, slots, saves, frame_record }
}

impl Layout {
    /// The bytes the first saved pair's store takes off the stack pointer
    /// (`stp [sp, #-N]!`) where the frame is only saved registers and fits the
    /// pair's offset (`AArch64FrameLowering`, the first CSR push).
    fn folded_bump(&self) -> Option<i64> {
        let first = self.saves.first().map(|(_, at)| *at).or(self.frame_record)?;
        (first == 0 && self.size <= 512).then_some(self.size as i64)
    }

    /// The register and the offset from it that address byte `offset` of frame
    /// object `slot` (or of the caller's argument area): the stack pointer, or
    /// the frame pointer where the stack pointer moves.
    fn place(
        &self,
        slot: u32,
        offset: i64,
    ) -> (Gpr, i64) {
        let from_sp = if slot == INCOMING_ARGUMENTS { self.size } else { self.slots[slot as usize] };
        if !self.dynamic {
            return (Gpr::R31, from_sp as i64 + offset);
        }
        // `x29` is where the frame record is: that many bytes above the stack
        // pointer the frame was made with.
        let record = self.frame_record.expect("a moving stack has a frame record");
        (Gpr::FP, from_sp as i64 + offset - record as i64)
    }
}

struct Emitter {
    insts: Vec<Inst>,
}

impl Emitter {
    fn cfi(
        &mut self,
        cfi: Cfi,
    ) {
        self.insts.push(Inst::Cfi(cfi));
    }

    /// `sp = sp + delta` (a multiple of 16).
    fn adjust_sp(
        &mut self,
        delta: i64,
    ) {
        if delta == 0 {
            return;
        }
        let subtract = delta < 0;
        let magnitude = delta.unsigned_abs();
        self.add_to(Gpr::R31, Gpr::R31, subtract, magnitude, Gpr(16));
    }

    /// `rd = rn +- magnitude`, through `scratch` where one add cannot do it.
    fn add_to(
        &mut self,
        rd: Gpr,
        rn: Gpr,
        subtract: bool,
        magnitude: u64,
        scratch: Gpr,
    ) {
        let op = AddSub { subtract, set_flags: false };
        if magnitude < 4096 {
            self.insts.push(Inst::AddSubImm { op, width: Width::X, rd, rn, imm12: magnitude as u16, shift12: false });
        } else if magnitude & 0xFFF == 0 && magnitude >> 12 < 4096 {
            self.insts.push(Inst::AddSubImm {
                op,
                width: Width::X,
                rd,
                rn,
                imm12: (magnitude >> 12) as u16,
                shift12: true,
            });
        } else {
            self.insts.extend(crate::expand_imm::mov_imm(magnitude, 64, scratch));
            // The register form takes the stack pointer through the extended
            // form.
            self.insts.push(Inst::AddSubExtended {
                op,
                width: Width::X,
                rd,
                rn,
                rm: scratch,
                extend: Extend::Uxtx,
                amount: 0,
            });
        }
    }
}

fn accessed_bytes(mem: &Memory) -> i64 {
    match mem {
        Memory::Int { bytes, .. } => i64::from(*bytes),
        Memory::Fp { bytes, .. } => i64::from(*bytes),
    }
}

/// `function`, its registers physical, as real instructions.
/// Where each source variable that lives in a frame object is, in bytes from
/// the canonical frame address (the stack pointer once the prologue has run,
/// plus the frame's size), below it; none where the stack pointer moves in the
/// body.
pub fn variable_offsets(
    function: &MFunc,
    allocation: &Allocation,
) -> Vec<(llrm_mir::MetadataId, i64)> {
    let frame = layout(function, allocation);
    if frame.dynamic {
        return Vec::new();
    }
    function
        .declared
        .iter()
        .map(|&(variable, slot)| (variable, frame.slots[slot as usize] as i64 - frame.size as i64))
        .collect()
}

pub fn lower(
    function: &MFunc,
    allocation: &Allocation,
) -> Result<Vec<Lowered>, String> {
    let frame = layout(function, allocation);
    let wrapped = crate::shrinkwrap::place(function).filter(|_| frame.size > 0);
    let save = wrapped.as_ref().map_or(0, |one| one.save);
    let mut out = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut emitter = Emitter { insts: Vec::new() };
        if index == save {
            prologue(&frame, &mut emitter);
        }
        // Without a placement every return takes the frame down; with one, the
        // Restore's return does, or the blocks that end the frame at
        // their end.
        let framed = wrapped.as_ref().is_none_or(|one| !one.at_end && one.restores.contains(&index));
        for op in &block.ops {
            match op {
                Op::I(inst) => lower_inst(inst, &frame, &mut emitter)?,
                Op::Copy { dst, src } => match (dst, src) {
                    (Reg::Gpr(rd), Reg::Gpr(rm)) => emitter.insts.push(Inst::LogicShifted {
                        op: Logic::Orr,
                        invert: false,
                        width: Width::X,
                        rd: *rd,
                        rn: Gpr::R31,
                        rm: *rm,
                        shift: Shift::Lsl,
                        amount: 0,
                    }),
                    (Reg::Fpr(rd), Reg::Fpr(rn)) => {
                        emitter.insts.push(Inst::FpUnary { op: FpUnary::Mov, size: FpSize::D, rd: *rd, rn: *rn })
                    }
                    _ => return Err("a copy between register classes".into()),
                },
                Op::Imm { dst, bits, value } => emitter.insts.extend(crate::expand_imm::mov_imm(*value, *bits, *dst)),
                Op::Call { callee, .. } => emitter.insts.push(match callee {
                    Callee::Symbol(name) => {
                        Inst::Branch { link: true, target: Target::Symbol { name: name.clone(), addend: 0 } }
                    }
                    Callee::Register(register) => Inst::BranchReg { link: true, rn: *register },
                }),
                Op::Return { tail, .. } => {
                    if framed {
                        epilogue(&frame, &mut emitter);
                    }
                    emitter.insts.push(match tail {
                        None => Inst::Ret { rn: Gpr::LR },
                        Some(Callee::Symbol(name)) => {
                            Inst::Branch { link: false, target: Target::Symbol { name: name.clone(), addend: 0 } }
                        }
                        Some(Callee::Register(register)) => Inst::BranchReg { link: false, rn: *register },
                    });
                }
                Op::FrameAddress { dst, slot, offset } => {
                    let (base, total) = frame.place(*slot, *offset);
                    emitter.add_to(*dst, base, total < 0, total.unsigned_abs(), Gpr(16));
                }
            }
        }
        if wrapped.as_ref().is_some_and(|one| one.at_end && one.restores.contains(&index)) {
            // Before the branches that leave.
            let at = emitter
                .insts
                .iter()
                .rposition(|inst| {
                    !matches!(
                        inst,
                        Inst::Branch { link: false, .. }
                            | Inst::BranchCond { .. }
                            | Inst::CompareBranch { .. }
                            | Inst::TestBranch { .. }
                    )
                })
                .map_or(0, |i| i + 1);
            let mut tail = Emitter { insts: Vec::new() };
            epilogue(&frame, &mut tail);
            emitter.insts.splice(at..at, tail.insts);
        }
        out.push(Lowered { label: block.label.clone(), insts: emitter.insts });
    }
    Ok(out)
}

fn prologue(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    let bump = frame.folded_bump();
    if bump.is_none() && frame.size > 0 {
        emitter.adjust_sp(-(frame.size as i64));
        emitter.cfi(Cfi::DefCfaOffset { offset: frame.size as i64 });
    }
    save_registers(frame, emitter, true, bump);
    if let Some(at) = frame.frame_record {
        emitter.add_to(Gpr::FP, Gpr::R31, false, at, Gpr(16));
        emitter.cfi(Cfi::DefCfa { register: unwind::FP, offset: (frame.size - at) as i64 });
    }
}

fn epilogue(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    // A moving stack pointer is put back from the frame pointer first.
    if let (true, Some(record)) = (frame.dynamic, frame.frame_record) {
        emitter.add_to(Gpr::R31, Gpr::FP, true, record, Gpr(16));
    }
    // The frame address is the stack pointer's again, before the registers
    // come back from under it.
    if frame.frame_record.is_some() {
        emitter.cfi(Cfi::DefCfa { register: unwind::SP, offset: frame.size as i64 });
    }
    let bump = frame.folded_bump();
    save_registers(frame, emitter, false, bump);
    if bump.is_none() && frame.size > 0 {
        emitter.adjust_sp(frame.size as i64);
        emitter.cfi(Cfi::DefCfaOffset { offset: 0 });
    }
}

/// The stores (`store`) or loads of the saved pairs and the frame record.
fn save_registers(
    frame: &Layout,
    emitter: &mut Emitter,
    store: bool,
    bump: Option<i64>,
) {
    let mut accesses: Vec<(Memory, Option<Memory>, u64, i64)> = Vec::new();
    for (save, offset) in &frame.saves {
        accesses.push(match *save {
            Save::GprPair(a, b) => (
                Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(a) },
                b.map(|b| Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(b) }),
                *offset,
                8,
            ),
            Save::FprPair(a, b) => {
                (Memory::Fp { bytes: 8, reg: Vreg(a) }, b.map(|b| Memory::Fp { bytes: 8, reg: Vreg(b) }), *offset, 8)
            }
        });
    }
    if let Some(at) = frame.frame_record {
        accesses.push((
            Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr::FP },
            Some(Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr::LR }),
            at,
            8,
        ));
    }
    // The access at the lowest address also moves the stack pointer: first in
    // the prologue (`stp [sp, #-N]!`), last in the epilogue (`ldp [sp], #N`).
    if bump.is_some() && !store {
        accesses.rotate_left(1);
    }
    for (first, second, offset, _) in accesses {
        let moves = bump.filter(|_| offset == 0);
        let addr = match moves {
            Some(by) if store => Addr::Pre { base: Gpr::R31, imm: -by },
            Some(by) => Addr::Post { base: Gpr::R31, imm: by },
            None => Addr::Offset { base: Gpr::R31, imm: offset as i64 },
        };
        if moves.is_some() {
            let inst = match (second, store) {
                (Some(second), true) => Inst::StorePair { first, second, addr },
                (Some(second), false) => Inst::LoadPair { first, second, addr },
                (None, true) => Inst::Store { mem: first, addr, mmo: MemOperand::default() },
                (None, false) => Inst::Load { mem: first, addr, mmo: MemOperand::default() },
            };
            emitter.insts.push(inst);
            declare(frame, emitter, store, &first, second.as_ref(), offset, true);
            continue;
        }
        // A pair reaches 504 bytes from the base; further, the address is made.
        let (base, imm) = if offset <= 504 {
            (Gpr::R31, offset as i64)
        } else {
            emitter.add_to(Gpr(16), Gpr::R31, false, offset, Gpr(17));
            (Gpr(16), 0)
        };
        let addr = if offset <= 504 { addr } else { Addr::Offset { base, imm } };
        let _ = imm;
        let inst = match (second, store) {
            (Some(second), true) => Inst::StorePair { first, second, addr },
            (Some(second), false) => Inst::LoadPair { first, second, addr },
            (None, true) => Inst::Store { mem: first, addr, mmo: MemOperand::default() },
            (None, false) => Inst::Load { mem: first, addr, mmo: MemOperand::default() },
        };
        emitter.insts.push(inst);
        declare(frame, emitter, store, &first, second.as_ref(), offset, false);
    }
}

/// The call frame information of a save (`store`) or a restore of `first` and
/// `second` at `offset` from the bottom of the frame; `moves` where the access
/// also moved the stack pointer.
fn declare(
    frame: &Layout,
    emitter: &mut Emitter,
    store: bool,
    first: &Memory,
    second: Option<&Memory>,
    offset: u64,
    moves: bool,
) {
    let number = |memory: &Memory| match memory {
        Memory::Int { reg, .. } => reg.0 as u16,
        Memory::Fp { reg, .. } => 64 + reg.0 as u16,
    };
    let size = frame.size as i64;
    if store && moves {
        emitter.cfi(Cfi::DefCfaOffset { offset: size });
    }
    for (index, memory) in std::iter::once(first).chain(second).enumerate() {
        let register = number(memory);
        let cfi = if store {
            Cfi::Offset { register, offset: offset as i64 + 8 * index as i64 - size }
        } else {
            Cfi::Restore { register }
        };
        emitter.cfi(cfi);
    }
    if !store && moves {
        emitter.cfi(Cfi::DefCfaOffset { offset: 0 });
    }
}

/// `inst` with a frame object address turned into an offset from the stack
/// pointer, through a scratch register where the offset does not encode.
fn lower_inst(
    inst: &Inst,
    frame: &Layout,
    emitter: &mut Emitter,
) -> Result<(), String> {
    let mut inst = inst.clone();
    match &mut inst {
        Inst::Load { mem, addr, .. } | Inst::Store { mem, addr, .. } => {
            if let Addr::Slot { slot, offset } = *addr {
                let bytes = accessed_bytes(mem);
                let (base, total) = frame.place(slot, offset);
                let scaled = total >= 0 && total % bytes == 0 && total / bytes < 4096;
                if scaled || (-256..256).contains(&total) {
                    *addr = Addr::Offset { base, imm: total };
                } else {
                    emitter.add_to(Gpr(16), base, total < 0, total.unsigned_abs(), Gpr(17));
                    *addr = Addr::Offset { base: Gpr(16), imm: 0 };
                }
            }
        }
        Inst::LoadPair { addr, .. } | Inst::StorePair { addr, .. } => {
            if let Addr::Slot { .. } = addr {
                return Err("a frame object in a register pair".into());
            }
        }
        _ => {}
    }
    emitter.insts.push(inst);
    Ok(())
}
