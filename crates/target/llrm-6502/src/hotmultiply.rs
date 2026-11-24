//! Whether a module's long multiplies are run often enough to link the speed
//! multiply.
//!
//! # How it works
//! `__mulsi3_fast` stops its walk of the multiplier after the top set bit
//! (llvm-mos's `__mulsi3`); the compact `__mulsi3` always walks 32 bits. A call
//! saves [`SAVED_CLOCKS`] on average over multipliers of one to four bytes, the
//! routine is [`fast_bytes`] longer, so a site
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

/// Bytes of `__mulsi3_fast` over `__mulsi3`, read from the runtime's text with
/// what each links (`mulimage::library_bytes`).
pub fn fast_bytes() -> f64 {
    static BYTES: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *BYTES.get_or_init(|| {
        f64::from(crate::mulimage::library_bytes("__mulsi3_fast"))
            - f64::from(crate::mulimage::library_bytes("__mulsi3"))
    })
}
/// Clocks a call saves on average
/// (`the_fast_multiply_saves_what_hotmultiply_prices`, within 10%: sim65 adds a
/// clock a page crossing, so it moves with where the runtime's text starts: 586
/// after the divide routines shrank, 553 to 607 with the 32-bit calls taking
/// their arguments in cells; 550 keeps phi_slot_unknown_arm at -Os on the
/// compact routine, 15 B under the fast one, 2026-11-15).
pub const SAVED_CLOCKS: f64 = 550.0;
/// Static runs from which a module's multiplies are taken to run.
pub const MIN_RUNS: f64 = 3.0;

/// Whether the 32-bit multiplies of `module` count [`MIN_RUNS`] runs together.
pub fn hot(module: &Module) -> bool {
    runs(module) >= MIN_RUNS
}

/// Whether, at speed, the multiplies of `module` run often enough that the
/// clocks the speed routine saves reach `clocks_per_byte` for each byte it
/// adds: the rate every speed form of the target is taken at
/// (`select::speed_division16_pays`). One run of a multiply repays it
/// (uint64_to_float: the module's one multiply ran the compact routine, 2,685
/// clocks against llvm-mos's early-exit walk's 1,977.)
pub fn hot_at_speed(module: &Module) -> bool {
    runs(module) * SAVED_CLOCKS >= fast_bytes() * f64::from(crate::alloc::Tuning::SPEED.clocks_per_byte)
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
