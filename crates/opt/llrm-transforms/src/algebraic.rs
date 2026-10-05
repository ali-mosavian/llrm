//! Algebraic identities over integer values, as LLVM's InstCombine and, for
//! a loop-carried operand, Reassociate.
//!
//! Adapted from llrm-core's `optimize/algebraic.rs`, the port of
//! `qbopt/optimize/algebraic.py`. Every rewrite holds modulo 2^width, so a
//! rewritten instruction drops `nsw`, `nuw`, `exact` and `disjoint`: a
//! reassociated sum may overflow where the original did not.
//!
//! Gone with the old MIR:
//! - `wholephis`, `wholestores`, `_halved`, `_halves`, `_takes_halves`,
//!   `_extracted`, `_recombined` and a `Concat` of constants: a long split
//!   into word halves. Every value is whole here.
//! - `_forwarded_zero_tests`: `or x, x` for its x86 flags; `icmp` reads the
//!   value.
//! - `_product`: a widening multiply's unused high half; `mul` has one result.
//! - `_zero_difference` and `_copied_zero`: `sub 0, x` is the negation here,
//!   and nothing is a copy.
//! - `wanted` and `wide`: which flags and which halves were read.
//! - `_distributing`'s and `_mask_scaled`'s pairing: the old rules read a
//!   snapshot and rewrote one op each, so both ops of a pair had to agree to
//!   change. Here a rewrite replaces both at once, on the current function.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::consts::{self, Known};
use llrm_analysis::manager::Registers;
use llrm_analysis::memory::Unit;
use llrm_analysis::{cfg, induction};
use llrm_mir::context::{Constant, ConstantExpr, ConstantKind, Context, mask};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use num_bigint::BigInt;

use crate::counting;
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;

pub struct Algebraic;

impl FunctionPass for Algebraic {
    fn name(&self) -> &'static str {
        "algebraic"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if simplified(unit.context, unit.layout, unit.function, analyses) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Every rule, to a fixed point; whether anything changed.
pub fn simplified(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &mut Analyses) -> bool {
    // A number proved before stays proved: every rewrite keeps each
    // surviving value's meaning, and none divides by a value.
    let divided_by_values = function.walk().any(|(_, inst)| {
        let instruction = function.instruction(inst);
        matches!(instruction.opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)) && matches!(instruction.operands[1], Operand::Value(_))
    });
    let facts = if divided_by_values { analyses.get::<Registers>(context, layout, function) } else { Default::default() };
    let outer = std::rc::Rc::clone(analyses.outer());
    let mut changed = false;
    loop {
        let mut round = _whole_fixed(context, layout, function, &outer);
        round |= _divisions(context, layout, function, &outer, &facts);
        let recurrences = _recurrences(context, layout, function, &outer);
        for (_, inst) in function.walk().collect::<Vec<_>>() {
            if !function.is_erased(inst) {
                round |= _rewritten(context, layout, function, &recurrences, inst);
            }
        }
        round |= _reassociated_recurrences(function);
        round |= _shared_shifts(context, function);
        if !round {
            return changed;
        }
        changed = true;
    }
}

/// The first rule that rewrites `inst`.
fn _rewritten(context: &mut Context, layout: &DataLayout, function: &mut Function, recurrences: &BTreeSet<ValueId>, inst: InstId) -> bool {
    _mask_scaled(context, function, recurrences, inst)
        || _constant_address(context, function, inst)
        || _offset_scaled(context, function, inst)
        || _cast_pair(context, function, inst)
        || _nonnegative_sext(context, function, inst)
        || _masked_extension(context, layout, function, inst)
        || _casted_logic(context, function, inst)
        || _phi_of_casts(context, function, inst)
        || _duplicate_phi(function, inst)
        || _negated_difference(context, function, inst)
        || _shift_chain(context, function, inst)
        || _scaled_chain(context, function, inst)
        || _offset_chain(context, function, inst)
        || _bitwise_chain(context, function, inst)
        || _identity(context, function, inst)
        || _decided(context, function, inst)
        || _selected(context, function, inst)
        || _inverted_compare(context, function, inst)
        || _extended_boolean_tested(context, function, inst)
        || _extended_boolean_negated(context, function, inst)
}

/// The `i1` `operand` extends, read through `sext` or `zext`, and whether
/// the extension makes a true all ones.
fn _extended_boolean(function: &Function, operand: Operand) -> Option<(Operand, bool)> {
    let made = _definition(function, operand)?;
    let instruction = function.instruction(made);
    match instruction.opcode {
        Opcode::Cast(kind @ (CastOp::SExt | CastOp::ZExt)) => Some((instruction.operands[0], kind == CastOp::SExt)),
        _ => None,
    }
}

/// `icmp ne (sext i1 x), 0` is `x`, and `icmp eq` of it `!x`: Nib's booleans
/// are eight-bit 0 and -1, tested against zero.
fn _extended_boolean_tested(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ICmp(predicate @ (IntPredicate::Ne | IntPredicate::Eq)) = instruction.opcode else { return false };
    let [extended, zero] = instruction.operands[..] else { return false };
    if _integer(context, zero) != Some(0) {
        return false;
    }
    let Some((boolean, _)) = _extended_boolean(function, extended) else { return false };
    if function.operand_type(context, boolean).and_then(|ty| context.types.int_bits(ty)) != Some(1) {
        return false;
    }
    if predicate == IntPredicate::Ne {
        _forward(function, inst, boolean);
    } else {
        let truth = counting::constant(context, &BigInt::from(1), 1);
        let bit = context.types.int(1);
        let flipped = function.create_instruction(Opcode::Binary(BinaryOp::Xor), bit, vec![boolean, truth], Flags::default(), None);
        function.insert(flipped, Position::Before(inst)).expect("a placed compare");
        let value = Operand::Value(function.instruction(flipped).result.expect("a value"));
        _forward(function, inst, value);
    }
    true
}

/// `xor (sext i1 x), -1` is `sext (xor i1 x, true)`, and `xor (zext x), 1`
/// the same zero-extended.
fn _extended_boolean_negated(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((BinaryOp::Xor, left, right, width)) = _binary(context, function, inst) else { return false };
    if width == 1 {
        return false;
    }
    let Some(((boolean, sign), number)) = [(left, right), (right, left)].into_iter().find_map(|(one, other)| Some((_extended_boolean(function, one)?, _integer(context, other)?))) else { return false };
    if number != if sign { mask(width) } else { 1 }
        || function.operand_type(context, boolean).and_then(|ty| context.types.int_bits(ty)) != Some(1)
    {
        return false;
    }
    let truth = counting::constant(context, &BigInt::from(1), 1);
    let bit = context.types.int(1);
    let flipped = function.create_instruction(Opcode::Binary(BinaryOp::Xor), bit, vec![boolean, truth], Flags::default(), None);
    function.insert(flipped, Position::Before(inst)).expect("a placed xor");
    let negated = Operand::Value(function.instruction(flipped).result.expect("a value"));
    let ty = function.instruction(inst).ty;
    let kind = if sign { CastOp::SExt } else { CastOp::ZExt };
    let extended = function.create_instruction(Opcode::Cast(kind), ty, vec![negated], Flags::default(), None);
    function.insert(extended, Position::Before(inst)).expect("a placed xor");
    let value = Operand::Value(function.instruction(extended).result.expect("a value"));
    _forward(function, inst, value);
    true
}

/// `xor (icmp P a b), true` is `icmp !P a b` where nothing else reads the
/// compare: Nib's `if !(i < n): break` branched on the negation, which
/// hid the exit from every counting pass and cost `setl; neg; xor`.
fn _inverted_compare(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((BinaryOp::Xor, left, right, 1)) = _binary(context, function, inst) else { return false };
    let Some((compare, _)) = [(left, right), (right, left)].into_iter().find(|&(_, other)| _integer(context, other) == Some(1)) else { return false };
    let Some(made) = _definition(function, compare).filter(|_| _single_use(function, compare)) else { return false };
    let Opcode::ICmp(predicate) = function.instruction(made).opcode else { return false };
    let operands = function.instruction(made).operands.clone();
    _replace(function, inst, Opcode::ICmp(predicate.inverse()), operands);
    true
}

fn _definition(function: &Function, operand: Operand) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `operand` is a value read in one operand slot.
fn _single_use(function: &Function, operand: Operand) -> bool {
    matches!(operand, Operand::Value(value) if function.users(value).len() == 1)
}

fn _integer(context: &Context, operand: Operand) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `inst`'s integer width.
fn _width(context: &Context, function: &Function, inst: InstId) -> Option<u32> {
    context.types.int_bits(function.instruction(inst).ty)
}

/// `inst`'s integer operation and operands.
fn _binary(context: &Context, function: &Function, inst: InstId) -> Option<(BinaryOp, Operand, Operand, u32)> {
    let instruction = function.instruction(inst);
    let Opcode::Binary(op) = instruction.opcode else { return None };
    Some((op, instruction.operands[0], instruction.operands[1], _width(context, function, inst)?))
}

/// A value and a constant, in either order where `op` commutes.
fn _value_and_constant(context: &Context, op: BinaryOp, left: Operand, right: Operand) -> Option<(Operand, u128)> {
    let commutes = matches!(op, BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor);
    match (left, _integer(context, right), _integer(context, left)) {
        (Operand::Value(_), Some(constant), _) => Some((left, constant)),
        (_, None, Some(constant)) if commutes && matches!(right, Operand::Value(_)) => Some((right, constant)),
        _ => None,
    }
}

/// `inst` rebuilt as `opcode` over `operands` where it was, flags cleared,
/// and what that orphaned, gone.
fn _replace(function: &mut Function, inst: InstId, opcode: Opcode, operands: Vec<Operand>) {
    let value = _before(function, inst, opcode, operands);
    _forward(function, inst, value);
}

/// Every use of `inst` now reads `with`, and `inst` and what it alone read, gone.
fn _forward(function: &mut Function, inst: InstId, with: Operand) {
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, with);
    _erase(function, inst);
}

fn _erase(function: &mut Function, inst: InstId) {
    let operands = function.instruction(inst).operands.clone();
    function.erase(inst).expect("its uses were replaced");
    for operand in operands {
        let Some(made) = _definition(function, operand) else { continue };
        let orphan = matches!(function.instruction(made).opcode, Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_));
        if orphan && !function.is_erased(made) && function.users(function.instruction(made).result.expect("a value")).is_empty() {
            _erase(function, made);
        }
    }
}

/// A new instruction before `at`, of `at`'s type; its value.
fn _before(function: &mut Function, at: InstId, opcode: Opcode, operands: Vec<Operand>) -> Operand {
    let ty = function.instruction(at).ty;
    let new = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(new, Position::Before(at)).expect("a placed instruction");
    Operand::Value(function.instruction(new).result.expect("a value"))
}

fn _constant(context: &mut Context, function: &Function, inst: InstId, bits: u128) -> Operand {
    Operand::Constant(context.int(function.instruction(inst).ty, bits as i128))
}

/// `x * factor`, as `mul` or `shl` of a constant: `(x, factor, op, constant)`.
struct Scale {
    source: Operand,
    factor: u128,
    op: BinaryOp,
    constant: Operand,
}

fn _scale(context: &Context, function: &Function, inst: InstId) -> Option<Scale> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let (source, constant) = _value_and_constant(context, op, left, right)?;
    let factor = match op {
        BinaryOp::Shl if 0 < constant && constant < u128::from(width) => 1 << constant,
        BinaryOp::Mul => constant,
        _ => return None,
    };
    let constant = if source == left { right } else { left };
    Some(Scale { source, factor, op, constant })
}

/// `x + amount`: an `add`, or a `sub` of a constant.
fn _offset(context: &Context, function: &Function, inst: InstId) -> Option<(Operand, u128)> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let (source, amount) = _value_and_constant(context, op, left, right)?;
    match op {
        BinaryOp::Add => Some((source, amount)),
        BinaryOp::Sub => Some((source, amount.wrapping_neg() & mask(width))),
        _ => None,
    }
}

/// `x op constant` for an associative bitwise `op`.
fn _bitwise(context: &Context, function: &Function, inst: InstId) -> Option<(BinaryOp, Operand, u128)> {
    let (op, left, right, _) = _binary(context, function, inst)?;
    if !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) {
        return None;
    }
    let (source, constant) = _value_and_constant(context, op, left, right)?;
    Some((op, source, constant))
}

/// Values advancing by a constant each trip of some loop, where a mask
/// feeds a scale, as `_mask_scaled` asks.
fn _recurrences(context: &Context, layout: &DataLayout, function: &Function, outer: &Outer) -> BTreeSet<ValueId> {
    let masked_scale = function.walk().any(|(_, inst)| {
        _scale(context, function, inst).and_then(|scale| _definition(function, scale.source)).is_some_and(|mask| _bitwise(context, function, mask).is_some_and(|(op, ..)| op == BinaryOp::And))
    });
    if !masked_scale {
        return BTreeSet::new();
    }
    let analysed = Unit::within(context, layout, function, outer);
    analysed.shape().loops.iter().flat_map(|one| induction::advances(&analysed, one).into_keys()).collect()
}

/// `(x & m) * 2^k` is `(x * 2^k) & (m * 2^k)` at every width, where the
/// mask is read only by the scale. Only where `x` advances with a loop,
/// whose step then takes the scale for nothing; elsewhere the scale is as
/// free in an address as it would be in the mask.
fn _mask_scaled(context: &mut Context, function: &mut Function, recurrences: &BTreeSet<ValueId>, inst: InstId) -> bool {
    let Some(scale) = _scale(context, function, inst) else { return false };
    if !scale.factor.is_power_of_two() || !_single_use(function, scale.source) {
        return false;
    }
    let Some(mask_op) = _definition(function, scale.source) else { return false };
    let Some((BinaryOp::And, source, bits)) = _bitwise(context, function, mask_op) else { return false };
    if !matches!(source, Operand::Value(value) if recurrences.contains(&value)) {
        return false;
    }
    let width = _width(context, function, inst).expect("an integer");
    let moved = _constant(context, function, inst, bits.wrapping_mul(scale.factor) & mask(width));
    let scaled = _before(function, inst, Opcode::Binary(scale.op), vec![source, scale.constant]);
    _replace(function, inst, Opcode::Binary(BinaryOp::And), vec![scaled, moved]);
    true
}

/// `(x + a) * k + b` is `x * k + (a * k + b)` at every width, each op read
/// only by the next: the inner offset dies.
fn _offset_scaled(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((middle, outer)) = _offset(context, function, inst) else { return false };
    if !_single_use(function, middle) {
        return false;
    }
    let Some(scale) = _definition(function, middle).and_then(|made| _scale(context, function, made)) else { return false };
    if !_single_use(function, scale.source) {
        return false;
    }
    let Some((source, inner)) = _definition(function, scale.source).and_then(|made| _offset(context, function, made)) else { return false };
    let width = _width(context, function, inst).expect("an integer");
    let amount = _constant(context, function, inst, inner.wrapping_mul(scale.factor).wrapping_add(outer) & mask(width));
    let scaled = _before(function, inst, Opcode::Binary(scale.op), vec![source, scale.constant]);
    _replace(function, inst, Opcode::Binary(BinaryOp::Add), vec![scaled, amount]);
    true
}

/// `sext x` where `x`'s sign bit is known zero is `zext x`, as InstCombine's
/// `visitSExt`: `zext` has the cheaper selection and folds with masks.
fn _nonnegative_sext(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::SExt) {
        return false;
    }
    let source = instruction.operands[0];
    let Some(width) = function.operand_type(context, source).and_then(|ty| context.types.int_bits(ty)).filter(|&width| width <= 128) else { return false };
    if llrm_mir::valuetracking::known_zero(context, function, source) >> (width - 1) & 1 == 0 {
        return false;
    }
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![source]);
    true
}

/// `zext(and x, 2^k-1)`, the mask read by nothing else and `k` a native
/// integer width, is `zext(trunc x)`: a native narrow value is a register's
/// low part, where the `and` copies the register and masks it.
fn _masked_extension(context: &mut Context, layout: &DataLayout, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::ZExt) || !_single_use(function, instruction.operands[0]) {
        return false;
    }
    let Some(made) = _definition(function, instruction.operands[0]) else { return false };
    let Some((BinaryOp::And, left, right, wide)) = _binary(context, function, made) else { return false };
    let Some((value, mask)) = _value_and_constant(context, BinaryOp::And, left, right) else { return false };
    let low = mask.trailing_ones();
    if !layout.legal_integer(low) || low >= wide || mask != (1_u128 << low) - 1 {
        return false;
    }
    let narrow = context.types.int(low);
    let cut = function.create_instruction(Opcode::Cast(CastOp::Trunc), narrow, vec![value], Flags::default(), None);
    function.insert(cut, Position::Before(inst)).expect("a placed instruction");
    let cut = Operand::Value(function.instruction(cut).result.expect("a value"));
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![cut]);
    true
}

/// Two integer casts that are one, or none: `trunc(ext x)` is `x`, a
/// narrower `trunc x` or a narrower `ext x`; `trunc(trunc x)` is one
/// `trunc`; an extension of a like extension, or `sext` of a `zext`, whose
/// sign bit is clear, is one extension. `ext(trunc(ext x))` is two steps.
fn _cast_pair(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    if _address_round_trip(context, function, inst) {
        return true;
    }
    let cast = |function: &Function, inst: InstId| match function.instruction(inst).opcode {
        Opcode::Cast(op @ (CastOp::Trunc | CastOp::ZExt | CastOp::SExt)) => Some((op, function.instruction(inst).operands[0])),
        _ => None,
    };
    let Some((outer, middle)) = cast(function, inst) else { return false };
    let Some((inner, source)) = _definition(function, middle).and_then(|made| cast(function, made)) else { return false };
    let bits = |operand: Operand| function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty));
    let result = Operand::Value(function.instruction(inst).result.expect("a value"));
    let (Some(from), Some(to)) = (bits(source), bits(result)) else { return false };
    // `zext(trunc x)` where the bits `trunc` drops are known zero reads `x`.
    if (inner, outer) == (CastOp::Trunc, CastOp::ZExt) && from <= 128 {
        let kept = bits(middle).unwrap_or(from);
        let dropped = (u128::MAX >> (128 - from)) & !(u128::MAX >> (128 - kept));
        if llrm_mir::valuetracking::known_zero(context, function, source) & dropped == dropped {
            if to == from {
                _forward(function, inst, source);
            } else {
                _replace(function, inst, Opcode::Cast(if to > from { CastOp::ZExt } else { CastOp::Trunc }), vec![source]);
            }
            return true;
        }
    }
    let op = match (inner, outer) {
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if to == from => {
            _forward(function, inst, source);
            return true;
        }
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if to < from => CastOp::Trunc,
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) | (CastOp::ZExt, CastOp::ZExt | CastOp::SExt) | (CastOp::SExt, CastOp::SExt) => inner,
        (CastOp::Trunc, CastOp::Trunc) => CastOp::Trunc,
        _ => return false,
    };
    _replace(function, inst, Opcode::Cast(op), vec![source]);
    true
}

/// A byte offset from a constant near address is the constant address,
/// `inttoptr`, as InstCombine's constant folder makes it: `gep i8, ptr null,
/// -4` is `inttoptr (i16 -4 to ptr)`. Isel has one form for such an address.
fn _constant_address(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    // `inttoptr` of a constant is the constant expression.
    if let (&Opcode::Cast(CastOp::IntToPtr), [Operand::Constant(number)], Some(result)) = (&instruction.opcode, &instruction.operands[..], instruction.result)
        && matches!(context.types.get(instruction.ty), llrm_mir::types::Type::Pointer(0))
        && matches!(context.get(*number).kind, ConstantKind::Int(_))
        && context.types.int_bits(context.get(*number).ty) == Some(16)
    {
        let address = context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value: *number }) });
        function.replace_all_uses_with(result, Operand::Constant(address));
        _erase(function, inst);
        return true;
    }
    let Opcode::GetElementPtr { source } = instruction.opcode else { return false };
    let [Operand::Constant(base), Operand::Constant(index)] = instruction.operands[..] else { return false };
    let (Some(result), pointer) = (instruction.result, instruction.ty) else { return false };
    let near = matches!(context.types.get(pointer), llrm_mir::types::Type::Pointer(0));
    let (Some(step), Some(8)) = (_integer(context, Operand::Constant(index)), context.types.int_bits(source)) else { return false };
    let index_ty = context.get(index).ty;
    let Some(width) = context.types.int_bits(index_ty).filter(|&width| near && width == 16) else { return false };
    let start = match context.get(base).kind.clone() {
        ConstantKind::Null | ConstantKind::Zero => 0,
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => match context.get(value).kind {
            ConstantKind::Int(bits) if context.get(value).ty == index_ty => bits,
            _ => return false,
        },
        _ => return false,
    };
    let at = context.int(index_ty, (start.wrapping_add(step) & mask(width)) as i128);
    let address = context.constant(Constant { ty: pointer, kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value: at }) });
    function.replace_all_uses_with(result, Operand::Constant(address));
    _erase(function, inst);
    true
}

/// `ptrtoint(inttoptr x)` to x's own width is `x`, as InstCombine folds it:
/// a far null compared as an integer is the constant 0 again.
fn _address_round_trip(context: &Context, function: &mut Function, inst: InstId) -> bool {
    if function.instruction(inst).opcode != Opcode::Cast(CastOp::PtrToInt) {
        return false;
    }
    let Some(made) = _definition(function, function.instruction(inst).operands[0]) else { return false };
    if function.instruction(made).opcode != Opcode::Cast(CastOp::IntToPtr) {
        return false;
    }
    let source = function.instruction(made).operands[0];
    let result = Operand::Value(function.instruction(inst).result.expect("a value"));
    if function.operand_type(context, source) != function.operand_type(context, result) {
        return false;
    }
    _forward(function, inst, source);
    true
}

/// Logic of two like extensions from one type is that extension of the
/// logic at the narrow type, as InstCombine's `foldCastedBitwiseLogic`:
/// two frontend truths `and`ed, then tested, become one `i1` `and`.
fn _casted_logic(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor), left, right, _)) = _binary(context, function, inst) else { return false };
    let extended = |operand: Operand| {
        let made = _definition(function, operand)?;
        match function.instruction(made).opcode {
            Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) => Some((kind, function.instruction(made).operands[0])),
            _ => None,
        }
    };
    let (Some((kind, one)), Some((other, two))) = (extended(left), extended(right)) else { return false };
    let narrow = function.operand_type(context, one);
    if kind != other || narrow.is_none() || narrow != function.operand_type(context, two) || !(_single_use(function, left) || _single_use(function, right)) {
        return false;
    }
    let logic = function.create_instruction(Opcode::Binary(op), narrow.expect("typed"), vec![one, two], Flags::default(), None);
    function.insert(logic, Position::Before(inst)).expect("a placed instruction");
    let logic = Operand::Value(function.instruction(logic).result.expect("a value"));
    _replace(function, inst, Opcode::Cast(kind), vec![logic]);
    true
}

/// A phi of like extensions from one type, each read by it alone, is that
/// extension of a phi of their sources, as InstCombine's
/// `foldPHIArgOpIntoPHI`: an `&&`'s truths as frontend bytes, then tested,
/// become one `i1` phi a branch reads.
fn _phi_of_casts(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    if function.instruction(inst).opcode != Opcode::Phi {
        return false;
    }
    let incoming = crate::lcssa::arms(function, inst);
    let extended = |operand: Operand| {
        let made = _definition(function, operand)?;
        let op = function.instruction(made);
        match op.opcode {
            Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) if _single_use(function, operand) => Some((kind, op.operands[0])),
            _ => None,
        }
    };
    let Some(sources) = incoming.iter().map(|&(value, _)| extended(value)).collect::<Option<Vec<_>>>() else { return false };
    let Some(&(kind, first)) = sources.first() else { return false };
    let narrow = function.operand_type(context, first);
    if narrow.is_none() || sources.iter().any(|&(other, source)| other != kind || function.operand_type(context, source) != narrow) {
        return false;
    }
    let arms = sources.iter().zip(&incoming).map(|(&(_, source), &(_, from))| (source, from)).collect::<Vec<_>>();
    let phi = function.create_instruction(Opcode::Phi, narrow.expect("typed"), crate::lcssa::from_arms(&arms), Flags::default(), None);
    function.insert(phi, Position::Before(inst)).expect("a placed phi");
    let block = function.parent(inst).expect("a placed phi");
    let first = function.block(block).instructions().iter().copied().find(|&one| function.instruction(one).opcode != Opcode::Phi).expect("a terminated block");
    let cast = function.create_instruction(Opcode::Cast(kind), function.instruction(inst).ty, vec![Operand::Value(function.instruction(phi).result.expect("a value"))], Flags::default(), None);
    function.insert(cast, Position::Before(first)).expect("a placed instruction");
    _forward(function, inst, Operand::Value(function.instruction(cast).result.expect("a value")));
    true
}

/// A phi that takes the values of an earlier phi of its block, from the same
/// predecessors, is that phi: SimplifyCFG's `EliminateDuplicatePHINodes`.
/// gvn's partial redundancy elimination made two where one stood, and the
/// loop carried both in registers (SPHEREMAPLASMA, #386).
fn _duplicate_phi(function: &mut Function, inst: InstId) -> bool {
    if function.instruction(inst).opcode != Opcode::Phi {
        return false;
    }
    let block = function.parent(inst).expect("a placed phi");
    let mine = &function.instruction(inst).operands;
    let ty = function.instruction(inst).ty;
    // One arm per predecessor: the same count, and each arm of this one among the other's.
    let earlier = function.block(block).instructions().iter().copied().take_while(|&one| one != inst).find(|&one| {
        let other = function.instruction(one);
        other.opcode == Opcode::Phi && other.ty == ty && other.operands.len() == mine.len() && mine.chunks(2).all(|arm| other.operands.chunks(2).any(|theirs| theirs == arm))
    });
    let Some(earlier) = earlier else { return false };
    let with = Operand::Value(function.instruction(earlier).result.expect("a phi's value"));
    _forward(function, inst, with);
    true
}

/// Negating a single-use modular difference reverses its operands.
fn _negated_difference(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((BinaryOp::Sub, zero, difference, _)) = _binary(context, function, inst) else { return false };
    if _integer(context, zero) != Some(0) || !_single_use(function, difference) {
        return false;
    }
    let Some(made) = _definition(function, difference) else { return false };
    let Some((BinaryOp::Sub, left, right, _)) = _binary(context, function, made) else { return false };
    _replace(function, inst, Opcode::Binary(BinaryOp::Sub), vec![right, left]);
    true
}

/// `(x << a) << b` is `x << (a + b)` while that is short of the width.
fn _shift_chain(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((BinaryOp::Shl, middle, Operand::Constant(_), width)) = _binary(context, function, inst) else { return false };
    if !_single_use(function, middle) {
        return false;
    }
    let Some(previous) = _definition(function, middle) else { return false };
    let Some((BinaryOp::Shl, source, first @ Operand::Constant(_), _)) = _binary(context, function, previous) else { return false };
    let (Some(first), Some(last)) = (_integer(context, first), _integer(context, function.instruction(inst).operands[1])) else { return false };
    let total = first.saturating_add(last);
    if first.min(last) == 0 || total >= u128::from(width) {
        return false;
    }
    let total = _constant(context, function, inst, total);
    _replace(function, inst, Opcode::Binary(BinaryOp::Shl), vec![source, total]);
    true
}

/// Two single-use scales are one `mul` at an unchanged modular width.
fn _scaled_chain(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some(last) = _scale(context, function, inst) else { return false };
    if !_single_use(function, last.source) {
        return false;
    }
    let Some(first) = _definition(function, last.source).and_then(|made| _scale(context, function, made)) else { return false };
    let width = _width(context, function, inst).expect("an integer");
    let factor = _constant(context, function, inst, first.factor.wrapping_mul(last.factor) & mask(width));
    _replace(function, inst, Opcode::Binary(BinaryOp::Mul), vec![first.source, factor]);
    true
}

/// Two single-use offsets are one `add` at an unchanged modular width.
fn _offset_chain(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((middle, last)) = _offset(context, function, inst) else { return false };
    if !_single_use(function, middle) {
        return false;
    }
    let Some((source, first)) = _definition(function, middle).and_then(|made| _offset(context, function, made)) else { return false };
    let width = _width(context, function, inst).expect("an integer");
    let amount = _constant(context, function, inst, first.wrapping_add(last) & mask(width));
    _replace(function, inst, Opcode::Binary(BinaryOp::Add), vec![source, amount]);
    true
}

/// Two single-use `and`s, `or`s or `xor`s of constants are one.
fn _bitwise_chain(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some((op, middle, last)) = _bitwise(context, function, inst) else { return false };
    if !_single_use(function, middle) {
        return false;
    }
    let Some((first_op, source, first)) = _definition(function, middle).and_then(|made| _bitwise(context, function, made)) else { return false };
    if first_op != op {
        return false;
    }
    let combined = match op {
        BinaryOp::And => first & last,
        BinaryOp::Or => first | last,
        _ => first ^ last,
    };
    let combined = _constant(context, function, inst, combined);
    _replace(function, inst, Opcode::Binary(op), vec![source, combined]);
    true
}

fn _identity(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let Some(answer) = identity(context, function, inst) else { return false };
    _forward(function, inst, answer);
    true
}

/// The operand `inst` equals by an identity: `x + 0`, `x - 0`, `x | 0`,
/// `x ^ 0`, a shift by 0, `x * 1`, `x / 1` and `x & -1` are `x`; `x * 0` and `x & 0`
/// are 0, and `x | -1` is -1.
pub fn identity(context: &Context, function: &Function, inst: InstId) -> Option<Operand> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let commutes = matches!(op, BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor);
    let pairs = [(left, right), (right, left)];
    pairs[..1 + usize::from(commutes)].iter().find_map(|&(kept, other)| {
        let number = _integer(context, other)?;
        let all = mask(width);
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if number == 0 => Some(kept),
            BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::SDiv if number == 1 => Some(kept),
            BinaryOp::And if number == all => Some(kept),
            BinaryOp::Mul | BinaryOp::And if number == 0 => Some(other),
            BinaryOp::Or if number == all => Some(other),
            _ => None,
        }
    })
}

/// `x % 1` is 0, a comparison of a value with itself is decided by its predicate,
/// and `x >= 0` or `x < 0` unsigned is true or false.
fn _decided(context: &mut Context, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let answer = match instruction.opcode {
        Opcode::Binary(BinaryOp::URem | BinaryOp::SRem) if _integer(context, instruction.operands[1]) == Some(1) => Some(0),
        Opcode::ICmp(predicate) => {
            let (left, right) = (instruction.operands[0], instruction.operands[1]);
            if left == right && matches!(left, Operand::Value(_)) {
                Some(u128::from(matches!(predicate, IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle)))
            } else {
                match predicate {
                    IntPredicate::Uge if _integer(context, right) == Some(0) => Some(1),
                    IntPredicate::Ult if _integer(context, right) == Some(0) => Some(0),
                    _ => None,
                }
            }
        }
        _ => None,
    };
    let Some(bits) = answer else { return false };
    let decided = _constant(context, function, inst, bits);
    _forward(function, inst, decided);
    true
}

/// A `select` on a constant condition is the arm it chooses.
fn _selected(context: &Context, function: &mut Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Select {
        return false;
    }
    let Some(bits) = _integer(context, instruction.operands[0]) else { return false };
    let chosen = instruction.operands[if bits & 1 == 1 { 1 } else { 2 }];
    _forward(function, inst, chosen);
    true
}

/// Put a loop-carried operand at the root of an integer `add` tree:
/// `(phi + a) + b` is `phi + (a + b)`, whose inner sum may be invariant.
///
/// Only a two-level tree in one block, its inner `add` read only by the
/// outer, whose value is what a latch carries into that phi.
fn _reassociated_recurrences(function: &mut Function) -> bool {
    let mut updates = BTreeMap::<ValueId, ValueId>::new();
    for one in cfg::Shape::of(function).loops {
        for &phi in function.block(cfg::block(one.header)).instructions() {
            let instruction = function.instruction(phi);
            if instruction.opcode != Opcode::Phi {
                break;
            }
            for pair in instruction.operands.chunks(2) {
                if let (Operand::Value(value), Operand::Block(from)) = (pair[0], pair[1])
                    && one.latches.contains(&cfg::id(from))
                {
                    updates.insert(value, instruction.result.expect("a phi's value"));
                }
            }
        }
    }
    let mut changed = false;
    for (value, recurrence) in updates {
        let ValueDef::Instruction(outer) = function.value(value).def else { continue };
        let plain = |function: &Function, inst: InstId| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Add);
        let recurrence = Operand::Value(recurrence);
        if function.is_erased(outer) || !plain(function, outer) || function.instruction(outer).operands.contains(&recurrence) {
            continue;
        }
        for position in 0..2 {
            let candidate = function.instruction(outer).operands[position];
            let Some(inner) = _definition(function, candidate) else { continue };
            if !plain(function, inner) || function.parent(inner) != function.parent(outer) || !_single_use(function, candidate) {
                continue;
            }
            let operands = function.instruction(inner).operands.clone();
            if operands.iter().filter(|one| **one == recurrence).count() != 1 {
                continue;
            }
            let leaf = if operands[0] == recurrence { operands[1] } else { operands[0] };
            let other = function.instruction(outer).operands[1 - position];
            function.set_operands(inner, vec![leaf, other]);
            function.set_flags(inner, Flags::default());
            // `other` may be defined between the two.
            function.move_to(inner, Position::Before(outer)).expect("a placed instruction");
            function.set_operands(outer, vec![recurrence, candidate]);
            function.set_flags(outer, Flags::default());
            changed = true;
            break;
        }
    }
    changed
}

/// Reuse a smaller shift of the same value, read elsewhere, in the same
/// block: `x << 3` after a used `t = x << 1` is `t << 2`.
fn _shared_shifts(context: &mut Context, function: &mut Function) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut available = IndexMap::<Operand, BTreeMap<u128, ValueId>>::default();
        for inst in function.block(block).instructions().to_vec() {
            let Some(Scale { source, op: BinaryOp::Shl, constant, .. }) = _scale(context, function, inst) else { continue };
            let count = _integer(context, constant).expect("a constant count");
            let candidates = available.entry(source).or_default();
            if let Some((&amount, &previous)) = candidates.range(..count).next_back() {
                let rest = _constant(context, function, inst, count - amount);
                function.set_operands(inst, vec![Operand::Value(previous), rest]);
                function.set_flags(inst, Flags::default());
                changed = true;
            }
            let result = function.instruction(inst).result.expect("a value");
            if !function.users(result).is_empty() {
                candidates.insert(count, result);
            }
        }
    }
    changed
}

/// A fixed-point product or quotient by a whole number `k`, its fraction
/// bits all clear: the scale cancels, leaving `x * k` or `x / k`. A
/// quotient by -1 stays, where the least value wraps and `sdiv` would be
/// undefined.
fn _whole_fixed(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> bool {
    let unit = Unit::within(context, layout, function, outer);
    let whole: Vec<(InstId, BinaryOp, i128)> = function
        .walk()
        .filter_map(|(_, inst)| {
            let Some(Intrinsic::Fixed { divide }) = unit.intrinsic(inst) else { return None };
            let operands = &function.instruction(inst).operands;
            let (width, scale) = (_width(context, function, inst)?, _integer(context, operands[2])?);
            let factor = _integer(context, operands[1])?;
            let signed = ((factor << (128 - width)) as i128) >> (128 - width);
            let whole = signed >> scale;
            let clear = scale < u128::from(width) && factor & ((1 << scale) - 1) == 0 && whole != 0;
            (clear && !(divide && whole == -1)).then_some((inst, if divide { BinaryOp::SDiv } else { BinaryOp::Mul }, whole))
        })
        .collect();
    for &(inst, op, whole) in &whole {
        let x = function.instruction(inst).operands[0];
        let by = _constant(context, function, inst, whole as u128);
        _replace(function, inst, Opcode::Binary(op), vec![x, by]);
    }
    !whole.is_empty()
}

/// Divide by a positive power of two, biasing a negative dividend to
/// truncate toward zero: `sdiv` and `srem` by a divisor consts proves, at
/// a legal integer width, where the shifts cost less than the division.
fn _divisions(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer, facts: &IndexMap<ValueId, Known>) -> bool {
    let unit = Unit::within(context, layout, function, outer);
    let divisors: Vec<(InstId, u32)> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)))
        .filter_map(|inst| {
            let width = _width(context, function, inst).filter(|&width| layout.legal_integer(width))?;
            let fact = consts::_operand(&unit, function.instruction(inst).operands[1], facts, None)?;
            let divisor = u128::try_from(consts::masked(&fact.n, width)).ok().filter(|_| fact.width >= width)?;
            (divisor.is_power_of_two() && divisor > 1 && divisor < 1 << (width - 1)).then(|| (inst, divisor.trailing_zeros()))
        })
        .collect();
    for &(inst, shift) in &divisors {
        if function.is_erased(inst) {
            continue;
        }
        let width = _width(context, function, inst).expect("an integer");
        let dividend = function.instruction(inst).operands[0];
        let binary = |op| Opcode::Binary(op);
        let top = _constant(context, function, inst, u128::from(width - 1));
        let sign = _before(function, inst, binary(BinaryOp::AShr), vec![dividend, top]);
        let adjusted = if shift == 1 {
            // The bias is the sign's low bit, 0 or 1: subtracting the sign word adds it.
            _before(function, inst, binary(BinaryOp::Sub), vec![dividend, sign])
        } else {
            let low = _constant(context, function, inst, (1 << shift) - 1);
            let bias = _before(function, inst, binary(BinaryOp::And), vec![sign, low]);
            _before(function, inst, binary(BinaryOp::Add), vec![dividend, bias])
        };
        let count = _constant(context, function, inst, u128::from(shift));
        let quotient = _before(function, inst, binary(BinaryOp::AShr), vec![adjusted, count]);
        if function.instruction(inst).opcode == binary(BinaryOp::SDiv) {
            _forward(function, inst, quotient);
        } else {
            let product = _before(function, inst, binary(BinaryOp::Shl), vec![quotient, count]);
            _replace(function, inst, binary(BinaryOp::Sub), vec![dividend, product]);
        }
    }
    !divisors.is_empty()
}

#[cfg(test)]
#[path = "algebraic_tests.rs"]
mod tests;
