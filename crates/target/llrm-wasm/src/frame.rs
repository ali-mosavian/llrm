//! The frame of a function: `PrologEpilogInserter`'s object layout,
//! `WebAssemblyRegisterInfo::eliminateFrameIndex`
//! and `WebAssemblyFrameLowering::emitPrologue` / `emitEpilogue`, with the
//! stack pointer as a register (the pin makes it a physical one and
//! `ReplacePhysRegs` renames it a few passes later).
//!
//! # How it works
//! Objects are laid out downwards from the incoming stack pointer in
//! frame-index order; the frame is that size rounded to the 16-byte stack
//! alignment. The prologue reads `__stack_pointer`, subtracts the frame and,
//! unless the leaf function fits the 128-byte red zone, writes it back; each
//! return writes the incoming value back. A frame index becomes an offset from
//! the new stack pointer.
//!
//! A variable-sized object (`hasVarSizedObjects`: a non-static alloca) makes
//! a frame pointer: a copy of the stack pointer after the prologue that
//! addresses the frame and, where the stack pointer is written back, restores
//! it. The `ADJCALLSTACKDOWN`/`UP` pair around an alloca is dropped, the end
//! writing the new stack pointer back (`eliminateCallFramePseudoInstr`).
//!
//! DEVIATE W11: not translated are the frame pointer of a taken frame address
//! and of stack maps, the base pointer for over-aligned objects,
//! `WasmLocal` stack objects and the libcall thread context. Reason: each
//! needs IR the slice does not reach. A function that needs one is refused by
//! name. Exit: no later than 2026-11-30.
//!
//! # See also
//! `WebAssemblyFrameLowering.cpp` (`emitPrologue` L254, `emitEpilogue` L330),
//! `WebAssemblyRegisterInfo.cpp` (`eliminateFrameIndex` L52) and
//! `PrologEpilogInserter.cpp` (`AdjustStackOffset`), pin d1106deb71cc.

use llrm_support::hash::HashMap;

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::dagisel::post_ids as post;
use crate::mir::{Function, Inst, Operand, VReg, ValType, is_argument};

/// `WebAssemblyFrameLowering::RedZoneSize`.
const RED_ZONE: u64 = 128;
/// `TargetFrameLowering::getStackAlignment` of wasm.
const STACK_ALIGN: u64 = 16;

fn align_to(
    value: u64,
    align: u64,
) -> u64 {
    value.div_ceil(align) * align
}

/// `COPY` of a pointer's class.
fn copy_of(pointer: ValType) -> u16 {
    match pointer {
        ValType::I64 => post::COPY_I64,
        _ => post::COPY_I32,
    }
}

fn has(
    inst: &Inst,
    flag: u64,
) -> bool {
    desc(inst.opcode).is_some_and(|desc| desc.has(flag))
}

/// Whether `opcode`'s operand after an immediate is its `addr`, which the
/// offset before it absorbs. `memory.copy` and `memory.fill` follow memory
/// indices with address operands that name no `addr`: folding into the index
/// made `memory.copy 0, 12` of a copy to a local.
fn names_address(opcode: u16) -> bool {
    !matches!(
        opcode,
        op::MEMORY_COPY_A32 | op::MEMORY_COPY_A64 | op::MEMORY_FILL_A32 | op::MEMORY_FILL_A64
    )
}

/// Whether anything changed. `pointer` is the type of an address.
pub fn frame_lowering(
    f: &mut Function,
    pointer: ValType,
) -> Result<bool, String> {
    // `calculateFrameObjectOffsets`: each object below the last, aligned.
    let mut offsets = Vec::with_capacity(f.frame.len());
    let mut size = 0;
    for &(bytes, align) in &f.frame {
        if align > STACK_ALIGN {
            return Err(format!(
                "@{}: an object aligned to {align} needs stack realignment, not supported yet",
                f.name
            ));
        }
        size = align_to(size + bytes, align.max(1));
        offsets.push(-(size as i64));
    }
    let stack_size = align_to(size, STACK_ALIGN);
    for variable in &mut f.variables {
        variable.disp = stack_size as i64 + offsets[variable.frame as usize];
    }
    // The joined `CALL` is a pseudo of this crate, outside the generated
    // descriptions.
    let has_calls = f
        .blocks
        .iter()
        .flatten()
        .any(|inst| matches!(inst.opcode, post::CALL | post::CALL_INDIRECT) || has(inst, flag::CALL));
    // `needsPrologForEH`: a function with a personality and a call keeps the
    // incoming stack pointer to restore after a catch.
    let for_eh = f.personality && has_calls;
    // `hasFPImpl`: a variable-sized object moves the stack pointer by an
    // unknown amount, so the frame is addressed from a frame pointer.
    let has_fp = f.has_var_sized;
    // `needsSPForLocalFrame`: the call-frame pseudos of a dynamic alloca are
    // `adjustsStack`; any other use of the register is explicit.
    let adjusts_stack =
        f.blocks.iter().flatten().any(|inst| matches!(inst.opcode, op::ADJCALLSTACKDOWN | op::ADJCALLSTACKUP));
    let needs_local_frame = stack_size > 0 || adjusts_stack || has_fp || f.sp.is_some();
    if !needs_local_frame && !for_eh {
        return Ok(false);
    }
    // `needsSPWriteback`: not where the red zone serves.
    let writeback = needs_local_frame && !(stack_size <= RED_ZONE && !has_calls);
    let (konst, add, sub, global_get, global_set) = match pointer {
        ValType::I64 => (op::CONST_I64, op::ADD_I64, op::SUB_I64, op::GLOBAL_GET_I64, op::GLOBAL_SET_I64),
        _ => (op::CONST_I32, op::ADD_I32, op::SUB_I32, op::GLOBAL_GET_I32, op::GLOBAL_SET_I32),
    };
    let stack_pointer = f.symbol("__stack_pointer", 0);
    if f.globals.is_empty() {
        f.globals.push(("__stack_pointer".to_string(), pointer));
    }
    // The register `ReplacePhysRegs` makes of `SP32`: the one the selected
    // code already copies to and from, when it does.
    let sp = match f.sp {
        Some(sp) => sp,
        None => {
            let sp = f.new_vreg(pointer);
            f.sp = Some(sp);
            sp
        }
    };
    let fp = has_fp.then(|| f.new_vreg(pointer));
    // `getFrameRegister`: the frame pointer where there is one.
    let base = fp.unwrap_or(sp);

    // The prologue goes below the ARGUMENTs.
    let at = f.blocks[0].iter().take_while(|inst| is_argument(inst.opcode)).count();
    let mut prologue = Vec::new();
    if stack_size == 0 {
        prologue.push(Inst::new(global_get, &[Operand::Def(sp), Operand::Sym(stack_pointer)]));
    } else {
        let (read, amount) = (f.new_vreg(pointer), f.new_vreg(pointer));
        prologue.extend([
            Inst::new(global_get, &[Operand::Def(read), Operand::Sym(stack_pointer)]),
            Inst::new(konst, &[Operand::Def(amount), Operand::Imm(stack_size as i64)]),
            Inst::new(sub, &[Operand::Def(sp), Operand::Use(read), Operand::Use(amount)]),
        ]);
    }
    // `FP` points to the bottom of the fixed-size locals, so frame offsets stay
    // positive.
    if let Some(fp) = fp {
        prologue.push(Inst::new(copy_of(pointer), &[Operand::Def(fp), Operand::Use(sp)]));
    }
    if stack_size > 0 && writeback {
        prologue.push(Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(sp)]));
    }

    // `eliminateFrameIndex`: an address added to a constant that has no other
    // use folds the offset into that constant.
    let mut uses: HashMap<VReg, u32> = HashMap::default();
    let mut constants: HashMap<VReg, (usize, usize, u32)> = HashMap::default();
    for (block, insts) in f.blocks.iter().enumerate() {
        for (at, inst) in insts.iter().enumerate() {
            for operand in &inst.operands {
                match *operand {
                    Operand::Use(reg) => *uses.entry(reg).or_default() += 1,
                    Operand::Def(reg) if inst.opcode == konst => {
                        constants.entry(reg).and_modify(|one| one.2 += 1).or_insert((block, at, 1));
                    }
                    Operand::Def(reg) => {
                        constants.entry(reg).and_modify(|one| one.2 += 1).or_insert((usize::MAX, 0, 1));
                    }
                    _ => {}
                }
            }
        }
    }
    for block in 0..f.blocks.len() {
        for i in 0..f.blocks[block].len() {
            let inst = &f.blocks[block][i];
            let ([Operand::Def(_), first, second], true) = (inst.operands.as_slice(), inst.opcode == add) else {
                continue;
            };
            let (frame, other) = match (*first, *second) {
                (Operand::Frame(index), Operand::Use(reg)) => (index, (2, reg)),
                (Operand::Use(reg), Operand::Frame(index)) => (index, (1, reg)),
                _ => continue,
            };
            let Some(&(def_block, def_at, 1)) = constants.get(&other.1) else { continue };
            if def_block == usize::MAX || uses.get(&other.1) != Some(&1) {
                continue;
            }
            let from_sp = stack_size as i64 + offsets[frame as usize];
            let Some(Operand::Imm(value)) = f.blocks[def_block][def_at].operands.get_mut(1) else { continue };
            *value += from_sp;
            f.blocks[block][i].operands[3 - other.0] = Operand::Use(base);
        }
    }
    // An address operand folds the offset in; anything else adds it to the
    // stack pointer.
    for block in 0..f.blocks.len() {
        let mut i = 0;
        while i < f.blocks[block].len() {
            let Some(operand) = f.blocks[block][i].operands.iter().position(|o| matches!(o, Operand::Frame(_))) else {
                i += 1;
                continue;
            };
            let Operand::Frame(index) = f.blocks[block][i].operands[operand] else { unreachable!() };
            let from_sp = stack_size as i64 + offsets[index as usize];
            let inst = &mut f.blocks[block][i];
            if (has(inst, flag::MAY_LOAD) || has(inst, flag::MAY_STORE)) && operand > 0 && names_address(inst.opcode) {
                if let Operand::Imm(offset) = &mut inst.operands[operand - 1] {
                    *offset += from_sp;
                    inst.operands[operand] = Operand::Use(base);
                    continue;
                }
            }
            if from_sp == 0 {
                inst.operands[operand] = Operand::Use(base);
                continue;
            }
            let (offset, address) = (f.new_vreg(pointer), f.new_vreg(pointer));
            f.blocks[block][i].operands[operand] = Operand::Use(address);
            f.blocks[block]
                .insert(i, Inst::new(add, &[Operand::Def(address), Operand::Use(base), Operand::Use(offset)]));
            f.blocks[block].insert(i, Inst::new(konst, &[Operand::Def(offset), Operand::Imm(from_sp)]));
            i += 2;
        }
    }

    // `emitEpilogue`: before each return.
    if writeback {
        for block in 0..f.blocks.len() {
            let Some(at) = f.blocks[block].iter().position(|inst| has(inst, flag::RETURN)) else { continue };
            // No frame of its own: the pointer itself goes back.
            if stack_size == 0 {
                let back = Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(base)]);
                f.blocks[block].insert(at, back);
                continue;
            }
            let (amount, restored): (VReg, VReg) = (f.new_vreg(pointer), f.new_vreg(pointer));
            let epilogue = [
                Inst::new(konst, &[Operand::Def(amount), Operand::Imm(stack_size as i64)]),
                Inst::new(add, &[Operand::Def(restored), Operand::Use(base), Operand::Use(amount)]),
                Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(restored)]),
            ];
            f.blocks[block].splice(at..at, epilogue);
        }
    }
    // `eliminateCallFramePseudoInstr`: the bracket of a dynamic alloca goes;
    // the end of it writes the new stack pointer back.
    for block in &mut f.blocks {
        let mut i = 0;
        while i < block.len() {
            match block[i].opcode {
                op::ADJCALLSTACKDOWN => {
                    block.remove(i);
                }
                op::ADJCALLSTACKUP => {
                    block.remove(i);
                    if writeback {
                        block.insert(i, Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(sp)]));
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
    }
    f.blocks[0].splice(at..at, prologue);
    Ok(true)
}
