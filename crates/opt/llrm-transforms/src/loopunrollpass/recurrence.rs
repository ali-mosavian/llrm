//! DEVIATE RU-REC (owner x86-m64; exit 2026-11-20): a loop is not unrolled at
//! run time when its loop-carried dependence takes longer than the core needs
//! to issue one trip.
//!
//! The pin's `getUnrollingPreferences` sets `UP.Runtime` for any loop that fits
//! the loop buffer (BasicTTIImpl.h L738-803) and asks nothing of the loop.
//! Unrolling only removes the trip's counter and branch; with the time per trip
//! set by a chain of dependent operations, the copies run no sooner, and the
//! guard, the split of the count and the remainder loop are code for nothing
//! (floats -O2 394 B against gcc's 207 for the same clocks).
//!
//! The chain is the pin's own estimate of a loop's critical path,
//! `SelectOptimize::computeLoopCosts`: two walks of the body, each operation
//! costing its latency on top of its dearest operand, the second walk seeing
//! the first's values through the header phis. The growth of the longest path
//! between the walks is the latency a trip adds. Only a single-block loop is
//! priced: its operations all run each trip, so the sums are the trip's.
//!
//! A load of the address a store wrote, an address the same every trip, waits for it, as a
//! phi waits for its latch value: a variable kept in memory (`volatile double
//! value`, floats -O2) is a chain like any other. Known limits, not claims: a
//! chain through different addresses (`a[i] = a[i - 1] * c`) shows no growth and
//! keeps the pin's unroll, and so does a loop of several blocks. Latencies and
//! throughputs are the `Tti`'s, as the pin's.
//!
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1305>

use llrm_mir::dense::IdMap;
use llrm_mir::module::{InstId, ValueDef};

use super::*;

/// Whether the runtime unroll of `lp` is vetoed: the target asks it
/// (`Tuning::unroll_recurrence_veto`), states its issue width, and the loop is
/// a single block whose chain is longer than its throughput cost over that width.
pub(super) fn recurrence_bound(
    run: &Run,
    lp: Loop,
) -> bool {
    let width = i64::from(run.tti.issue_width());
    if width == 0 || !run.tti.vetoes_recurrence_bound_unroll() {
        return false;
    }
    let &[block] = lp.get_blocks() else {
        if llrm_support::debug::enabled("unroll") {
            eprintln!("unroll: RU-REC: {} blocks, not priced", lp.get_blocks().len());
        }
        return false;
    };
    let price = |inst: InstId, kind: CostKind| {
        let cost = instruction_cost(run.tti, run.context, run.layout, run.globals, run.function, inst, kind);
        cost.valid.then_some(cost.value)
    };
    let chain = chain_growth(run.function, block, |inst| price(inst, CostKind::Latency));
    let issue = run
        .function
        .iter_block(block)
        .map(|inst| price(inst, CostKind::RecipThroughput))
        .sum::<Option<i64>>();
    if llrm_support::debug::enabled("unroll") {
        eprintln!("unroll: RU-REC: chain {chain:?}, issue {issue:?}, width {width}");
    }
    let (Some(chain), Some(issue)) = (chain, issue) else { return false };
    // Strictly: when the two are equal the issue side binds too, and the copies
    // do relieve it (jpeg -O2: chain 3, issue 12 / 4, +2.9% instructions, clocks
    // unmeasured).
    chain * width > issue
}

/// Integer arithmetic the copies of an unrolled body compose: `x * a + b` twice
/// is `x * a * a + (a + 1) * b`, one multiply and add for two trips, so the
/// chain it makes is not a bound unrolling cannot loosen (quicksort -O3: the
/// LCG fill, +7.3% clocks when vetoed). Floating point, divides, selects and
/// memory do not compose, so only they are counted.
fn composes(opcode: &Opcode) -> bool {
    use llrm_mir::opcode::{BinaryOp::*, CastOp};
    matches!(
        opcode,
        Opcode::Phi
            | Opcode::GetElementPtr { .. }
            | Opcode::ICmp(_)
            | Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt)
            | Opcode::Binary(Add | Sub | Mul | Shl | LShr | AShr | And | Or | Xor)
    )
}

/// The longest dependence path through two walks of `block`, the second minus
/// the first: what one more trip adds. `None` when an operation has no price.
fn chain_growth(
    function: &Function,
    block: BlockId,
    latency: impl Fn(InstId) -> Option<i64>,
) -> Option<i64> {
    let mut path: IdMap<InstId, i64> = IdMap::new();
    // The path of the value last stored to an address, by the address operand.
    let mut stored: Vec<(Operand, i64)> = Vec::new();
    let mut longest = [0i64; 2];
    for longest in &mut longest {
        // A store reaches the next trip's load only at an address that is the
        // same every trip: one a trip computes (`counts[i]++`) is another cell.
        stored.retain(|(address, _)| match address {
            Operand::Value(value) => match function.value(*value).def {
                ValueDef::Instruction(def) => function.parent(def) != Some(block),
                _ => true,
            },
            _ => true,
        });
        for inst in function.iter_block(block) {
            let operands = function
                .instruction(inst)
                .operands
                .iter()
                .filter_map(|&operand| match operand {
                    Operand::Value(value) => match function.value(value).def {
                        ValueDef::Instruction(def) => path.get(&def).copied(),
                        _ => None,
                    },
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let instruction = function.instruction(inst);
            let forwarded = match instruction.opcode {
                Opcode::Load { .. } => stored.iter().find(|(address, _)| *address == instruction.operands[0]),
                _ => None,
            };
            let own = if composes(&instruction.opcode) { 0 } else { latency(inst)? };
            let here = operands.max(forwarded.map_or(0, |&(_, path)| path)) + own;
            if let Opcode::Store { .. } = instruction.opcode {
                let address = instruction.operands[1];
                stored.retain(|(one, _)| *one != address);
                stored.push((address, here));
            }
            path.insert(inst, here);
            *longest = (*longest).max(here);
        }
    }
    Some(longest[1] - longest[0])
}
