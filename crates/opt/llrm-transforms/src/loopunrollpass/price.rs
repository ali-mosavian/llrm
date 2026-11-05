//! DEVIATE UP1 (owner x86-m32; exit 2026-11-20): a full unroll the pin's
//! threshold allows is made only where the clocks it saves repay the bytes it
//! adds. The pin's bar counts instructions and never asks what a copy of the
//! body costs in code (x_ld_max: the 8-trip inner loop, copied 8 times,
//! 232 -> 755 bytes at m32 -O2 for a few clocks).

use std::collections::HashSet;

use llrm_mir::module::ValueId;

use super::*;
use crate::loopidioms::{trip_clocks, trip_cost};

/// How many times a loop of unknown trip count is assumed to run, as the
/// pin's branch probabilities do (`LBH_TAKEN_WEIGHT` 124 : `LBH_NONTAKEN_WEIGHT`
/// 4, a back edge taken 31 times in 32).
const ASSUMED_TRIPS: i64 = 32;

/// Whether `saved` clocks repay `added` bytes at `rate` thousandths of a clock
/// a byte: the one profit rule of a priced unroll, shared by every target.
pub(crate) fn unroll_pays(
    saved: i64,
    added: i64,
    rate: i64,
) -> bool {
    saved * 1000 >= added * rate
}

/// Whether the full unroll of `lp` by `trips` stays, at the target's rate
/// (`Tti::unroll_milliclocks_per_byte`). Only at -O2/-O3: -Os and -Oz hold to
/// the pin's bytes.
pub(super) fn full_unroll_pays(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    eph: &IdSet<InstId>,
    trips: u32,
    uce: &UnrollCostEstimator,
    up: &UnrollingPreferences,
) -> bool {
    let rate = run.tti.unroll_milliclocks_per_byte();
    if rate <= 0 || run.options.opt_size || run.options.min_size || run.options.opt_level < 2 {
        return true;
    }
    let clocks = crate::profit::costs(run.outer);
    let bytes = run.outer.target().size_costs();
    let blocks = lp.get_blocks();
    let body_bytes = trip_cost(run.context, run.layout, run.function, run.callees, blocks.iter(), &bytes, true);
    let body_clocks = trip_clocks(run.context, run.layout, run.function, run.callees, blocks.iter(), &clocks);
    // What the copies need not run: the instructions the analysis simplifies
    // (the counter, the exit test, folded loads), at the body's average clocks
    // a unit; where it cannot analyze (a loop around loops), the back edge of
    // every trip, and the operands the copies make constant (below).
    let analysis = analyze_loop_unroll_cost(
        run,
        analyzed,
        lp,
        trips,
        eph,
        u32::MAX,
        trips,
        None,
    );
    // The code the copies add, at the body's average bytes a unit: what the
    // analysis folds away is not emitted (byval_struct: a 41-trip loop of
    // constants is two instructions, not 41 copies).
    let rolled_units = i64::from(uce.rolled_loop_size().max(1));
    let unrolled_units = analysis
        .as_ref()
        .map_or(uce.unrolled_loop_size(up, trips) as i64, |cost| i64::from(cost.unrolled_cost));
    let added = ((unrolled_units - rolled_units).max(0) * body_bytes) / rolled_units;
    let (saved_units, constant_clocks) = match analysis {
        Some(cost) => (i64::from(cost.rolled_dynamic_cost.saturating_sub(cost.unrolled_cost)), 0),
        None => (
            i64::from(trips) * i64::from(up.be_insns),
            i64::from(trips) * constant_operand_uses(run, analyzed, lp) * clocks.r#move,
        ),
    };
    // The loop runs once for each trip of the loops around it, which the
    // bytes do not: the saving is counted over those runs (x_cordic: the
    // 12-trip loop inside a loop of `n` runs `n` times, 89 clocks each).
    let runs = ASSUMED_TRIPS.pow((lp.get_loop_depth() - 1).min(2));
    let saved = (saved_units * body_clocks / rolled_units + constant_clocks) * runs;
    why(
        run,
        lp,
        &format!("priced unroll: {trips} trips, saves {saved} clk, adds {added} B (body {body_bytes} B), rate {rate}"),
    );
    unroll_pays(saved, added, rate)
}

/// The operands of the loops inside `lp` that each copy of its body makes a
/// constant, one for each time they run (a loop of unknown trips runs
/// `ASSUMED_TRIPS`): a value computed from the counters of `lp` alone is a
/// constant in a copy, and its readers take an immediate, not a register
/// (x_ld_clamp: `8 + r` a bound of the inner compare, the stack's `fxch`).
fn constant_operand_uses(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> i64 {
    let function = &*run.function;
    let header = lp.get_header();
    let (Some(preheader), Some(latch)) = (lp.get_loop_preheader(function), lp.get_loop_latch(function)) else {
        return 0;
    };
    let depth = lp.get_loop_depth();
    let own: Vec<InstId> = lp
        .get_blocks()
        .iter()
        .filter(|&&block| analyzed.loops.get_loop_depth(block) == depth)
        .flat_map(|&block| function.iter_block(block).collect::<Vec<_>>())
        .collect();
    let incoming = |inst: InstId, from: BlockId| {
        function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0])
    };
    // The counters start at constants; keep those whose step is computed from
    // them alone.
    let mut counters: Vec<InstId> = function
        .iter_block(header)
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .filter(|&inst| matches!(incoming(inst, preheader), Some(Operand::Constant(_))))
        .collect();
    let known = loop {
        let mut known: HashSet<ValueId> = counters.iter().filter_map(|&inst| function.instruction(inst).result).collect();
        let mut grew = true;
        while grew {
            grew = false;
            for &inst in &own {
                let one = function.instruction(inst);
                let pure = matches!(
                    one.opcode,
                    Opcode::FNeg
                        | Opcode::Binary(_)
                        | Opcode::Cast(_)
                        | Opcode::ICmp(_)
                        | Opcode::FCmp(_)
                        | Opcode::Select
                        | Opcode::GetElementPtr { .. }
                ) || matches!(one.opcode, Opcode::Call(_))
                    && crate::licm::hoist::is_safe_to_speculatively_execute_in(
                        run.context,
                        run.layout,
                        function,
                        run.outer,
                        inst,
                    );
                let Some(result) = one.result else { continue };
                if pure
                    && !known.contains(&result)
                    && one.operands.iter().all(|operand| match operand {
                        Operand::Value(value) => known.contains(value),
                        Operand::Constant(_) => true,
                        Operand::Block(_) => false,
                    })
                {
                    known.insert(result);
                    grew = true;
                }
            }
        }
        let before = counters.len();
        counters.retain(|&inst| match incoming(inst, latch) {
            Some(Operand::Value(value)) => known.contains(&value),
            Some(Operand::Constant(_)) => true,
            _ => false,
        });
        if counters.len() == before {
            break known;
        }
    };
    let mut uses = 0;
    for &block in lp.get_blocks() {
        let nested = analyzed.loops.get_loop_depth(block).saturating_sub(depth);
        if nested == 0 {
            continue;
        }
        let weight = ASSUMED_TRIPS.pow(nested.min(2));
        for inst in function.iter_block(block) {
            let reads = function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if known.contains(value)));
            if reads {
                uses += weight;
            }
        }
    }
    uses
}
