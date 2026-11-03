//! `DeadMachineInstructionElim`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp)):
//! an instruction nothing reads and that has no other effect is removed,
//! bottom up so a chain of them goes in one sweep.
//!
//! The pin runs it before `MachineLICM` and again after the peephole. Here the
//! first run follows `fix_br_table_defaults`, which leaves the guard's check
//! with no reader; it was kept as an `i32.gt_u` and a `drop`.
//!
//! DEVIATE W24: a copy for the wasm MIR. `backend/deadmi.rs` of llrm-codegen
//! is the shared translation, on `LirBody`; the wasm passes (`machine_sink`,
//! `machine_cse`, `machine_licm`) work on their own MIR as well. Exit: with
//! them, when wasm takes the shared body (S4), no later than 2026-11-20.
//!
//! # See also
//! `DeadMachineInstructionElimImpl::{isDead, eliminateDeadMI}`, pin
//! d1106deb71cc.

use crate::machine_sink::is_safe_to_move;
use crate::mir::{Function, Operand};

/// Whether anything changed.
pub fn dead_machine_instruction_elim(f: &mut Function) -> bool {
    let mut uses = vec![0u32; f.num_vregs()];
    for inst in f.blocks.iter().flatten() {
        for operand in &inst.operands {
            if let Operand::Use(reg) = operand {
                uses[*reg as usize] += 1;
            }
        }
    }
    let mut changed = false;
    for block in &mut f.blocks {
        let mut at = block.len();
        while at > 0 {
            at -= 1;
            let inst = &block[at];
            // `isDead`: `SawStore` is true, so nothing that reads memory is.
            let mut saw_store = true;
            if !is_safe_to_move(inst, &mut saw_store) || inst.defs().any(|reg| uses[reg as usize] != 0) {
                continue;
            }
            for operand in &inst.operands {
                if let Operand::Use(reg) = operand {
                    uses[*reg as usize] -= 1;
                }
            }
            block.remove(at);
            changed = true;
        }
    }
    changed
}
