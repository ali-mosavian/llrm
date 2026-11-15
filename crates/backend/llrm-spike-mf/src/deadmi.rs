//! An instruction nothing reads is gone, with the phis and chains that only it
//! kept alive: `DeadMachineInstructionElim` over the machine function.
//! Translated from
//! [DeadMachineInstructionElim.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp)
//! (`isDead`, `runOnMachineFunction`); a phi is an instruction there.
//!
//! An instruction goes when every register it defines is virtual and unread, it
//! has no side effect (a call, a store, a volatile access, a terminator), and
//! the flags it sets are read by no one. Its operands then lose a reader, so
//! the walk repeats until nothing goes (the pin does one walk, in post order).
//!
//! DEVIATE: the flags are one register here (`Inst::sets_flags`), so their
//! liveness is solved over the blocks; the pin tracks each physical register.
//!
//! # Example
//! `trunc (phi i64 (shl ..) (lshr ..))` at m68k: the phi of the high word has
//! no reader, and nor have the selects that fed it.

use crate::mfunc::{Inst, MFunction, Operand, Reg};
use crate::regalloc::successors;

/// Remove what nothing reads; whether anything went.
pub fn dead_machine_instruction_elim<C>(function: &mut MFunction<C>) -> bool {
    let mut readers = vec![0_u32; function.classes.len()];
    for block in &function.blocks {
        for operand in block.insts.iter().flat_map(|inst| &inst.operands) {
            if let Operand::Use(register) = operand {
                adjust(&mut readers, *register, 1);
            }
        }
        for &(_, register) in block.phis.iter().flat_map(|phi| &phi.incoming) {
            adjust(&mut readers, register, 1);
        }
    }
    let successors = successors(function);
    let mut any = false;
    loop {
        let flags_out = flags_live_out(function, &successors);
        let mut gone = false;
        for (at, block) in function.blocks.iter_mut().enumerate().rev() {
            let mut live = flags_out[at];
            let mut index = block.insts.len();
            while index > 0 {
                index -= 1;
                let inst = &block.insts[index];
                if is_dead(inst, &readers) && !(inst.sets_flags() && live) {
                    for operand in &inst.operands {
                        if let Operand::Use(register) = operand {
                            adjust(&mut readers, *register, -1);
                        }
                    }
                    block.insts.remove(index);
                    gone = true;
                    continue;
                }
                live &= !inst.sets_flags();
                live |= inst.reads_flags();
            }
            let before = block.phis.len();
            block.phis.retain(|phi| {
                let unread = matches!(phi.result, Reg::Virt(value) if readers[value as usize] == 0);
                if unread {
                    for &(_, register) in &phi.incoming {
                        adjust(&mut readers, register, -1);
                    }
                }
                !unread
            });
            gone |= block.phis.len() != before;
        }
        if !gone {
            return any;
        }
        any = true;
    }
}

fn adjust(
    readers: &mut [u32],
    register: Reg,
    delta: i32,
) {
    if let Reg::Virt(value) = register {
        readers[value as usize] = readers[value as usize].saturating_add_signed(delta);
    }
}

/// `MachineInstr::isDead`: only virtual registers defined, none read, and
/// nothing the instruction does besides.
fn is_dead(
    inst: &Inst,
    readers: &[u32],
) -> bool {
    if inst.call || inst.terminator || inst.barrier || inst.may_store || inst.side_effects {
        return false;
    }
    let mut defined = false;
    for operand in &inst.operands {
        if let Operand::Def(register) = operand {
            match register {
                Reg::Virt(value) if readers[*value as usize] == 0 => defined = true,
                _ => return false,
            }
        }
    }
    defined
}

/// Whether the flags are read after the end of each block.
fn flags_live_out<C>(
    function: &MFunction<C>,
    successors: &[Vec<usize>],
) -> Vec<bool> {
    let live_in = |block: &crate::mfunc::Block, out: bool| {
        block.insts.iter().rev().fold(out, |live, inst| (live && !inst.sets_flags()) || inst.reads_flags())
    };
    let mut out = vec![false; function.blocks.len()];
    let mut inward = vec![false; function.blocks.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for at in (0..function.blocks.len()).rev() {
            let now = successors[at].iter().any(|&next| inward[next]);
            let entry = live_in(&function.blocks[at], now);
            if now != out[at] || entry != inward[at] {
                (out[at], inward[at]) = (now, entry);
                changed = true;
            }
        }
    }
    out
}
