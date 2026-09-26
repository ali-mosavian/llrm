//! Instructions made simpler, to a fixed point: LLVM's InstSimplify, which
//! answers with a value that already exists, and the part of InstCombine
//! that rewrites one instruction into a cheaper or more canonical one.
//! Constants fold through the interpreter's arithmetic, so what an
//! operation means is decided in one place.

use crate::context::{Constant, ConstantKind, Context, mask, signed};
use crate::dominators::DominatorTree;
use crate::edit::Position;
use crate::interpret::{self, Val};
use crate::module::{Function, InstId, Operand, ValueDef};
use crate::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, PreservedAnalyses, Unit};
use crate::types::{Type, TypeId};

pub struct InstCombine;

impl FunctionPass for InstCombine {
    fn name(&self) -> &'static str {
        "instcombine"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let mut changed = false;
        loop {
            let mut round = false;
            for (_, inst) in unit.function.walk().collect::<Vec<_>>() {
                if unit.function.is_erased(inst) {
                    continue;
                }
                if dead(unit.function, inst) {
                    unit.function.erase(inst).expect("nothing uses it");
                } else if let Some(simpler) = simplified(unit, &tree, inst) {
                    let result = unit.function.instruction(inst).result.expect("a simplified value");
                    unit.function.replace_all_uses_with(result, simpler);
                    unit.function.erase(inst).expect("its uses were replaced");
                } else if !combined(unit, inst) {
                    continue;
                }
                round = true;
            }
            if !round {
                break;
            }
            changed = true;
        }
        // Blocks and edges are as they were.
        if changed { PreservedAnalyses::none().preserve::<Dominators>() } else { PreservedAnalyses::all() }
    }
}

/// An instruction nothing reads, whose only effect is its value.
fn dead(function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let pure = matches!(
        instruction.opcode,
        Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select
            | Opcode::FNeg | Opcode::ExtractValue(_) | Opcode::InsertValue(_) | Opcode::Freeze | Opcode::Alloca { .. } | Opcode::Load { volatile: false, .. }
    );
    pure && instruction.result.is_some_and(|result| function.users(result).is_empty())
}

/// A constant operand as the interpreter holds values.
fn value(context: &Context, operand: Operand) -> Option<Val> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Int(bits), Type::Int(width)) => Some(Val::Int { bits: *bits, width: *width }),
        (ConstantKind::Float(bits), Type::Float(kind)) => Some(Val::Float(*kind, *bits)),
        (ConstantKind::Poison, _) => Some(Val::Poison),
        _ => None,
    }
}

fn constant(context: &mut Context, ty: TypeId, value: Val) -> Option<Operand> {
    let kind = match value {
        Val::Int { bits, .. } => ConstantKind::Int(bits),
        Val::Float(_, bits) => ConstantKind::Float(bits),
        Val::Poison => ConstantKind::Poison,
        _ => return None,
    };
    Some(Operand::Constant(context.constant(Constant { ty, kind })))
}

fn int(operand: Operand, context: &Context) -> Option<(u128, u32)> {
    match value(context, operand)? {
        Val::Int { bits, width } => Some((bits, width)),
        _ => None,
    }
}

/// A value that already exists and equals `inst`'s: InstSimplify.
fn simplified(unit: &mut Unit, tree: &DominatorTree, inst: InstId) -> Option<Operand> {
    let ty = unit.function.instruction(inst).ty;
    let integer = unit.context.types.int_bits(ty).is_some();
    let (zero, one) = (integer.then(|| Operand::Constant(unit.context.int(ty, 0))), integer.then(|| Operand::Constant(unit.context.int(ty, 1))));
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    let operands = instruction.operands.clone();
    let context = &*unit.context;
    let is = |operand: Operand, n: i128| int(operand, context).is_some_and(|(bits, width)| bits == (n as u128 & mask(width)));
    match instruction.opcode.clone() {
        Opcode::Binary(op) => {
            let (a, b) = (operands[0], operands[1]);
            if let (Some(x), Some(y)) = (value(context, a), value(context, b)) {
                let folded = interpret::binary(op, instruction.flags, x, y).ok()?;
                return constant(unit.context, ty, folded);
            }
            match op {
                BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if is(b, 0) => Some(a),
                BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::SDiv if is(b, 1) => Some(a),
                BinaryOp::Mul | BinaryOp::And if is(b, 0) => Some(b),
                BinaryOp::And if is(b, -1) => Some(a),
                BinaryOp::Or if is(b, -1) => Some(b),
                BinaryOp::And | BinaryOp::Or if a == b => Some(a),
                BinaryOp::Sub | BinaryOp::Xor if a == b => zero,
                BinaryOp::URem if is(b, 1) => zero,
                BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if is(a, 0) => Some(a),
                _ => None,
            }
        }
        Opcode::ICmp(predicate) => {
            let (a, b) = (operands[0], operands[1]);
            if let (Some(x), Some(y)) = (value(context, a), value(context, b)) {
                return constant(unit.context, ty, interpret::icmp(predicate, x, y));
            }
            let truth = |value: bool| if value { one } else { zero };
            let bool_type = function.operand_type(context, a).is_some_and(|one| context.types.int_bits(one) == Some(1));
            match predicate {
                _ if a == b => truth(matches!(predicate, IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle)),
                // An i1 compared with what makes it itself.
                IntPredicate::Ne if bool_type && is(b, 0) => Some(a),
                IntPredicate::Eq if bool_type && is(b, 1) => Some(a),
                IntPredicate::Uge if is(b, 0) => truth(true),
                IntPredicate::Ult if is(b, 0) => truth(false),
                _ => None,
            }
        }
        Opcode::Cast(op) => {
            let from = function.operand_type(context, operands[0])?;
            if let Some(x) = value(context, operands[0]) {
                let folded = interpret::cast(&context.types, unit.layout, op, x, from, ty, instruction.flags).ok()?;
                return constant(unit.context, ty, folded);
            }
            // An extension undone.
            let Operand::Value(source) = operands[0] else { return None };
            let ValueDef::Instruction(inner) = function.value(source).def else { return None };
            let inner = function.instruction(inner);
            match (op, &inner.opcode) {
                (CastOp::Trunc, Opcode::Cast(CastOp::ZExt | CastOp::SExt)) if function.operand_type(context, inner.operands[0]) == Some(ty) => Some(inner.operands[0]),
                _ => None,
            }
        }
        Opcode::Phi => {
            // Every input the same value, or the phi itself.
            let own = Operand::Value(instruction.result?);
            let mut inputs = operands.chunks(2).map(|pair| pair[0]).filter(|one| *one != own);
            let first = inputs.next()?;
            if !inputs.all(|one| one == first) {
                return None;
            }
            match first {
                Operand::Value(one) => match function.value(one).def {
                    ValueDef::Argument(_) => Some(first),
                    ValueDef::Instruction(def) => tree.instruction_dominates(function, def, inst).then_some(first),
                },
                _ => Some(first),
            }
        }
        _ => None,
    }
}

/// `inst` rewritten in place, or replaced by one new instruction: the
/// canonical forms later rules and passes expect.
fn combined(unit: &mut Unit, inst: InstId) -> bool {
    let function = &*unit.function;
    let context = &*unit.context;
    let instruction = function.instruction(inst).clone();
    let ty = instruction.ty;
    let operands = instruction.operands.clone();
    let constant_operand = |operand: Operand| matches!(operand, Operand::Constant(_));
    let defined = |operand: Operand| match operand {
        Operand::Value(one) => match function.value(one).def {
            ValueDef::Instruction(def) => Some((def, function.instruction(def).clone())),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    };
    match instruction.opcode {
        // A constant goes to the right of an operation that commutes, and of
        // a comparison, its predicate swapped.
        Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) | Opcode::ICmp(_)
            if constant_operand(operands[0]) && !constant_operand(operands[1]) =>
        {
            if let Opcode::ICmp(predicate) = instruction.opcode {
                let swapped = unit.function.create_instruction(Opcode::ICmp(predicate.swapped()), ty, vec![operands[1], operands[0]], instruction.flags, None);
                return replaced(unit, inst, swapped);
            }
            unit.function.set_operand(inst, 0, operands[1]);
            unit.function.set_operand(inst, 1, operands[0]);
            true
        }
        // x - c is x + -c, which reassociates.
        Opcode::Binary(BinaryOp::Sub) if int(operands[1], context).is_some() => {
            let (bits, width) = int(operands[1], context).expect("an integer");
            let negated = Operand::Constant(unit.context.int(ty, -signed(bits, width)));
            let add = unit.function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![operands[0], negated], Flags::default(), None);
            replaced(unit, inst, add)
        }
        // x * 2^k is x << k.
        Opcode::Binary(BinaryOp::Mul) if int(operands[1], context).is_some_and(|(bits, _)| bits.is_power_of_two() && bits > 1) => {
            let (bits, _) = int(operands[1], context).expect("an integer");
            let count = Operand::Constant(unit.context.int(ty, i128::from(bits.trailing_zeros())));
            let shift = unit.function.create_instruction(Opcode::Binary(BinaryOp::Shl), ty, vec![operands[0], count], Flags::default(), None);
            replaced(unit, inst, shift)
        }
        // (x op c1) op c2 is x op (c1 op c2), for an operation that associates.
        Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) if constant_operand(operands[1]) => {
            let Some((_, inner)) = defined(operands[0]) else { return false };
            if inner.opcode != Opcode::Binary(op) || !constant_operand(inner.operands[1]) {
                return false;
            }
            let (Some(c1), Some(c2)) = (value(context, inner.operands[1]), value(context, operands[1])) else { return false };
            let Some(folded) = interpret::binary(op, Flags::default(), c1, c2).ok().and_then(|one| constant(unit.context, ty, one)) else { return false };
            unit.function.set_operand(inst, 0, inner.operands[0]);
            unit.function.set_operand(inst, 1, folded);
            unit.function.set_flags(inst, Flags::default());
            true
        }
        // An extension keeps whether a value is zero.
        Opcode::ICmp(predicate @ (IntPredicate::Eq | IntPredicate::Ne)) if int(operands[1], context).is_some_and(|(bits, _)| bits == 0) => {
            let Some((_, inner)) = defined(operands[0]) else { return false };
            if !matches!(inner.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt)) {
                return false;
            }
            let narrow = function.operand_type(context, inner.operands[0]).expect("a typed operand");
            let zero = Operand::Constant(unit.context.int(narrow, 0));
            let compare = unit.function.create_instruction(Opcode::ICmp(predicate), ty, vec![inner.operands[0], zero], Flags::default(), None);
            replaced(unit, inst, compare)
        }
        // Two casts that are one.
        Opcode::Cast(outer) => {
            let Some((_, inner)) = defined(operands[0]) else { return false };
            let Opcode::Cast(first) = inner.opcode else { return false };
            let source = inner.operands[0];
            let bits = |operand: Operand| function.operand_type(context, operand).and_then(|one| context.types.int_bits(one));
            let (Some(from), Some(to)) = (bits(source), context.types.int_bits(ty)) else { return false };
            let op = match (first, outer) {
                (CastOp::ZExt, CastOp::ZExt) | (CastOp::SExt, CastOp::SExt) | (CastOp::ZExt, CastOp::SExt) => first,
                (CastOp::Trunc, CastOp::Trunc) => CastOp::Trunc,
                (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if from < to => first,
                (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if from > to => CastOp::Trunc,
                _ => return false,
            };
            let cast = unit.function.create_instruction(Opcode::Cast(op), ty, vec![source], Flags::default(), None);
            replaced(unit, inst, cast)
        }
        _ => false,
    }
}

/// `inst` replaced by `new`, placed where it was.
fn replaced(unit: &mut Unit, inst: InstId, new: InstId) -> bool {
    let function = &mut *unit.function;
    function.insert(new, Position::Before(inst)).expect("a placed instruction");
    let (old, value) = (function.instruction(inst).result.expect("a value"), function.instruction(new).result.expect("a value"));
    function.replace_all_uses_with(old, Operand::Value(value));
    function.erase(inst).expect("its uses were replaced");
    true
}
