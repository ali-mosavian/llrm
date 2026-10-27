//! Frame lowering: the frame's layout, the prologue and epilogue, and the
//! frame objects turned into stack-pointer offsets
//! (`AArch64FrameLowering::emitPrologue`, `emitEpilogue`,
//! `eliminateFrameIndex`). The frame, from the stack pointer up: outgoing
//! arguments, frame objects, the callee-saved registers in pairs, and the frame
//! record (`x29`, `x30`) at the top. Offsets are from the stack pointer, which
//! does not move in the body.
//!
//! Shrink wrapping (`ShrinkWrap.cpp`): when the blocks that need the frame
//! (a call, a frame object, a callee-saved register) are all dominated by one
//! block other than the entry, and that block's region is left only by
//! returns, the prologue goes at the start of that block and the epilogue
//! before the returns inside the region; the early exits cost nothing.

use crate::mc::inst::*;
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
    let frame_record = function
        .calls
        .then(
            || {
                let at = top;
                top += 16;
                at
            },
        );
    Layout { size: top, dynamic: function.dynamic, slots, saves, frame_record }
}

impl Layout {
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

/// Whether `op` needs the frame: it calls, addresses a frame object or the
/// caller's arguments, or touches a callee-saved register.
fn needs_frame(op: &Op) -> bool {
    match op {
        Op::Call { .. } | Op::FrameAddress { .. } => true,
        Op::I(Inst::Load { addr: Addr::Slot { .. }, .. } | Inst::Store { addr: Addr::Slot { .. }, .. }) => true,
        _ => {
            let (defs, uses) = op.defs_and_uses();
            defs.iter().chain(&uses).any(|register| crate::regalloc::pool::callee_saved(*register))
        }
    }
}

/// `ShrinkWrap`'s save point: the block whose start gets the prologue, if it
/// is not the entry. It is the nearest common dominator of the blocks that
/// need the frame, and its region (the blocks it dominates) must be left by
/// returns only and never branch back to it.
fn save_point(function: &MFunc) -> Option<(usize, Vec<bool>)> {
    let count = function.blocks.len();
    if function.dynamic || count < 2 {
        return None;
    }
    let labels: std::collections::HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, one)| (one.label.as_str(), at)).collect();
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| crate::mfunc::successors_of(block, at, count, &labels))
        .collect();
    // Immediate dominators over the blocks reachable from the entry, by the
    // iterative algorithm on the reverse postorder.
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some((block, next)) = stack.pop() {
        if next < successors[block].len() {
            stack.push((block, next + 1));
            let to = successors[block][next];
            if !seen[to] {
                seen[to] = true;
                stack.push((to, 0));
            }
        } else {
            order.push(block);
        }
    }
    order.reverse();
    let mut position = vec![usize::MAX; count];
    for (at, &block) in order.iter().enumerate() {
        position[block] = at;
    }
    let mut predecessors = vec![Vec::new(); count];
    for &block in &order {
        for &to in &successors[block] {
            predecessors[to].push(block);
        }
    }
    let mut idom: Vec<Option<usize>> = vec![None; count];
    idom[0] = Some(0);
    let mut changed = true;
    while changed {
        changed = false;
        for &block in order.iter().skip(1) {
            let mut new: Option<usize> = None;
            for &pred in &predecessors[block] {
                if idom[pred].is_none() {
                    continue;
                }
                new = Some(match new {
                    None => pred,
                    Some(mut a) => {
                        let mut b = pred;
                        while a != b {
                            while position[a] > position[b] {
                                a = idom[a].expect("reached");
                            }
                            while position[b] > position[a] {
                                b = idom[b].expect("reached");
                            }
                        }
                        a
                    }
                });
            }
            if new != idom[block] {
                idom[block] = new;
                changed = true;
            }
        }
    }
    let dominates = |a: usize, mut b: usize| loop {
        if a == b {
            return true;
        }
        match idom[b] {
            Some(up) if up != b => b = up,
            _ => return false,
        }
    };
    let mut save: Option<usize> = None;
    for &block in &order {
        if !function.blocks[block].ops.iter().any(needs_frame) {
            continue;
        }
        save = Some(match save {
            None => block,
            Some(mut a) => {
                let mut b = block;
                while a != b {
                    while position[a] > position[b] {
                        a = idom[a].expect("reached");
                    }
                    while position[b] > position[a] {
                        b = idom[b].expect("reached");
                    }
                }
                a
            }
        });
    }
    let save = save.filter(|&at| at != 0)?;
    for &block in order.iter().filter(|&&one| dominates(save, one)) {
        for &to in &successors[block] {
            if !dominates(save, to) || to == save {
                return None;
            }
        }
    }
    let mut region = vec![false; count];
    for &block in &order {
        region[block] = dominates(save, block);
    }
    Some((save, region))
}

/// A block that only returns is copied into the blocks that jump or fall to it
/// (`TailDuplicator`'s return blocks), so that a frame-free path can return
/// before the frame is made. The original goes unreachable.
fn duplicate_returns(function: &MFunc) -> MFunc {
    let mut function = function.clone();
    let count = function.blocks.len();
    for target in 1..count {
        let ops = &function.blocks[target].ops;
        if !matches!(ops.as_slice(), [Op::Return { .. }]) {
            continue;
        }
        let label = function.blocks[target].label.clone();
        let copy = ops.clone();
        for at in 0..count {
            if at == target {
                continue;
            }
            let block = &mut function.blocks[at];
            let ends_in_jump = matches!(
                block.ops.last(),
                Some(Op::I(Inst::Branch { link: false, target: Target::Label(to) })) if *to == label
            );
            if ends_in_jump {
                block.ops.pop();
                block.ops.extend(copy.clone());
            } else if at + 1 == target && !matches!(block.ops.last(), Some(Op::Return { .. } | Op::I(Inst::Branch { .. }))) {
                block.ops.extend(copy.clone());
            }
        }
    }
    function
}

/// `function`, its registers physical, as real instructions.
pub fn lower(
    function: &MFunc,
    allocation: &Allocation,
) -> Result<Vec<Lowered>, String> {
    let frame = layout(function, allocation);
    // The copies of the return blocks stay only where they let the frame be made later.
    let duplicated = duplicate_returns(function);
    let (function, save, wrapped) = match save_point(&duplicated).filter(|_| frame.size > 0) {
        Some((at, region)) => (&duplicated, Some(at), Some(region)),
        None => (function, None, None),
    };
    let mut out = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut emitter = Emitter { insts: Vec::new() };
        if index == save.unwrap_or(0) {
            prologue(&frame, &mut emitter);
        }
        let framed = wrapped.as_ref().is_none_or(|region| region[index]);
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
                Op::Return { .. } => {
                    if framed {
                        epilogue(&frame, &mut emitter);
                    }
                    emitter.insts.push(Inst::Ret { rn: Gpr::LR });
                }
                Op::FrameAddress { dst, slot, offset } => {
                    let (base, total) = frame.place(*slot, *offset);
                    emitter.add_to(*dst, base, total < 0, total.unsigned_abs(), Gpr(16));
                }
            }
        }
        out.push(Lowered { label: block.label.clone(), insts: emitter.insts });
    }
    Ok(out)
}

fn prologue(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    emitter.adjust_sp(-(frame.size as i64));
    save_registers(frame, emitter, true);
    if let Some(at) = frame.frame_record {
        emitter.add_to(Gpr::FP, Gpr::R31, false, at, Gpr(16));
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
    save_registers(frame, emitter, false);
    emitter.adjust_sp(frame.size as i64);
}

/// The stores (`store`) or loads of the saved pairs and the frame record.
fn save_registers(
    frame: &Layout,
    emitter: &mut Emitter,
    store: bool,
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
    for (first, second, offset, _) in accesses {
        let addr = Addr::Offset { base: Gpr::R31, imm: offset as i64 };
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
            (None, true) => Inst::Store { mem: first, addr },
            (None, false) => Inst::Load { mem: first, addr },
        };
        emitter.insts.push(inst);
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
        Inst::Load { mem, addr } | Inst::Store { mem, addr } => {
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
