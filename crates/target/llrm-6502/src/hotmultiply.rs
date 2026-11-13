//! Whether a module's long multiplies are run often enough to link the speed
//! multiply.
//!
//! # How it works
//! `__mulsi3_fast` walks the multiplier a byte at a time and stops at its last
//! set byte; the compact `__mulsi3` always walks 32 bits. A call saves
//! [`SAVED_CLOCKS`] on average over multipliers of one to four bytes (1323,
//! 851, 379 and -93 clocks), the routine is [`FAST_BYTES`] longer, so a site
//! that runs repays it at once. The static count cannot tell a site that runs
//! from one that is dead after folding (an exported function nothing calls
//! counts once: `mul_words_to_long`, `float_in_memory_loop` and
//! `struct_with_i64` paid 19 B for 0 clocks), so the speed routine is called
//! only where the sites count [`MIN_RUNS`] together, as `hotdivision` counts:
//! 10 per level of loop depth, times the runs of the function.
//!
//! DEVIATE (6502-MUL-1): not in the pin, whose `__mulsi3` is one routine.
//! Reason: the compact routine in a 200 trip loop cost mul_constants 20.7M
//! clocks against 16.8M, overflow_builtins 720k against 426k. Exit: a profile
//! says how often a site runs (2026-11-20).

use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{GlobalKind, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::types::TypeId;

use crate::staticlocals::{block_depths, call_weights, callee_intrinsic, weight};

/// Bytes of `__mulsi3_fast` over `__mulsi3` (`mos_rt.s`).
pub const FAST_BYTES: f64 = 19.0;
/// Clocks a call saves on average
/// (`the_fast_multiply_saves_what_hotmultiply_prices`).
pub const SAVED_CLOCKS: f64 = 615.0;
/// Static runs from which a module's multiplies are taken to run.
pub const MIN_RUNS: f64 = 3.0;

/// Whether the 32-bit multiplies of `module` count [`MIN_RUNS`] runs together.
pub fn hot(module: &Module) -> bool {
    runs(module) >= MIN_RUNS
}

/// The static runs of the 32-bit multiplies of `module`: 10 per level of loop
/// depth, times the runs of the function.
pub fn runs(module: &Module) -> f64 {
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
                let operand_type = |at: usize| -> Option<TypeId> {
                    match instruction.operands.get(at)? {
                        Operand::Value(value) => Some(function.value(*value).ty),
                        Operand::Constant(id) => Some(module.context.get(*id).ty),
                        Operand::Block(_) => None,
                    }
                };
                let product = match instruction.opcode {
                    Opcode::Binary(BinaryOp::Mul) => true,
                    Opcode::Call(_) => matches!(
                        callee_intrinsic(module, function, inst),
                        Some(Intrinsic::WithOverflow { op: BinaryOp::Mul, .. })
                    ),
                    _ => false,
                };
                let long = product && operand_type(0).is_some_and(|ty| module.context.types.int_bits(ty) == Some(32));
                if long {
                    let at = function.parent(inst).and_then(|b| depths.get(&llrm_analysis::cfg::id(b))).copied();
                    runs += called * weight(at.unwrap_or(0));
                }
            }
        }
    }
    runs
}
