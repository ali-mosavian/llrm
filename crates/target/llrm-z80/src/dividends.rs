//! The bits the middle end proves the dividend of each long divide needs
//! (`BuilderTarget::dividend_ranges`): the speed routine's price is a step a
//! bit beyond the divisor's, so a call is priced by the dividend it has, not
//! a typical one.
//!
//! DEVIATE DIV-RANGE-NOTE (owner w12, exit 2026-12-13): the pin's libcall has
//! no operand facts but the DAG's known bits, which a loop-carried value has
//! none of; the range comes from the middle end (`ValueFacts`) and rides on
//! the call as a note (`runtime_size::split_note`). Exit: the ranges ride on
//! the IR as metadata the builder reads and the DAG asserts.

use llrm_codegen::backend::selectiondag::builder::DividendRange;
use llrm_codegen::backend::valuefacts::ValueFacts;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

/// The range of the dividend of every 32-bit divide or remainder of
/// `function` that the middle end proves narrower than the type.
pub fn dividend_ranges(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
) -> IdMap<InstId, DividendRange> {
    let facts = ValueFacts::new(module, layout, function);
    let mut ranges = IdMap::new();
    for &block in function.layout() {
        for at in function.iter_block(block) {
            let instruction = function.instruction(at);
            let Opcode::Binary(op @ (BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)) =
                instruction.opcode
            else {
                continue;
            };
            let Some(&Operand::Value(dividend)) = instruction.operands.first() else { continue };
            if module.context.types.int_bits(instruction.ty) != Some(32) {
                continue;
            }
            let Some(span) = facts.get(at, dividend).filter(|span| span.width == 32) else { continue };
            let (Ok(low), Ok(high)) = (i64::try_from(&span.low), i64::try_from(&span.high)) else { continue };
            // an unsigned dividend of a span below zero is 2^32 above it: no
            // bound
            let unsigned = matches!(op, BinaryOp::UDiv | BinaryOp::URem);
            if !(unsigned && low < 0) {
                ranges.insert(at, DividendRange { low, high });
            }
        }
    }
    ranges
}
