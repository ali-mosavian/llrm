//! LLVM's SeparateConstOffsetFromGEP: the constant an index adds, under a
//! scale or through an extension its flags allow, moves into a `gep` of its
//! own, where every address mode takes it as a displacement.
//!
//! ```text
//! gep T, p, (i + 8) * 2   =>   gep T, (gep i8, p, 32), i * 2
//! ```
//!
//! An index is `rest + constant`: the walk goes through add, sub, a constant
//! multiply or shift, truncation, and a `sext` or `zext` only over adds
//! the matching `nsw` or `nuw` keeps from wrapping (and not over a multiply:
//! the rest would wrap where the whole does not). An index narrower than
//! its pointer's is sign-extended by the `gep`, so it is read as under a
//! `sext`. The width is the address space's, from the datalayout.
//!
//! The constant `gep` is free where isel folds it, so a split is kept when
//! the instructions it makes cost no more than those it leaves unread
//! (`profit::operation`): `i + 8` read by two addresses is split in both for
//! nothing, `(i + 8) * 2` for a multiply. Neither new `gep` is `inbounds`:
//! the constant may take the first outside the object.
//!
//! It runs last, after `hoist`: a hoisted constant `gep` would be a register
//! held across the loop instead of a displacement.

use std::collections::BTreeSet;

use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::types::Type;
use num_bigint::BigInt;

use crate::counting;
use crate::profit;

pub struct GepOffset;

impl FunctionPass for GepOffset {
    fn name(&self) -> &'static str {
        "gepoffset"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if separated(unit, analyses) { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

/// `index = rest + constant`, modulo the index's width; no `rest` is zero.
struct Split {
    rest: Option<Operand>,
    constant: BigInt,
    /// Whether it is the operand itself, nothing taken out.
    same: bool,
}

/// `operand`'s bits, where it is an integer constant.
fn integer(context: &Context, operand: Operand) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

struct Splitter<'a> {
    context: &'a mut Context,
    function: &'a mut Function,
    before: InstId,
    created: Vec<InstId>,
    /// What the split read and left changed: instructions it may leave unread.
    cone: Vec<InstId>,
}

impl Splitter<'_> {
    fn width(&self, operand: Operand) -> Option<u32> {
        self.function.operand_type(self.context, operand).and_then(|ty| self.context.types.int_bits(ty))
    }

    fn leaf(&self, operand: Operand) -> Split {
        Split { rest: Some(operand), constant: BigInt::from(0), same: true }
    }

    fn make(&mut self, opcode: Opcode, width: u32, operands: Vec<Operand>) -> Operand {
        let ty = self.context.types.int(width);
        let inst = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(inst, Position::Before(self.before)).expect("a position before the address");
        self.created.push(inst);
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    fn modulus(width: u32) -> BigInt {
        BigInt::from(1) << width
    }

    fn masked(value: &BigInt, width: u32) -> BigInt {
        ((value % Self::modulus(width)) + Self::modulus(width)) % Self::modulus(width)
    }

    /// `operand` as a rest and a constant. Under `exact`, the flag an
    /// extension needs of each add and sub it is read through: a rest summed
    /// without wrapping is the whole's, and nothing is read through a multiply.
    fn split(&mut self, operand: Operand, exact: Option<Flags>) -> Split {
        let Some(width) = self.width(operand) else { return self.leaf(operand) };
        let Operand::Value(value) = operand else {
            return match integer(self.context, operand) {
                Some(bits) => Split { rest: None, constant: BigInt::from(bits), same: false },
                None => self.leaf(operand),
            };
        };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return self.leaf(operand) };
        let op = self.function.instruction(inst).clone();
        let constant = |this: &Self, one: Operand| integer(this.context, one).map(|bits| BigInt::from(signed(bits, width)));
        let flagged = exact.is_none_or(|flag| op.flags.contains(flag));
        let split = match op.opcode {
            Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) if flagged => {
                let (first, second) = (self.split(op.operands[0], exact), self.split(op.operands[1], exact));
                if first.same && second.same {
                    return self.leaf(operand);
                }
                let negate = kind == BinaryOp::Sub;
                let total = if negate { &first.constant - &second.constant } else { &first.constant + &second.constant };
                let rest = match (first.rest, second.rest) {
                    (Some(one), Some(other)) => Some(self.make(Opcode::Binary(kind), width, vec![one, other])),
                    (one, None) => one,
                    (None, Some(other)) if !negate => Some(other),
                    (None, Some(other)) => {
                        let zero = counting::constant(self.context, &BigInt::from(0), width);
                        Some(self.make(Opcode::Binary(BinaryOp::Sub), width, vec![zero, other]))
                    }
                };
                Split { rest, constant: Self::masked(&total, width), same: false }
            }
            Opcode::Binary(kind @ (BinaryOp::Mul | BinaryOp::Shl)) if exact.is_none() => {
                let by = match (kind, constant(self, op.operands[1]), constant(self, op.operands[0])) {
                    (BinaryOp::Mul, Some(by), _) => Some((0, by)),
                    (BinaryOp::Mul, None, Some(by)) => Some((1, by)),
                    (BinaryOp::Shl, Some(count), _) if count >= BigInt::from(0) && count < BigInt::from(width) => Some((0, BigInt::from(1) << usize::try_from(&count).expect("a count below the width"))),
                    _ => None,
                };
                let Some((at, by)) = by else { return self.leaf(operand) };
                let inner = self.split(op.operands[at], exact);
                if inner.same {
                    return self.leaf(operand);
                }
                let by_operand = counting::constant(self.context, &by, width);
                let rest = inner.rest.map(|rest| self.make(Opcode::Binary(BinaryOp::Mul), width, vec![rest, by_operand]));
                Split { rest, constant: Self::masked(&(inner.constant * by), width), same: false }
            }
            Opcode::Cast(cast @ (CastOp::SExt | CastOp::ZExt)) => {
                let wanted = if cast == CastOp::SExt { Flags::NSW } else { Flags::NUW };
                if exact.is_some_and(|flag| flag != wanted) {
                    return self.leaf(operand);
                }
                let Some(narrow) = self.width(op.operands[0]) else { return self.leaf(operand) };
                let inner = self.split(op.operands[0], Some(wanted));
                if inner.same {
                    return self.leaf(operand);
                }
                let constant = if cast == CastOp::SExt { Self::masked(&BigInt::from(signed(u128::try_from(&inner.constant).expect("a word"), narrow)), width) } else { inner.constant };
                let rest = inner.rest.map(|rest| self.make(Opcode::Cast(cast), width, vec![rest]));
                Split { rest, constant, same: false }
            }
            // Truncation commutes with add and mul: the low bits of a rest and a constant.
            Opcode::Cast(CastOp::Trunc) if exact.is_none() => {
                let inner = self.split(op.operands[0], None);
                if inner.same {
                    return self.leaf(operand);
                }
                let rest = inner.rest.map(|rest| self.make(Opcode::Cast(CastOp::Trunc), width, vec![rest]));
                Split { rest, constant: Self::masked(&inner.constant, width), same: false }
            }
            _ => return self.leaf(operand),
        };
        self.cone.push(inst);
        split
    }
}

/// Every `gep` whose indices carry a constant, split where it pays; whether any was.
pub fn separated(unit: &mut passes::Unit, analyses: &Analyses) -> bool {
    let costs = profit::costs(analyses.outer());
    let callees = analyses.outer().callees();
    let geps = unit.function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::GetElementPtr { .. })).collect::<Vec<_>>();
    let mut changed = false;
    for gep in geps {
        if unit.function.is_erased(gep) {
            continue;
        }
        let instruction = unit.function.instruction(gep).clone();
        let Opcode::GetElementPtr { source } = instruction.opcode else { continue };
        let Type::Pointer(space) = *unit.context.types.get(instruction.ty) else { continue };
        let pointer_bits = unit.layout.pointer(space).index_bits;
        let known = instruction.operands[1..]
            .iter()
            .map(|&one| integer(unit.context, one).map(|bits| signed(bits, unit.context.types.int_bits(unit.function.operand_type(unit.context, one).expect("typed")).unwrap_or(64))))
            .collect::<Vec<_>>();
        let (_, variable) = unit.layout.collect_offset(&unit.context.types, source, &known);
        if variable.is_empty() {
            continue;
        }
        let mut splitter = Splitter { context: unit.context, function: unit.function, before: gep, created: Vec::new(), cone: Vec::new() };
        let mut moved = BigInt::from(0);
        let mut indices = Vec::new();
        for &(at, scale) in &variable {
            let index = instruction.operands[1 + at];
            let Some(width) = splitter.width(index) else { continue };
            // The `gep` sign-extends an index narrower than the pointer's.
            let exact = (width < pointer_bits).then_some(Flags::NSW);
            let split = splitter.split(index, exact);
            if split.same {
                continue;
            }
            let bytes = if width >= pointer_bits { signed(u128::try_from(Splitter::masked(&split.constant, pointer_bits)).expect("a word"), pointer_bits) } else { signed(u128::try_from(&split.constant).expect("a word"), width) };
            moved += BigInt::from(bytes) * scale;
            indices.push((at, split.rest, width));
        }
        if indices.is_empty() {
            continue;
        }
        let dying = _dying(splitter.function, &splitter.cone, gep);
        let (function, context, layout) = (&*splitter.function, &*splitter.context, unit.layout);
        let price = |list: &mut dyn Iterator<Item = InstId>| list.map(|one| profit::operation(context, layout, function, callees, one, &costs)).sum::<Option<i64>>();
        let made = price(&mut splitter.created.iter().copied());
        let saved = price(&mut dying.iter().copied());
        let created = splitter.created.clone();
        if !matches!((made, saved), (Some(made), Some(saved)) if made <= saved) {
            for one in created.into_iter().rev() {
                unit.function.erase(one).expect("nothing reads a rejected split");
            }
            continue;
        }
        for (at, rest, width) in indices {
            let rest = rest.unwrap_or_else(|| counting::constant(unit.context, &BigInt::from(0), width));
            unit.function.set_operand(gep, 1 + at, rest);
        }
        if moved != BigInt::from(0) {
            let i8 = unit.context.types.int(8);
            let offset = counting::constant(unit.context, &moved, pointer_bits);
            let inner = unit.function.create_instruction(Opcode::GetElementPtr { source: i8 }, instruction.ty, vec![instruction.operands[0], offset], Flags::default(), None);
            unit.function.insert(inner, Position::Before(gep)).expect("a position before the address");
            let base = Operand::Value(unit.function.instruction(inner).result.expect("an address"));
            unit.function.set_operand(gep, 0, base);
        }
        unit.function.set_flags(gep, Flags::default());
        _erase_unread(unit.function, &dying);
        changed = true;
    }
    changed
}

/// The instructions of `cone` nothing but `gep` and each other read.
fn _dying(function: &Function, cone: &[InstId], gep: InstId) -> Vec<InstId> {
    let mut dying = BTreeSet::<InstId>::new();
    let mut grew = true;
    while grew {
        grew = false;
        for &inst in cone {
            let Some(result) = function.instruction(inst).result else { continue };
            if !dying.contains(&inst) && function.users(result).iter().all(|one| one.user == gep || dying.contains(&one.user)) {
                dying.insert(inst);
                grew = true;
            }
        }
    }
    dying.into_iter().collect()
}

/// `dying`, each once nothing reads it.
fn _erase_unread(function: &mut Function, dying: &[InstId]) {
    let mut left = dying.to_vec();
    while !left.is_empty() {
        let before = left.len();
        left.retain(|&inst| {
            let unread = function.instruction(inst).result.is_some_and(|result| function.users(result).is_empty());
            if unread {
                function.erase(inst).expect("an unread instruction");
            }
            !unread
        });
        if left.len() == before {
            break;
        }
    }
}

#[cfg(test)]
#[path = "gepoffset_tests.rs"]
mod tests;
