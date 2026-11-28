//! Whether a module's long divisions are run often enough to link the speed
//! division with the pin-structure wide path.
//!
//! # How it works
//! The speed division's pin-structure wide path (`udivmod32_wide`, `mos_rt.s`)
//! is [`WIDE_BYTES`] over the small one (`udivmod32_fast_small`, the first
//! speed variant): [`WIDE_BYTES`] x [`CLOCKS_PER_BYTE`] = 423 clocks. A call
//! saves [`SAVED_CLOCKS`] on average over ten wide shapes (measured with
//! `timeit.py`, new against old: 3000009 / 1000003 saves 609, 70000 / 300 costs
//! 169, 4000000000 / 65537 saves 0, the ten sum to 1824), so a site repays the
//! bytes from `423 / 182`, 3 runs. The cold calls of the speed levels link the
//! lean body (`udivmod32_fast_lean`, 31 B under the small one): the wide path
//! is 172 B over it and repays from 3 runs too (`172 x 3 / 182` = 2.8). A site
//! counts as `staticlocals` counts: 10 per level of loop depth, times the runs
//! of its function (the call sites in the module, one where it has none). A
//! divisor known to be a byte has its own entry that never links either wide
//! path, and counts for nothing. The compact `__divsi3` is no cold choice: it
//! has no byte-divisor path and one `40000 / -3` costs it 3195 clocks more.
//!
//! DEVIATE (6502-DIV-1): not in the pin, whose `__udivXi3` is one routine.
//! Reason: `compound_assignment_types` divides once, cold, and the new wide
//! path cost it +139 B for -9 clocks. Exit: a profile says how often a site
//! runs (2026-11-20).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{GlobalKind, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::staticlocals::{block_depths, call_weights, weight};

/// Bytes of the pin-structure wide path over the small one (`mos_rt.s`).
pub const WIDE_BYTES: f64 = 141.0;
/// What a byte is worth in clocks (6502-RATE, provisional).
pub const CLOCKS_PER_BYTE: f64 = 3.0;
/// Clocks a call saves on average over the wide shapes (see above).
pub const SAVED_CLOCKS: f64 = 182.0;

/// Runs of a division site that repay the wide path's bytes.
pub fn hot_runs() -> f64 {
    (WIDE_BYTES * CLOCKS_PER_BYTE / SAVED_CLOCKS).ceil()
}

/// Whether the 32-bit divisions of `module` that are not by a known byte run
/// [`hot_runs`] times together.
pub fn hot(module: &Module) -> bool {
    runs(module) >= hot_runs()
}

/// What a call saves at size, in clocks, when the compact `__divsi3` family (32
/// bits a call whatever the quotient) becomes the speed family with the small
/// wide path, averaged over six shapes: `__modsi3` 4333/984, 4159/964,
/// 4072/1531, 4072/1436, 3898/934, 4793/3249 (sim65).
pub const SAVED_AT_SIZE: f64 = 2700.0;

/// The static runs of the 32-bit divisions of `module` that are not by a known
/// byte: 10 per level of loop depth, times the runs of the function.
pub fn runs(module: &Module) -> f64 {
    runs_of_width(module, 32, None)
}

/// [`runs`] with each block weighing what `weights::block_weights` says (a
/// counted loop its trips), the units the module's priced clocks are in. A
/// static ten trips a loop put the saving of 200 trips of divides against
/// the clocks of 200 (div_constants_long -Os: 2.9% of its clocks, 54% run).
pub fn runs_priced(
    module: &Module,
    weights: &std::collections::HashMap<usize, Vec<u64>>,
) -> f64 {
    runs_of_width(module, 32, Some(weights))
}

/// The shapes the 16-bit divisions are priced over (dividend, divisor): a
/// byte divisor of 6 bits and of 2, a wide one of a quotient of 8 bits, a
/// divisor of 1000, and four of a quotient under 6 bits (a ratio near 1:
/// Euclid's steps), where the speed routine shifts the divisor up to the
/// dividend.
pub const SHAPES_16: [(u16, u16); 8] =
    [(60707, 37), (40000, 300), (65535, 2), (9999, 1000), (50000, 31000), (2100, 1500), (1500, 360), (300, 60)];

/// The clocks of `routine` (a word divide of `mos_rt.s`) on `shape`, by running
/// its text (`routine_clocks`).
fn clocks_16(
    cpu: crate::dagisel::hooks::Cpu,
    routine: &str,
    (a, b): (u16, u16),
) -> f64 {
    let cells = [(2, a as u8), (3, (a >> 8) as u8), (4, b as u8), (5, (b >> 8) as u8)];
    let run = crate::routine_clocks::run(cpu, routine, &cells).unwrap_or_else(|e| panic!("{routine} on {a}/{b}: {e}"));
    run.clocks as f64
}

/// What a call of the compact 16-bit division (`__udivhi3c`, the 16-step bit
/// loop) costs on average over [`SHAPES_16`], read from the runtime's text.
pub fn clocks_compact_16(cpu: crate::dagisel::hooks::Cpu) -> f64 {
    SHAPES_16.iter().map(|&shape| clocks_16(cpu, "__udivhi3c", shape)).sum::<f64>() / SHAPES_16.len() as f64
}

/// What a call saves at speed, in clocks, when the speed 16-bit division (a
/// byte divisor takes a short path) is called and not the compact one, over
/// [`SHAPES_16`], read from the runtime's text: a divisor under 256 saves
/// hundreds of clocks, one of 256 or more costs the speed routine its test.
/// The speed routine is 39 bytes over the compact one.
pub fn saved_at_speed_16(cpu: crate::dagisel::hooks::Cpu) -> f64 {
    let sum: f64 =
        SHAPES_16.iter().map(|&shape| clocks_16(cpu, "__udivhi3c", shape) - clocks_16(cpu, "__udivhi3", shape)).sum();
    sum / SHAPES_16.len() as f64
}

/// [`runs`] of the 16-bit divisions.
pub fn runs16(module: &Module) -> f64 {
    runs_of_width(module, 16, None)
}

/// [`runs16`] in the block weights of the module's priced clocks.
pub fn runs16_priced(
    module: &Module,
    weights: &std::collections::HashMap<usize, Vec<u64>>,
) -> f64 {
    runs_of_width(module, 16, Some(weights))
}

fn runs_of_width(
    module: &Module,
    width: u32,
    priced: Option<&std::collections::HashMap<usize, Vec<u64>>>,
) -> f64 {
    let calls = call_weights(module);
    let mut runs = 0.0;
    for (index, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.entry().is_none() {
            continue;
        }
        let depths = block_depths(function);
        let called = calls.get(&index).copied().unwrap_or(1.0);
        // a remainder beside the division of the same operands is the same call
        // (`__divmodsi4`, `__udivmodsi4`: the remainder comes back in rc4..rc7)
        let quotients: Vec<_> = function
            .layout()
            .iter()
            .flat_map(|block| function.iter_block(*block))
            .filter_map(|inst| {
                let instruction = function.instruction(inst);
                let signed = match instruction.opcode {
                    Opcode::Binary(BinaryOp::UDiv) => false,
                    Opcode::Binary(BinaryOp::SDiv) => true,
                    _ => return None,
                };
                Some((signed, instruction.operands.get(0).copied(), instruction.operands.get(1).copied()))
            })
            .collect();
        for block in function.layout() {
            for inst in function.iter_block(*block) {
                let instruction = function.instruction(inst);
                let Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) =
                    instruction.opcode
                else {
                    continue;
                };
                let paired = |signed| {
                    let (a, b) = (instruction.operands.get(0).copied(), instruction.operands.get(1).copied());
                    width == 32 && quotients.iter().any(|&(s, x, y)| s == signed && x == a && y == b)
                };
                if match instruction.opcode {
                    Opcode::Binary(BinaryOp::URem) => paired(false),
                    Opcode::Binary(BinaryOp::SRem) => paired(true),
                    _ => false,
                } {
                    continue;
                }
                let long = instruction
                    .result
                    .is_some_and(|result| module.context.types.int_bits(function.value(result).ty) == Some(width));
                let byte = matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if matches!(module.context.get(*id).kind, ConstantKind::Int(1..=255))
                );
                if long && !byte {
                    let block = function.parent(inst);
                    runs += called
                        * match priced.and_then(|weights| weights.get(&index)) {
                            Some(weights) => block.and_then(|b| weights.get(b.0 as usize)).map_or(1.0, |w| *w as f64),
                            None => {
                                let at = block.and_then(|b| depths.get(&llrm_analysis::cfg::id(b))).copied();
                                weight(at.unwrap_or(0))
                            }
                        };
                }
            }
        }
    }
    runs
}
