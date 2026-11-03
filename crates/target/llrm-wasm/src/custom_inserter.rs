//! `WebAssemblyTargetLowering::EmitInstrWithCustomInserter` for `MEMCPY` and
//! `MEMSET`: `memory.copy` and `memory.fill` trap on a bad pointer even for no
//! bytes, so a length that is not a constant is tested first, in a triangle
//! around the instruction.
//!
//! # How it works
//! A constant length needs no test: zero drops the instruction, any other is
//! the bare instruction. Otherwise the block is split at the instruction: the
//! head ends in `eqz len; br_if done`, a new block holds the instruction and a
//! `br done`, and the rest of the block moves to `done`, which takes over its
//! successors and the incoming edges of their phis.
//!
//! # See also
//! `WebAssemblyISelLowering.cpp`, pin d1106deb71cc: `LowerMemcpy` (L609),
//! `LowerMemset` (L701).

use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Function, Inst, Operand, PHI, ValType};

/// Lowers every `MEMCPY` and `MEMSET` of `f`.
pub fn lower_memory_pseudos(f: &mut Function) {
    while let Some((block, at)) = find(f) {
        lower(f, block, at);
    }
}

fn find(f: &Function) -> Option<(usize, usize)> {
    let pseudos = [op::MEMCPY_A32, op::MEMCPY_A64, op::MEMSET_A32, op::MEMSET_A64];
    f.blocks
        .iter()
        .enumerate()
        .find_map(|(block, insts)| insts.iter().position(|inst| pseudos.contains(&inst.opcode)).map(|at| (block, at)))
}

fn lower(
    f: &mut Function,
    block: usize,
    at: usize,
) {
    let inst = f.blocks[block][at].clone();
    let (real, int64, length) = match inst.opcode {
        op::MEMCPY_A32 => (op::MEMORY_COPY_A32, false, 4),
        op::MEMCPY_A64 => (op::MEMORY_COPY_A64, true, 4),
        op::MEMSET_A32 => (op::MEMORY_FILL_A32, false, 3),
        _ => (op::MEMORY_FILL_A64, true, 3),
    };
    let Operand::Use(len) = inst.operands[length] else { unreachable!("the length is a register") };
    let constant = f
        .def_use()
        .def(len)
        .and_then(
            |(b, i)| {
                let def = &f.blocks[b][i];
                match (def.opcode, def.operands.get(1)) {
                    (op::CONST_I32 | op::CONST_I64, Some(Operand::Imm(value))) => Some(*value),
                    _ => None,
                }
            },
        );
    match constant {
        Some(0) => {
            f.blocks[block].remove(at);
            return;
        }
        Some(_) => {
            f.blocks[block][at] = Inst { opcode: real, ..inst };
            return;
        }
        None => {}
    }
    let (done, copy) = (block + 2, block + 1);
    // Two blocks after `block`: every block number past it moves up by two.
    for operand in f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
        if let Operand::Block(to) = operand {
            if *to as usize > block {
                *to += 2;
            }
        }
    }
    for to in f.succs.iter_mut().flatten() {
        if *to > block {
            *to += 2;
        }
    }
    let tail = f.blocks[block].split_off(at + 1);
    f.blocks[block].pop();
    let successors = std::mem::take(&mut f.succs[block]);
    let zero = f.new_vreg(ValType::I32);
    let eqz = if int64 { op::EQZ_I64 } else { op::EQZ_I32 };
    f.blocks[block].push(Inst::new(eqz, &[Operand::Def(zero), Operand::Use(len)]));
    f.blocks[block].push(Inst::new(op::BR_IF, &[Operand::Block(done as u32), Operand::Use(zero)]));
    f.blocks.insert(copy, vec![Inst { opcode: real, ..inst }, Inst::new(op::BR, &[Operand::Block(done as u32)])]);
    f.blocks.insert(done, tail);
    f.succs[block] = vec![copy, done];
    f.succs.insert(copy, vec![done]);
    f.succs.insert(done, successors.clone());
    // The successors' phis name the block the edge leaves.
    for &to in &successors {
        for phi in f.blocks[to].iter_mut().take_while(|inst| inst.opcode == PHI) {
            for operand in &mut phi.operands {
                if *operand == Operand::Block(block as u32) {
                    *operand = Operand::Block(done as u32);
                }
            }
        }
    }
}
