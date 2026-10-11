//! A phi's incoming value that the edge it arrives on pins to one point is that
//! constant: gcc's EVRP substitutes a singleton range, LLVM's
//! CorrelatedValuePropagation replaces a value whose `ConstantRange` is one
//! number. A counted loop's exit value (`i + 1` where the latch left on
//! `i + 1 < 32`, and `i` stays below 32) is read from the counter no more, so
//! its phi copy is no longer the old counter kept alive past its step.

use llrm_analysis::cfg;
use llrm_analysis::consts::{self, Calls};
use llrm_analysis::manager::Held;
use llrm_analysis::ranges;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_bigint::BigInt;

pub struct Pinned;

impl FunctionPass for Pinned {
    fn name(&self) -> &'static str {
        "pinned"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let pinned = {
            let held = Held::of(unit.context, unit.layout, unit.function, analyses, true).with_bounded(
                unit.context,
                unit.layout,
                unit.function,
                analyses,
            );
            let view = held.unit(unit.context, unit.layout, unit.function, analyses.outer());
            let function = view.function;
            let facts = consts::known_walked(&view, &Calls::default());
            let Ok(bounds) = ranges::bounds(&view) else { return PreservedAnalyses::all() };
            let mut found = Vec::new();
            for &block in function.layout() {
                for &phi in function.block(block).instructions() {
                    let op = function.instruction(phi);
                    if op.opcode != Opcode::Phi {
                        break;
                    }
                    for (index, pair) in op.operands.chunks(2).enumerate() {
                        let &[Operand::Value(value), Operand::Block(from)] = pair else { continue };
                        let Some(scope) = bounds.at(cfg::id(from)) else { continue };
                        let Ok(Some(edge)) = ranges::on_edge(&view, from, block, scope, Some(&facts)) else { continue };
                        let Some(interval) = ranges::_operand(&view, Operand::Value(value), &edge, &facts) else {
                            continue;
                        };
                        if interval.low == interval.high {
                            found.push((phi, 2 * index, value, interval.low, interval.width));
                        }
                    }
                }
            }
            found
        };
        if pinned.is_empty() {
            return PreservedAnalyses::all();
        }
        for (phi, at, value, number, width) in pinned {
            let ty = unit.function.value(value).ty;
            let constant = Operand::Constant(unit.context.int(ty, to_i128(&number, width)));
            unit.function.set_operand(phi, at, constant);
        }
        PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
    }
}

fn to_i128(number: &BigInt, _width: u32) -> i128 {
    i128::try_from(number.clone()).expect("an interval of at most 64 bits")
}

#[cfg(test)]
#[path = "pinned_tests.rs"]
mod tests;
