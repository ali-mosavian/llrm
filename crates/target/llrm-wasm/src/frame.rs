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
//! DEVIATE: not translated are the frame pointer (`hasFP`: variable-sized
//! objects, taken frame address), the base pointer for over-aligned objects,
//! `WasmLocal` stack objects, stack maps and the libcall thread context.
//! Reason: each needs IR the slice does not reach. A function that needs one is
//! refused by name. Exit: with the alloca forms that use them.
//!
//! # See also
//! `WebAssemblyFrameLowering.cpp` (`emitPrologue` L254, `emitEpilogue` L330),
//! `WebAssemblyRegisterInfo.cpp` (`eliminateFrameIndex` L52) and
//! `PrologEpilogInserter.cpp` (`AdjustStackOffset`), pin d1106deb71cc.

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

fn has(
    inst: &Inst,
    flag: u64,
) -> bool {
    desc(inst.opcode).is_some_and(|desc| desc.has(flag))
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
    if stack_size == 0 {
        return Ok(false);
    }
    // The joined `CALL` is a pseudo of this crate, outside the generated
    // descriptions.
    let has_calls = f
        .blocks
        .iter()
        .flatten()
        .any(|inst| matches!(inst.opcode, post::CALL | post::CALL_INDIRECT) || has(inst, flag::CALL));
    let writeback = !(stack_size <= RED_ZONE && !has_calls);
    let (konst, add, sub, global_get, global_set) = match pointer {
        ValType::I64 => (op::CONST_I64, op::ADD_I64, op::SUB_I64, op::GLOBAL_GET_I64, op::GLOBAL_SET_I64),
        _ => (op::CONST_I32, op::ADD_I32, op::SUB_I32, op::GLOBAL_GET_I32, op::GLOBAL_SET_I32),
    };
    let stack_pointer = f.symbol("__stack_pointer", 0);
    if f.globals.is_empty() {
        f.globals.push(("__stack_pointer".to_string(), pointer));
    }
    let sp = f.new_vreg(pointer);

    // The prologue goes below the ARGUMENTs.
    let at = f.blocks[0].iter().take_while(|inst| is_argument(inst.opcode)).count();
    let (read, amount) = (f.new_vreg(pointer), f.new_vreg(pointer));
    let mut prologue = vec![
        Inst::new(global_get, &[Operand::Def(read), Operand::Sym(stack_pointer)]),
        Inst::new(konst, &[Operand::Def(amount), Operand::Imm(stack_size as i64)]),
        Inst::new(sub, &[Operand::Def(sp), Operand::Use(read), Operand::Use(amount)]),
    ];
    if writeback {
        prologue.push(Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(sp)]));
    }

    // `eliminateFrameIndex`: an address operand folds the offset in; anything
    // else adds it to the stack pointer.
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
            if (has(inst, flag::MAY_LOAD) || has(inst, flag::MAY_STORE)) && operand > 0 {
                if let Operand::Imm(offset) = &mut inst.operands[operand - 1] {
                    *offset += from_sp;
                    inst.operands[operand] = Operand::Use(sp);
                    continue;
                }
            }
            if from_sp == 0 {
                inst.operands[operand] = Operand::Use(sp);
                continue;
            }
            let (offset, address) = (f.new_vreg(pointer), f.new_vreg(pointer));
            f.blocks[block][i].operands[operand] = Operand::Use(address);
            f.blocks[block].insert(i, Inst::new(add, &[Operand::Def(address), Operand::Use(sp), Operand::Use(offset)]));
            f.blocks[block].insert(i, Inst::new(konst, &[Operand::Def(offset), Operand::Imm(from_sp)]));
            i += 2;
        }
    }

    // `emitEpilogue`: before each return.
    if writeback {
        for block in 0..f.blocks.len() {
            let Some(at) = f.blocks[block].iter().position(|inst| has(inst, flag::RETURN)) else { continue };
            let (amount, restored): (VReg, VReg) = (f.new_vreg(pointer), f.new_vreg(pointer));
            let epilogue = [
                Inst::new(konst, &[Operand::Def(amount), Operand::Imm(stack_size as i64)]),
                Inst::new(add, &[Operand::Def(restored), Operand::Use(sp), Operand::Use(amount)]),
                Inst::new(global_set, &[Operand::Sym(stack_pointer), Operand::Use(restored)]),
            ];
            f.blocks[block].splice(at..at, epilogue);
        }
    }
    f.blocks[0].splice(at..at, prologue);
    Ok(true)
}
