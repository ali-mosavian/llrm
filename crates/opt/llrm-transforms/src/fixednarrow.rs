//! A fixed-point product whose operands' product fits the width: `llvm.smul.fix(a, b, s)`
//! is `(a * b) >> s` at the width, as `mul` and `ashr` are, where `ranges` proves the
//! 64-bit product the intrinsic stands for never exceeds 32 bits. isel lowers the
//! intrinsic to a one-operand `imul` and `shrd`, which take EAX and EDX; the narrow form
//! is `imul r, r` and `sar`.
//!
//! The test is on the operands: `smul.fix(65536, 65536, 16)` is 65536, but its product is 2^32.
//!
//! A factor `v << k` or `v + v` of the product, which cannot wrap since its interval fits,
//! leaves it for the scale: `(v * 2^k * b) >> s` is `(v * b) >> (s - k)`, so `x * 2 * y` in Q8
//! is one `sar` by 7, and a fixed-point integer made by `<< s` and scaled back costs nothing.
//! It runs once, before LSR, which would hide such a shift in an induction variable.

use llrm_analysis::memory::Unit;
use llrm_analysis::{cfg, ranges};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use num_bigint::BigInt;

pub struct FixedNarrow;

impl FunctionPass for FixedNarrow {
    fn name(&self) -> &'static str {
        "fixednarrow"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let narrow = narrowable(unit, analyses);
        for product in &narrow {
            let ty = unit.function.instruction(product.inst).ty;
            let make = |unit: &mut passes::Unit, opcode, operands| {
                let new = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
                unit.function.insert(new, Position::Before(product.inst)).expect("a placed instruction");
                Operand::Value(unit.function.instruction(new).result.expect("a value"))
            };
            let mut result = make(unit, Opcode::Binary(BinaryOp::Mul), vec![product.a, product.b]);
            if product.scale > 0 {
                let by = Operand::Constant(unit.context.int(ty, i128::from(product.scale)));
                result = make(unit, Opcode::Binary(BinaryOp::AShr), vec![result, by]);
            }
            let old = unit.function.instruction(product.inst).result.expect("a value");
            unit.function.replace_all_uses_with(old, result);
            unit.function.erase(product.inst).expect("its uses were replaced");
        }
        if narrow.is_empty() { PreservedAnalyses::all() } else { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() }
    }
}

/// `smul.fix` `inst` as `(a * b) >> scale`.
struct Narrow {
    inst: InstId,
    a: Operand,
    b: Operand,
    scale: u32,
}

/// The products whose operands, less the shifts they carry, multiply within their width.
fn narrowable(unit: &passes::Unit, analyses: &mut Analyses) -> Vec<Narrow> {
    let held = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    let memory = Unit::within(unit.context, unit.layout, unit.function, analyses.outer()).with_registers(&held).with_shape(&shape);
    let calls: Vec<_> = unit.function.walk().filter(|&(_, inst)| matches!(memory.intrinsic(inst), Some(Intrinsic::Fixed { divide: false }))).collect();
    if calls.is_empty() {
        return Vec::new();
    }
    let facts = ranges::scoped(&memory).unwrap_or_default();
    let registers = &*held;
    calls
        .into_iter()
        .filter_map(|(block, inst)| {
            let operands = &unit.function.instruction(inst).operands;
            let width = memory.int_bits(operands[0]).filter(|&width| width == 32)?;
            let mut scale = u32::try_from(memory.int_constant(operands[2])?).ok().filter(|&scale| scale < width)?;
            let scope = facts.get(&cfg::id(block)).cloned().unwrap_or_default();
            let interval = |one: Operand| ranges::_operand(&memory, one, &scope, &registers).filter(|interval| interval.width == width);
            let limit = BigInt::from(1) << (width - 1);
            let fits = |low: &BigInt, high: &BigInt| -&limit <= *low && *high < limit;
            // `v << k` or `v + v` where `v`'s interval keeps it from wrapping: `v * 2^k`.
            let doubled = |one: Operand, scale: u32| -> Option<(Operand, u32)> {
                let (_, op) = memory.defining(one)?;
                let (source, bits) = match (&op.opcode, &op.operands[..]) {
                    (Opcode::Binary(BinaryOp::Shl), &[source, count]) => (source, u32::try_from(memory.int_constant(count)?).ok()?),
                    (Opcode::Binary(BinaryOp::Add), &[source, other]) if source == other => (source, 1),
                    _ => return None,
                };
                let v = interval(source)?;
                (1..=scale).contains(&bits).then_some(())?;
                fits(&(&v.low << bits), &(&v.high << bits)).then_some((source, bits))
            };
            let mut taken = [operands[0], operands[1]];
            for one in &mut taken {
                while let Some((source, bits)) = doubled(*one, scale) {
                    *one = source;
                    scale -= bits;
                }
            }
            let (a, b) = (interval(taken[0])?, interval(taken[1])?);
            let (low, high) = ranges::product(&a, &b, taken[0] == taken[1]);
            fits(&low, &high).then_some(Narrow { inst, a: taken[0], b: taken[1], scale })
        })
        .collect()
}

#[cfg(test)]
#[path = "fixednarrow_tests.rs"]
mod tests;
