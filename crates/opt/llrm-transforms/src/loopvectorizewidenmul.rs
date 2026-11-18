// SPDX-License-Identifier: GPL-3.0-or-later
//! gcc's widening multiply for the loop vectorizer:
//! `vect_recog_widen_mult_pattern` of
//! [`tree-vect-patterns.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/tree-vect-patterns.cc).
//!
//! # How it works
//! `mul i64 (sext i16 a), (sext i16 b)` is `sext i64 (mul i32 (sext i16 a),
//! (sext i16 b))`: the product of two `p`- and `q`-bit extensions fits `p + q`
//! bits, so the multiply runs in the smallest of `i16` and `i32` that holds it.
//! SSE2 has no 64-bit multiply (`pmuludq` and shifts); the narrow one is
//! `pmaddwd` or `pmullw`/`pmulhw`. The pin leaves the `i64` multiply
//! (InstCombine does not narrow it; the loop vectorizer prices `<2 x i64>` at
//! the pin's row).
//!
//! [`narrow`] rewrites the multiplies of the single-block loops before the
//! vectorizer plans them, so its cost model and its widening see the narrow
//! form; [`widen_back`] undoes the rewrite of every multiply that stays scalar,
//! so a loop that is not vectorized is unchanged.
//!
//! # DEVIATE rows
//! - LVW1 x86-64 only (`Tti::narrows_widening_multiplies`); the pin never
//!   narrows. Owner llrm-target-x86-m16-b, exit 2026-11-30.
//! - LVW2 only two extensions of one kind, each used by the multiply alone,
//!   into an `i64`.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::Type;

/// The name of the `sext`/`zext` back to `i64` that [`narrow`] makes;
/// [`widen_back`] finds it.
const MARK: &str = "widen.mul";

/// The kind of an extension and the width it extends from, if `value` is one.
fn extension(
    context: &Context,
    function: &Function,
    value: Operand,
) -> Option<(CastOp, ValueId, u32)> {
    let Operand::Value(result) = value else { return None };
    let llrm_mir::module::ValueDef::Instruction(def) = function.value(result).def else { return None };
    let inst = function.instruction(def);
    let Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt)) = inst.opcode else { return None };
    let Operand::Value(source) = inst.operands[0] else { return None };
    let Type::Int(bits) = context.types.get(function.value(source).ty).clone() else { return None };
    Some((op, result, bits))
}

/// A block that branches to itself: the loops the vectorizer plans.
fn self_loops(function: &Function) -> Vec<llrm_mir::module::BlockId> {
    function
        .layout()
        .iter()
        .copied()
        .filter(|&block| function.successors(block).iter().any(|&next| next == block))
        .collect()
}

/// Narrows every multiply of two same-kind extensions into an `i64` that the
/// product fits `i32` or `i16` for; whether any was.
pub(crate) fn narrow(
    context: &mut Context,
    function: &mut Function,
) -> bool {
    let mut any = false;
    for block in self_loops(function) {
        let multiplies: Vec<InstId> = function
            .iter_block(block)
            .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul))
            .collect();
        for mul in multiplies {
            let inst = function.instruction(mul).clone();
            let Some(result) = inst.result else { continue };
            if context.types.get(inst.ty) != &Type::Int(64) {
                continue;
            }
            let (Some((op_a, ext_a, bits_a)), Some((op_b, ext_b, bits_b))) =
                (extension(context, function, inst.operands[0]), extension(context, function, inst.operands[1]))
            else {
                continue;
            };
            // The product of a `p`- and a `q`-bit value fits `p + q` bits,
            // signed or not.
            let narrow_bits = match bits_a + bits_b {
                0..=16 => 16,
                17..=32 => 32,
                _ => continue,
            };
            if op_a != op_b
                || ext_a == ext_b
                || function.users(ext_a).iter().count() != 1
                || function.users(ext_b).iter().count() != 1
            {
                continue;
            }
            let signed = op_a == CastOp::SExt;
            let narrow_ty = context.types.int(narrow_bits);
            let before = Position::Before(mul);
            let mut cast = |function: &mut Function, source: Operand, to, name| {
                let made = function.create_instruction(Opcode::Cast(op_a), to, vec![source], Flags::default(), name);
                function.insert(made, before).expect("the multiply's block");
                Operand::Value(function.instruction(made).result.expect("a cast"))
            };
            let sources = [ext_a, ext_b].map(|ext| function.instruction(def_of(function, ext)).operands[0]);
            let (a, b) = (cast(function, sources[0], narrow_ty, None), cast(function, sources[1], narrow_ty, None));
            let flags = if signed { Flags::NSW } else { Flags::NUW };
            let product =
                function.create_instruction(Opcode::Binary(BinaryOp::Mul), narrow_ty, vec![a, b], flags, None);
            function.insert(product, before).expect("the multiply's block");
            let product = Operand::Value(function.instruction(product).result.expect("a product"));
            let wide = cast(function, product, inst.ty, Some(MARK));
            function.replace_all_uses_with(result, wide);
            for dead in [mul, def_of(function, ext_a), def_of(function, ext_b)] {
                function.erase(dead).expect("a multiply and its extensions have no other user");
            }
            any = true;
        }
    }
    any
}

fn def_of(
    function: &Function,
    value: ValueId,
) -> InstId {
    match function.value(value).def {
        llrm_mir::module::ValueDef::Instruction(def) => def,
        _ => unreachable!("an extension is an instruction"),
    }
}

/// Undoes [`narrow`] for the multiplies still scalar: `sext i64 (mul iN (sext
/// a), (sext b))` becomes `mul i64 (sext a), (sext b)`.
pub(crate) fn widen_back(
    context: &mut Context,
    function: &mut Function,
) {
    let marked: Vec<InstId> = function
        .layout()
        .to_vec()
        .into_iter()
        .flat_map(|block| function.iter_block(block).collect::<Vec<_>>())
        .filter(|&inst| {
            let one = function.instruction(inst);
            matches!(one.opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt))
                && context.types.get(one.ty) == &Type::Int(64)
                && one
                    .result
                    .and_then(|result| function.value(result).name.as_deref())
                    .is_some_and(|name| name.starts_with(MARK))
        })
        .collect();
    for wide in marked {
        let one = function.instruction(wide).clone();
        let (Some(result), Operand::Value(product)) = (one.result, one.operands[0]) else { continue };
        let product_inst = def_of(function, product);
        let multiply = function.instruction(product_inst).clone();
        if multiply.opcode != Opcode::Binary(BinaryOp::Mul) {
            continue;
        }
        let (Some((_, ext_a, _)), Some((_, ext_b, _))) =
            (extension(context, function, multiply.operands[0]), extension(context, function, multiply.operands[1]))
        else {
            continue;
        };
        let before = Position::Before(wide);
        let sources = [ext_a, ext_b].map(|ext| function.instruction(def_of(function, ext)).operands[0]);
        let Opcode::Cast(op) = one.opcode else { continue };
        let mut made = Vec::new();
        for source in sources {
            let cast = function.create_instruction(Opcode::Cast(op), one.ty, vec![source], Flags::default(), None);
            function.insert(cast, before).expect("the extension's block");
            made.push(Operand::Value(function.instruction(cast).result.expect("a cast")));
        }
        let flags = if op == CastOp::SExt { Flags::NSW } else { Flags::NUW };
        let product_wide = function.create_instruction(Opcode::Binary(BinaryOp::Mul), one.ty, made, flags, None);
        function.insert(product_wide, before).expect("the extension's block");
        let product_wide = Operand::Value(function.instruction(product_wide).result.expect("a product"));
        function.replace_all_uses_with(result, product_wide);
        for dead in [wide, product_inst, def_of(function, ext_a), def_of(function, ext_b)] {
            if function.instruction(dead).result.is_none_or(|value| function.users(value).iter().next().is_none()) {
                let _ = function.erase(dead);
            }
        }
    }
}
