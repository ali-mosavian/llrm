//! A restore of the condition codes from the register that still holds them is dropped:
//! `cmp.l (%a1), %d3; move.w %sr, %d4; scs %d5; move.w %d4, %ccr; bls` is the same without the
//! `move.w %d4, %ccr` (`scs` leaves the codes alone).
//!
//! # How it works
//! [`drop_redundant_restores`] runs on the allocated function. Walking a block, the register a
//! `CCR_SAVE` wrote holds the codes until an instruction sets them again (a flag setter, a copy
//! or load that becomes a `move`, a call) or writes that register. A `CCR_RESTORE` from it while
//! it does is a no-op and goes.
//!
//! DEVIATE: the pin keeps both copies (`copyPhysReg` of `$ccr` is never pruned; the emitter
//! saves where the DAG order had a setter between, and a later sink may move it out). Reason:
//! `move to ccr` is 12 clk at 68000, a restore of nothing. Exit: 2026-12-10, when the shared
//! flow's copy propagation owns `$ccr` copies.

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Kind, Operand};

use crate::flags::{root, written};
use crate::mfunc::MFunction;
use crate::regalloc::Assignment;

/// Drops each `CCR_RESTORE` of codes the CPU still holds. Returns how many.
pub fn drop_redundant_restores(
    function: &mut MFunction,
    assignment: &Assignment,
) -> usize {
    let mut dropped = 0;
    for block in &mut function.blocks {
        let mut held: Option<RegId> = None;
        let mut at = 0;
        while at < block.insts.len() {
            let inst = &block.insts[at];
            match inst.kind {
                Kind::Op("CCR_SAVE") => {
                    held = match inst.operands.first() {
                        Some(Operand::Def(register)) => Some(root(*register, assignment)),
                        _ => None,
                    };
                }
                Kind::Op("CCR_RESTORE") => {
                    let from = inst.operands.iter().find_map(|operand| match operand {
                        Operand::Use(register) => Some(root(*register, assignment)),
                        _ => None,
                    });
                    if held.is_some() && held == from {
                        block.insts.remove(at);
                        dropped += 1;
                        continue;
                    }
                    // the codes are the register's again
                    held = from;
                }
                _ => {
                    let clobbers_codes = inst.sets_flags() || inst.call || written(inst, assignment).is_some();
                    let rewrites_held = held.is_some_and(|register| {
                        inst.operands
                            .iter()
                            .any(|operand| matches!(operand, Operand::Def(def) if root(*def, assignment) == register))
                    });
                    if clobbers_codes || rewrites_held {
                        held = None;
                    }
                }
            }
            at += 1;
        }
    }
    dropped
}
