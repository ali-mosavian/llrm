//! A 32-bit fixed-point product whose operands' product fits 32 bits is a `mul`
//! and an `ashr`.
//!
//! # How it works
//! `llvm.smul.fix(a, b, s)` is `(a * b) >> s` computed in 64 bits. isel lowers
//! it to the one-operand `imul` and `shrd`, which take EAX and EDX; the narrow
//! form `mul a, b; ashr s` is `imul r, r` and `sar`. The two are the same value
//! where the 64-bit product never leaves 32 bits.
//!
//! The test is on the operands, from `ranges`' interval of each at the call's
//! block, not on the result: `smul.fix(65536, 65536, 16)` is 65536, but its
//! product is 2^32 and keeps the wide form. The product's bounds are
//! `ranges::product` of the intervals; it must lie in `[-2^31, 2^31)`. Only
//! 32-bit products with a constant scale below 32 are taken.
//!
//! A factor `v << k` or `v + v` whose interval keeps it from wrapping leaves
//! the product for the scale: `(v * 2^k * b) >> s` is `(v * b) >> (s - k)`, so
//! `x * 2 * y` in Q8 is one `sar` by 7, and a fixed-point integer made by `<<
//! s` and scaled back costs nothing (the scale reaches 0 and the `ashr` is
//! dropped).
//!
//! ```text
//!  %x2 = shl %x, 1                          %m = mul %x, %y
//!  %p = call smul.fix(%x2, %y, 8)     ==>   %p = ashr %m, 7
//!  (x and y small enough that x*y fits)
//! ```
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
//! b0:
//!   %t = add i32 %a, %a
//!   %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
//!   ret i32 %p
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
//! b0:
//!   %t = add i32 %a, %a
//!   %0 = mul i32 %a, %b
//!   %1 = ashr i32 %0, 7
//!   ret i32 %1
//! }
//! ```
//!
//! The doubling leaves the product and the scale drops from 8 to 7; `%t` is
//! left for dead-code removal.
//!
//! From unit test `a_doubled_factor_leaves_the_product_for_the_scale` in
//! `fixednarrow_tests.rs` (fixture `SHIFTED`; the printed text was taken from a
//! run).
//!
//! # Runs
//! Once, after the loop-header copy and before `lsr`, which would hide such a
//! shift inside an induction variable. Not at -O0.
//!
//! # Needs
//! `ranges` (scoped intervals), `Registers` and the CFG shape.
//!
//! # Keeps
//! Blocks and edges are unchanged; dominators and loops are preserved.

use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, ranges};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

pub struct FixedNarrow;

impl FunctionPass for FixedNarrow {
    fn name(&self) -> &'static str {
        "fixednarrow"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let narrow = narrowable(unit, analyses);
        // The operands were taken before any rewrite: one that is an earlier
        // product's result reads its replacement.
        let mut replaced: std::collections::HashMap<Operand, Operand> = std::collections::HashMap::new();
        for product in &narrow {
            let product = &Narrow {
                a: *replaced.get(&product.a).unwrap_or(&product.a),
                b: *replaced.get(&product.b).unwrap_or(&product.b),
                ..*product
            };
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
            replaced.insert(Operand::Value(old), result);
            unit.function.replace_all_uses_with(old, result);
            unit.function.erase(product.inst).expect("its uses were replaced");
        }
        if narrow.is_empty() {
            PreservedAnalyses::all()
        } else {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        }
    }
}

/// `smul.fix` `inst` as `(a * b) >> scale`.
struct Narrow {
    inst: InstId,
    a: Operand,
    b: Operand,
    scale: u32,
}

/// The products whose operands, less the shifts they carry, multiply within
/// their width.
fn narrowable(
    unit: &passes::Unit,
    analyses: &mut Analyses,
) -> Vec<Narrow> {
    let held = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    let memory = Unit::within(unit.context, unit.layout, unit.function, analyses.outer())
        .with_registers(&held)
        .with_shape(&shape);
    let calls: Vec<_> = unit
        .function
        .walk()
        .filter(|&(_, inst)| matches!(
            memory.intrinsic(inst),
            Some(Intrinsic::Fixed { divide: false })
        ))
        .collect();
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
            let interval = |one: Operand| {
                ranges::_operand(&memory, one, &scope, &registers).filter(|interval| interval.width == width)
            };
            let limit = BigInt::from(1) << (width - 1);
            let fits = |low: &BigInt, high: &BigInt| -&limit <= *low && *high < limit;
            // `v << k` or `v + v` where `v`'s interval keeps it from wrapping:
            // `v * 2^k`.
            let doubled = |one: Operand, scale: u32| -> Option<(Operand, u32)> {
                let (_, op) = memory.defining(one)?;
                let (source, bits) = match (&op.opcode, &op.operands[..]) {
                    (Opcode::Binary(BinaryOp::Shl), &[source, count]) => {
                        (source, u32::try_from(memory.int_constant(count)?).ok()?)
                    }
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
