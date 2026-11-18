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

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::signbits;

/// The name of the `sext`/`zext` back to `i64` that [`narrow`] makes;
/// [`widen_back`] finds it.
const MARK: &str = "widen.mul";
/// Names a bitwise or add chain `narrow` computes in the narrow type.
const CHAIN: &str = "widen.chain";
/// The deepest chain of operations `narrow` computes narrow.
const CHAIN_DEPTH: u32 = 4;

/// What one operand of the multiply is, to narrow it.
enum Part {
    /// An extension only the multiply uses: narrowed by extending its source to
    /// the narrow type.
    Extension { op: CastOp, definition: InstId, source: Operand, source_bits: u32 },
    /// Any other value: narrowed by `trunc`.
    Plain(Operand),
}

/// The bits a signed `value` of `i64` takes (`ComputeMaxSignificantBits`).
fn signed_bits(
    context: &Context,
    function: &Function,
    value: Operand,
) -> u32 {
    thread_local! {
        static LAYOUT: DataLayout = DataLayout::default();
    }
    let signs =
        LAYOUT.with(|layout| signbits::compute_num_sign_bits(&Query::bare(context, function, layout), value, 0));
    65 - signs.min(64)
}

fn part(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Part {
    if let Operand::Value(result) = operand
        && let ValueDef::Instruction(definition) = function.value(result).def
        && function.users(result).iter().count() == 1
        && let Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt)) = function.instruction(definition).opcode
    {
        let source = function.instruction(definition).operands[0];
        if let Some(Type::Int(source_bits)) =
            function.operand_type(context, source).map(|ty| context.types.get(ty).clone())
        {
            return Part::Extension { op, definition, source, source_bits };
        }
    }
    Part::Plain(operand)
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

/// The `i64` constant `operand`, if it fits `bits` signed.
fn small_constant(
    context: &Context,
    operand: Operand,
    bits: u32,
) -> Option<i64> {
    let Operand::Constant(id) = operand else { return None };
    let (value, 64) = context.splat_int(id)? else { return None };
    let value = value as u64 as i64;
    (value >> (bits - 1) == 0 || value >> (bits - 1) == -1).then_some(value)
}

fn chain_op(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add | BinaryOp::Sub)
    )
}

/// Whether `operand` is an `i64` and/or/xor/add/sub of constants that fit
/// `bits` and values the same holds for or that `trunc` takes, each used once.
fn is_chain(
    context: &Context,
    function: &Function,
    operand: Operand,
    bits: u32,
) -> bool {
    let Operand::Value(value) = operand else { return false };
    let Some(def) = def_of(function, operand) else { return false };
    let inst = function.instruction(def);
    chain_op(&inst.opcode)
        && function.users(value).iter().count() == 1
        && inst
            .operands
            .iter()
            .all(|&arg| small_constant(context, arg, bits).is_some() || matches!(arg, Operand::Value(_)))
}

/// `operand` in the `narrow_ty` of `bits`: a chain `is_chain` accepts computed
/// narrow (LLVM's `canEvaluateTruncated`), anything else `trunc`ated. The
/// instructions a chain replaces are pushed to `dead`, outer first.
fn shrink(
    context: &mut Context,
    function: &mut Function,
    operand: Operand,
    narrow_ty: TypeId,
    before: Position,
    depth: u32,
    dead: &mut Vec<InstId>,
) -> Operand {
    let bits = context.types.scalar_int_bits(narrow_ty).expect("an integer type");
    if depth < CHAIN_DEPTH && is_chain(context, function, operand, bits) {
        let def = def_of(function, operand).expect("a chain is an instruction");
        let inst = function.instruction(def).clone();
        dead.push(def);
        let arguments = inst
            .operands
            .iter()
            .map(|&arg| match small_constant(context, arg, bits) {
                Some(value) => {
                    let mask = (1u128 << bits) - 1;
                    Operand::Constant(
                        context.constant(Constant { ty: narrow_ty, kind: ConstantKind::Int(value as u128 & mask) }),
                    )
                }
                None => shrink(context, function, arg, narrow_ty, before, depth + 1, dead),
            })
            .collect();
        let made = function.create_instruction(inst.opcode, narrow_ty, arguments, Flags::default(), Some(CHAIN));
        function.insert(made, before).expect("the multiply's block");
        return Operand::Value(function.instruction(made).result.expect("a chain operation"));
    }
    let made =
        function.create_instruction(Opcode::Cast(CastOp::Trunc), narrow_ty, vec![operand], Flags::default(), None);
    function.insert(made, before).expect("the multiply's block");
    Operand::Value(function.instruction(made).result.expect("a cast"))
}

/// Narrows every multiply into an `i64` whose product fits `i32` or `i16`: of
/// two same-kind extensions (the product of a `p`- and a `q`-bit value fits `p
/// + q` bits), or of an extension and a value that takes few signed bits;
///   whether
/// any was.
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
            let parts = [part(context, function, inst.operands[0]), part(context, function, inst.operands[1])];
            // The kind of the result extension, and the narrow width.
            let (kind, narrow_bits) = match &parts {
                [
                    Part::Extension { op: op_a, source_bits: bits_a, .. },
                    Part::Extension { op: op_b, source_bits: bits_b, .. },
                ] if op_a == op_b => match bits_a + bits_b {
                    0..=16 => (*op_a, 16),
                    17..=32 => (*op_a, 32),
                    _ => continue,
                },
                // An extension and a variable that takes few bits: signed.
                [Part::Extension { .. }, Part::Plain(Operand::Value(_))]
                | [Part::Plain(Operand::Value(_)), Part::Extension { .. }] => {
                    let bits: u32 = inst.operands.iter().map(|&operand| signed_bits(context, function, operand)).sum();
                    match bits {
                        0..=16 => (CastOp::SExt, 16),
                        17..=32 => (CastOp::SExt, 32),
                        _ => continue,
                    }
                }
                _ => continue,
            };
            let narrow_ty = context.types.int(narrow_bits);
            let before = Position::Before(mul);
            let cast = |function: &mut Function, op, source: Operand, to, name| {
                let made = function.create_instruction(Opcode::Cast(op), to, vec![source], Flags::default(), name);
                function.insert(made, before).expect("the multiply's block");
                Operand::Value(function.instruction(made).result.expect("a cast"))
            };
            let mut operands = Vec::new();
            let mut dead = vec![mul];
            for part in &parts {
                operands.push(match *part {
                    Part::Extension { op, definition, source, .. } => {
                        dead.push(definition);
                        cast(function, op, source, narrow_ty, None)
                    }
                    Part::Plain(value) => shrink(context, function, value, narrow_ty, before, 0, &mut dead),
                });
            }
            let flags = if kind == CastOp::SExt { Flags::NSW } else { Flags::NUW };
            let product = function.create_instruction(Opcode::Binary(BinaryOp::Mul), narrow_ty, operands, flags, None);
            function.insert(product, before).expect("the multiply's block");
            let product = Operand::Value(function.instruction(product).result.expect("a product"));
            let wide = cast(function, kind, product, inst.ty, Some(MARK));
            function.replace_all_uses_with(result, wide);
            for inst in dead {
                function.erase(inst).expect("a multiply and its extensions have no other user");
            }
            any = true;
        }
    }
    any
}

fn def_of(
    function: &Function,
    value: Operand,
) -> Option<InstId> {
    match value {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(def),
            _ => None,
        },
        _ => None,
    }
}

/// `operand` computed wide again: what `narrow` replaced, rebuilt from the
/// values it narrowed. The narrow instructions passed are pushed to `seen`,
/// outer first.
fn rewiden(
    context: &mut Context,
    function: &mut Function,
    operand: Operand,
    wide: TypeId,
    before: Position,
    seen: &mut Vec<InstId>,
) -> Option<Operand> {
    if let Operand::Constant(id) = operand {
        let (value, bits) = context.splat_int(id)?;
        let value = ((value << (128 - bits)) as i128 >> (128 - bits)) as u128 & u128::from(u64::MAX);
        return Some(Operand::Constant(context.constant(Constant { ty: wide, kind: ConstantKind::Int(value) })));
    }
    let def = def_of(function, operand)?;
    let inst = function.instruction(def).clone();
    let named_chain = |function: &Function| {
        inst.result
            .and_then(|result| function.value(result).name.as_deref())
            .is_some_and(|name| name.starts_with(CHAIN))
    };
    seen.push(def);
    match inst.opcode.clone() {
        Opcode::Cast(CastOp::Trunc) => Some(inst.operands[0]),
        Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt)) => {
            let made =
                function.create_instruction(Opcode::Cast(op), wide, vec![inst.operands[0]], Flags::default(), None);
            function.insert(made, before).expect("the extension's block");
            Some(Operand::Value(function.instruction(made).result.expect("a cast")))
        }
        opcode if chain_op(&opcode) && named_chain(function) => {
            let mut arguments = Vec::new();
            for &arg in &inst.operands {
                arguments.push(rewiden(context, function, arg, wide, before, seen)?);
            }
            let made = function.create_instruction(opcode, wide, arguments, Flags::default(), None);
            function.insert(made, before).expect("the chain's block");
            Some(Operand::Value(function.instruction(made).result.expect("a chain operation")))
        }
        _ => None,
    }
}

/// Undoes [`narrow`] for the multiplies still scalar: `sext i64 (mul iN (sext
/// a), (trunc b))` becomes `mul i64 (sext a), b`.
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
        let (Some(result), Some(product_inst)) = (one.result, def_of(function, one.operands[0])) else { continue };
        let multiply = function.instruction(product_inst).clone();
        if multiply.opcode != Opcode::Binary(BinaryOp::Mul) {
            continue;
        }
        let before = Position::Before(wide);
        let mut seen = vec![wide, product_inst];
        let mut operands = Vec::new();
        for &operand in &multiply.operands {
            match rewiden(context, function, operand, one.ty, before, &mut seen) {
                Some(rebuilt) => operands.push(rebuilt),
                None => break,
            }
        }
        if operands.len() != 2 {
            continue;
        }
        let flags = if one.opcode == Opcode::Cast(CastOp::SExt) { Flags::NSW } else { Flags::NUW };
        let product_wide = function.create_instruction(Opcode::Binary(BinaryOp::Mul), one.ty, operands, flags, None);
        function.insert(product_wide, before).expect("the extension's block");
        let product_wide = Operand::Value(function.instruction(product_wide).result.expect("a product"));
        function.replace_all_uses_with(result, product_wide);
        for dead in seen {
            if function.instruction(dead).result.is_none_or(|value| function.users(value).iter().next().is_none()) {
                let _ = function.erase(dead);
            }
        }
    }
}
