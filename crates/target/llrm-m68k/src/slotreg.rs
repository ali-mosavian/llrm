//! Spill slots in free address registers (DEVIATE SLOT-REG,
//! `llrm_spike_mf::slotreg`).
//!
//! A frame slot a spill store and reload share lives in `a0`..`a5` where one is
//! free over its range: `movea.l %d0,%a0` and `move.l %a0,%d1` are 2 B each
//! against the frame store and reload's 4 B (6 B as `movem`), and 4 clocks
//! against 12 and 16 on the 68000. A reload into a data register is a `move`
//! and sets the condition codes, so a slot is moved only where the codes are
//! dead after each of its reloads ([`crate::spillmove`]).

use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Inst, Operand, Reg};

use crate::mfunc::MFunction;
use crate::regalloc::{Assignment, M68kRegisters};
use crate::{REGISTER_INFO, reg, spillmove};

/// Moves the slots of `slots` into address registers; returns those moved.
pub fn slots_in_address_registers(
    function: &mut MFunction,
    assignment: &Assignment,
    slots: &[i32],
) -> Vec<i32> {
    let live = spillmove::live_in(function);
    let successors = successors(function);
    let dead_after = |function: &MFunction, b: usize, i: usize| match function.blocks[b].insts[i + 1..]
        .iter()
        .find_map(spillmove::touches)
    {
        Some(reads) => !reads,
        None => !successors[b].iter().any(|&next| live[next]),
    };
    let hosts = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];
    let callee_saved = crate::frame::callee_saved();
    let mentioned = |function: &MFunction, host: llrm_lir::registers::RegId| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.insts)
            .any(
                |inst| inst.operands.iter().any(|one| match one {
                    Operand::Def(Reg::Phys(id)) | Operand::Use(Reg::Phys(id)) => REGISTER_INFO.root(*id) == host,
                    Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => {
                        REGISTER_INFO.root(assignment.0[*v as usize]) == host
                    }
                    _ => false,
                }),
            )
    };
    llrm_spike_mf::slotreg::slots_to_registers::<M68kRegisters>(
        function,
        assignment,
        &REGISTER_INFO,
        slots,
        &hosts,
        32,
        dead_after,
        // a callee-saved host is saved and restored: 4 B, so it takes three
        // accesses to pay
        |function, host, accesses| !callee_saved.contains(&host) || mentioned(function, host) || accesses >= 3,
        // `movea.l` leaves the codes alone, a `move` into a data register sets
        // them
        |inst: &mut Inst, into_host: bool| inst.sets_flags = !into_host,
    )
}
