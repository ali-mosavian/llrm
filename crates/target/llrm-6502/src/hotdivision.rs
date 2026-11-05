//! Whether a module's long divisions are run often enough to link the wide
//! speed routine.
//!
//! # How it works
//! The wide speed division (`udivmod32_wide`, `mos_rt.s`) is 141 bytes
//! over the compact one: 423 clocks at 3 a byte. One call saves a few hundred
//! clocks in the best shape and is slower in some (`70000 % 300`, +13%), so a
//! site pays from about [`HOT_RUNS`] runs. A site counts as `staticlocals`
//! counts: 10 per level of loop depth, times the runs of its function (the
//! call sites in the module, one where it has none). A divisor known to be a
//! byte has its own entry that never links the wide routine, and counts for
//! nothing.
//!
//! DEVIATE (6502-DIV-1): not in the pin, whose `__udivXi3` is one routine.
//! Reason: `compound_assignment_types` divides once, cold, and the wide path
//! cost it +139 B for -9 clocks. Exit: a profile says how often a site runs
//! (2026-11-20).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{GlobalKind, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::staticlocals::{block_depths, call_weights, weight};

/// Runs of a division site that repay the wide routine's bytes.
pub const HOT_RUNS: f64 = 10.0;

/// Whether the 32-bit divisions of `module` that are not by a known byte run
/// [`HOT_RUNS`] times together.
pub fn hot(module: &Module) -> bool {
    let calls = call_weights(module);
    let mut runs = 0.0;
    for (index, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.entry().is_none() {
            continue;
        }
        let depths = block_depths(function);
        let called = calls.get(&index).copied().unwrap_or(1.0);
        for block in function.layout() {
            for inst in function.iter_block(*block) {
                let instruction = function.instruction(inst);
                let Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) =
                    instruction.opcode
                else {
                    continue;
                };
                let long = instruction
                    .result
                    .is_some_and(|result| module.context.types.int_bits(function.value(result).ty) == Some(32));
                let byte = matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if matches!(module.context.get(*id).kind, ConstantKind::Int(1..=255))
                );
                if long && !byte {
                    let at = function.parent(inst).and_then(|b| depths.get(&llrm_analysis::cfg::id(b))).copied();
                    runs += called * weight(at.unwrap_or(0));
                }
            }
        }
    }
    runs >= HOT_RUNS
}
