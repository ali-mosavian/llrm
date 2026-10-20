//! A 32-bit fixed-point product whose operands' product fits 32 bits is a `mul`
//! and an `ashr`.
//!
//! # How it works
//! `llvm.smul.fix(a, b, s)` is `(a * b) >> s` computed in 64 bits. isel lowers
//! it to the one-operand `imul` and `shrd`, which take EAX and EDX; the narrow
//! form `mul a, b; ashr s` is `imul r, r` and `sar`. The two are the same value
//! where the 64-bit product never leaves 32 bits.
//!
//! The test is on the operands, from LazyValueInfo's `getConstantRange` of each
//! at the call, not on the result: `smul.fix(65536, 65536, 16)` is 65536, but
//! its product is 2^32 and keeps the wide form. The product's bounds are
//! `ConstantRange::multiply` of the two ranges taken in 64 bits; it must lie
//! in `[-2^31, 2^31)`. Only 32-bit products with a constant scale below 32 are
//! taken.
//!
//! DEVIATE 1: not an LLVM pass. LazyValueInfo answers from definitions and
//! the branches on the edges to the call, not from trip counts or from a
//! test on a derived value, so a factor bounded only by a loop's escape test
//! (`examples/mandel`: `zr^2 + zi^2 > 4`) keeps its wide form, where the
//! `ranges` engine this replaced narrowed it.
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
//! LazyValueInfo (`LviCache`).
//!
//! # Keeps
//! Blocks and edges are unchanged; dominators and loops are preserved.

use std::rc::Rc;

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo, LviCache};
use llrm_mir::module::{GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::valuetracking::AssumptionCache;

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
        let globals = Rc::clone(&analyses.outer().globals);
        let calls: Vec<InstId> = unit
            .function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| {
                llrm_mir::memory::callee(unit.context, unit.function, inst)
                    .and_then(|callee| globals.get(callee.0 as usize)?.name.as_deref().and_then(Intrinsic::named))
                    == Some(Intrinsic::Fixed { divide: false })
            })
            .collect();
        if calls.is_empty() {
            return PreservedAnalyses::all();
        }
        let assumptions = AssumptionCache::of(unit.context, &globals, unit.metadata, unit.function);
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let mut lvi = shared.take();
        let mut changed = false;
        // One product at a time, rewritten before the next is asked: a product
        // that reads an earlier one's result asks LazyValueInfo about the
        // `mul` and `ashr` that replaced it.
        for inst in calls {
            let Some(product) = narrowable(unit, &globals, &assumptions, &mut lvi, inst) else { continue };
            rewrite(unit, &mut lvi, &product);
            changed = true;
        }
        shared.put(lvi);
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
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

/// `product` replaced by the `mul` and `ashr` that compute it.
fn rewrite(
    unit: &mut passes::Unit,
    lvi: &mut LazyValueInfo,
    product: &Narrow,
) {
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
    lvi.forget_value(Operand::Value(old));
    unit.function.erase(product.inst).expect("its uses were replaced");
}

/// The 32-bit width the product is taken in: only dwords are narrowed.
const WIDTH: u32 = 32;

/// `inst` as a `mul` and a shift, if its operands, less the shifts they
/// carry, multiply within their width.
fn narrowable(
    unit: &mut passes::Unit,
    globals: &[GlobalValue],
    assumptions: &AssumptionCache,
    lvi: &mut LazyValueInfo,
    inst: InstId,
) -> Option<Narrow> {
    let operands = unit.function.instruction(inst).operands.clone();
    let mut scale = u32::try_from(int_constant(unit, operands[2])?).ok().filter(|&scale| scale < WIDTH)?;
    let mut range = |unit: &mut passes::Unit, one: Operand| -> Option<ConstantRange> {
        let mut env = Env::new(&mut *unit.context, globals, unit.metadata, &*unit.function, unit.layout, assumptions);
        Some(lvi.get_constant_range(&mut env, one, inst, false)).filter(|range| range.width == WIDTH)
    };
    // `v << k` or `v + v` where `v`'s range keeps it from wrapping: `v * 2^k`.
    let doubled = |unit: &mut passes::Unit,
                   range: &mut dyn FnMut(&mut passes::Unit, Operand) -> Option<ConstantRange>,
                   one: Operand,
                   scale: u32|
     -> Option<(Operand, u32)> {
        let Operand::Value(value) = one else { return None };
        let ValueDef::Instruction(def) = unit.function.value(value).def else { return None };
        let (source, bits) =
            match (&unit.function.instruction(def).opcode, &unit.function.instruction(def).operands[..]) {
                (Opcode::Binary(BinaryOp::Shl), &[source, count]) => {
                    (source, u32::try_from(int_constant(unit, count)?).ok()?)
                }
                (Opcode::Binary(BinaryOp::Add), &[source, other]) if source == other => (source, 1),
                _ => return None,
            };
        (1..=scale).contains(&bits).then_some(())?;
        let v = range(unit, source)?;
        fits(&v.sign_extend(2 * WIDTH).multiply(&ConstantRange::single(2 * WIDTH, 1 << bits), 0))?
            .then_some((source, bits))
    };
    let mut taken = [operands[0], operands[1]];
    for one in &mut taken {
        while let Some((source, bits)) = doubled(unit, &mut range, *one, scale) {
            *one = source;
            scale -= bits;
        }
    }
    let wide = |range: ConstantRange| range.sign_extend(2 * WIDTH);
    let product = wide(range(unit, taken[0])?).multiply(&wide(range(unit, taken[1])?), 0);
    fits(&product)?.then_some(Narrow { inst, a: taken[0], b: taken[1], scale })
}

/// Whether `range`, taken in twice the width, lies within the signed width.
fn fits(range: &ConstantRange) -> Option<bool> {
    let limit = 1_i128 << (WIDTH - 1);
    let sign = |bits: u128| i128::from(bits as u64 as i64);
    Some(-limit <= sign(range.get_signed_min()) && sign(range.get_signed_max()) < limit)
}

/// An integer constant's bits, if `operand` is one.
fn int_constant(
    unit: &passes::Unit,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match unit.context.get(id).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "fixednarrow_tests.rs"]
mod tests;
