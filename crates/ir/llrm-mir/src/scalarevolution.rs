//! Integers that step by the same amount every iteration of a loop, as
//! LLVM's ScalarEvolution proves add recurrences: a header phi one latch
//! adds a loop invariant to, and sums, differences and invariant multiples
//! of such. Arithmetic wraps, so each holds modulo its width.

use std::collections::HashMap;

use crate::context::{ConstantKind, Context};
use crate::dominators::DominatorTree;
use crate::loops::{Loop, LoopInfo};
use crate::module::{BlockId, Function, Operand, ValueDef, ValueId};
use crate::opcode::{BinaryOp, IntPredicate, Opcode};

/// A sum of loop-invariant values, each times a constant, and a constant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Linear {
    pub terms: Vec<(ValueId, u128)>,
    pub constant: u128,
}

/// `start + i * step` on iteration `i` of the loop `header` heads.
#[derive(Clone, Debug, PartialEq)]
pub struct Recurrence {
    pub header: BlockId,
    pub start: Linear,
    pub step: Linear,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evolution {
    recurrences: HashMap<ValueId, Recurrence>,
}

impl Evolution {
    pub fn new(context: &Context, function: &Function, loops: &LoopInfo) -> Self {
        let mut solver = Solver { context, function, loops, known: HashMap::new() };
        for (block, inst) in function.walk() {
            if let Some(value) = function.instruction(inst).result
                && loops.loop_of(block).is_some()
            {
                solver.recurrence(value, 0);
            }
        }
        Self { recurrences: solver.known.into_iter().filter_map(|(value, one)| Some((value, one?))).collect() }
    }

    /// `value`'s recurrence in the innermost loop defining it.
    pub fn of(&self, value: ValueId) -> Option<&Recurrence> {
        self.recurrences.get(&value)
    }
}

const DEPTH: u32 = 16;

struct Solver<'a> {
    context: &'a Context,
    function: &'a Function,
    loops: &'a LoopInfo,
    known: HashMap<ValueId, Option<Recurrence>>,
}

impl Solver<'_> {
    fn width(&self, value: ValueId) -> Option<u32> {
        self.context.types.int_bits(self.function.value(value).ty)
    }

    /// `operand` as a sum, if `one` does not define it.
    fn invariant(&self, operand: Operand, one: &Loop, width: u32) -> Option<Linear> {
        match operand {
            Operand::Constant(id) => match self.context.get(id).kind {
                ConstantKind::Int(bits) => Some(Linear { terms: Vec::new(), constant: bits & mask(width) }),
                _ => None,
            },
            Operand::Value(value) => match self.function.value(value).def {
                ValueDef::Instruction(inst) if self.function.parent(inst).is_some_and(|block| one.blocks.contains(&block)) => None,
                _ => Some(Linear { terms: vec![(value, 1)], constant: 0 }),
            },
            Operand::Block(_) => None,
        }
    }

    fn recurrence(&mut self, value: ValueId, depth: u32) -> Option<Recurrence> {
        if let Some(known) = self.known.get(&value) {
            return known.clone();
        }
        // Guards a cycle through phis while this one is being solved.
        self.known.insert(value, None);
        let found = self.solve(value, depth);
        self.known.insert(value, found.clone());
        found
    }

    fn solve(&mut self, value: ValueId, depth: u32) -> Option<Recurrence> {
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        let block = self.function.parent(inst)?;
        let one = self.loops.loop_of(block)?;
        let width = self.width(value)?;
        let instruction = self.function.instruction(inst);
        let operands = instruction.operands.clone();
        // Either side of a binary operation: a recurrence of this loop, or invariant in it.
        let side = |this: &mut Self, operand: Operand| -> Option<(Linear, Linear)> {
            if let Some(start) = this.invariant(operand, one, width) {
                return Some((start, Linear::default()));
            }
            let Operand::Value(value) = operand else { return None };
            if depth == DEPTH {
                return None;
            }
            let found = this.recurrence(value, depth + 1)?;
            (found.header == one.header).then_some((found.start, found.step))
        };
        let (start, step) = match instruction.opcode {
            Opcode::Phi if block == one.header => {
                let [Operand::Value(_) | Operand::Constant(_), Operand::Block(first), Operand::Value(_) | Operand::Constant(_), Operand::Block(second)] = operands[..] else { return None };
                let (entry, next) = match (one.blocks.contains(&first), one.blocks.contains(&second)) {
                    (false, true) => (operands[0], operands[2]),
                    (true, false) => (operands[2], operands[0]),
                    _ => return None,
                };
                let start = self.invariant(entry, one, width)?;
                (start, self.stepped(value, next, one, width)?)
            }
            Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)) => {
                let (a, b) = (side(self, operands[0])?, side(self, operands[1])?);
                match op {
                    BinaryOp::Add => (a.0.plus(&b.0, 1, width), a.1.plus(&b.1, 1, width)),
                    BinaryOp::Sub => (a.0.plus(&b.0, mask(width), width), a.1.plus(&b.1, mask(width), width)),
                    BinaryOp::Shl => {
                        let amount = b.0.as_constant().filter(|_| b.1 == Linear::default())?;
                        let factor = 1u128.checked_shl(u32::try_from(amount).ok().filter(|&one| one < width)?)?;
                        (a.0.times(factor, width), a.1.times(factor, width))
                    }
                    _ => match (a.1 == Linear::default(), b.1 == Linear::default()) {
                        (true, true) => return None,
                        (false, true) => (a.0.product(&b.0, width)?, a.1.product(&b.0, width)?),
                        (true, false) => (b.0.product(&a.0, width)?, b.1.product(&a.0, width)?),
                        (false, false) => return None,
                    },
                }
            }
            _ => return None,
        };
        // A value that does not change is invariant, no recurrence.
        (step != Linear::default()).then_some(Recurrence { header: one.header, start, step })
    }

    /// What `next`, the value a latch hands `phi`, adds to it.
    fn stepped(&self, phi: ValueId, next: Operand, one: &Loop, width: u32) -> Option<Linear> {
        let Operand::Value(next) = next else { return None };
        let ValueDef::Instruction(inst) = self.function.value(next).def else { return None };
        let instruction = self.function.instruction(inst);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        match (op, &instruction.operands[..]) {
            (BinaryOp::Add, &[Operand::Value(a), b]) if a == phi => self.invariant(b, one, width),
            (BinaryOp::Add, &[b, Operand::Value(a)]) if a == phi => self.invariant(b, one, width),
            (BinaryOp::Sub, &[Operand::Value(a), b]) if a == phi => Some(self.invariant(b, one, width)?.times(mask(width), width)),
            _ => None,
        }
    }
}

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1 << width) - 1 }
}

impl Linear {
    fn as_constant(&self) -> Option<u128> {
        self.terms.is_empty().then_some(self.constant)
    }

    /// `self + other * factor`.
    fn plus(&self, other: &Linear, factor: u128, width: u32) -> Linear {
        let mut out = self.clone();
        for &(value, coefficient) in &other.terms {
            match out.terms.iter_mut().find(|(one, _)| *one == value) {
                Some((_, sum)) => *sum = sum.wrapping_add(coefficient.wrapping_mul(factor)) & mask(width),
                None => out.terms.push((value, coefficient.wrapping_mul(factor) & mask(width))),
            }
        }
        out.terms.retain(|&(_, coefficient)| coefficient != 0);
        out.constant = out.constant.wrapping_add(other.constant.wrapping_mul(factor)) & mask(width);
        out
    }

    fn times(&self, factor: u128, width: u32) -> Linear {
        Linear::default().plus(self, factor, width)
    }

    pub fn scaled(&self, factor: u128, width: u32) -> Linear {
        self.times(factor, width)
    }

    /// `self + constant`.
    pub fn shifted(&self, constant: u128, width: u32) -> Linear {
        Linear { terms: self.terms.clone(), constant: self.constant.wrapping_add(constant) & mask(width) }
    }

    /// `self * other`, when one side is a constant.
    fn product(&self, other: &Linear, width: u32) -> Option<Linear> {
        match (self.as_constant(), other.as_constant()) {
            (_, Some(factor)) => Some(self.times(factor, width)),
            (Some(factor), _) => Some(other.times(factor, width)),
            _ => None,
        }
    }
}

/// A loop's exit a counter must reach: the block testing it on every
/// iteration's way round, the successor it stays in, the counter, its step
/// of one either way, the invariant bound, and the predicate under which
/// the loop stays.
#[derive(Clone, Debug, PartialEq)]
pub struct Counted {
    pub block: BlockId,
    pub inside: BlockId,
    pub counter: ValueId,
    pub step: i8,
    pub bound: Operand,
    pub stays: IntPredicate,
}

pub fn counted(context: &Context, function: &Function, tree: &DominatorTree, evolution: &Evolution, one: &Loop) -> Option<Counted> {
    one.blocks.iter().find_map(|&block| {
        if !one.latches.iter().all(|&latch| tree.dominates(block, latch)) {
            return None;
        }
        let branch = function.instruction(function.terminator(block)?);
        let [Operand::Value(condition), Operand::Block(taken), Operand::Block(otherwise)] = branch.operands[..] else { return None };
        let (stays_on_true, inside) = match (one.blocks.contains(&taken), one.blocks.contains(&otherwise)) {
            (true, false) => (true, taken),
            (false, true) => (false, otherwise),
            _ => return None,
        };
        let ValueDef::Instruction(compare) = function.value(condition).def else { return None };
        let compare = function.instruction(compare);
        let Opcode::ICmp(predicate) = compare.opcode else { return None };
        let (left, right) = (compare.operands[0], compare.operands[1]);
        let stepped = |operand: Operand| -> Option<(ValueId, i8)> {
            let Operand::Value(value) = operand else { return None };
            let found = evolution.of(value).filter(|found| found.header == one.header)?;
            let width = context.types.int_bits(function.value(value).ty)?;
            match (found.step.terms.is_empty(), found.step.constant) {
                (true, 1) => Some((value, 1)),
                (true, step) if step == mask(width) => Some((value, -1)),
                _ => None,
            }
        };
        let invariant = |operand: Operand| match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|block| !one.blocks.contains(&block)),
                ValueDef::Argument(_) => true,
            },
            _ => true,
        };
        let ((counter, step), bound, predicate) = match (stepped(left), stepped(right)) {
            (Some(found), None) if invariant(right) => (found, right, predicate),
            (None, Some(found)) if invariant(left) => (found, left, predicate.swapped()),
            _ => return None,
        };
        let stays = if stays_on_true { predicate } else { predicate.inverse() };
        let finite = matches!(
            (step, stays),
            (1, IntPredicate::Slt | IntPredicate::Ult | IntPredicate::Ne) | (-1, IntPredicate::Sgt | IntPredicate::Ugt | IntPredicate::Ne)
        );
        finite.then_some(Counted { block, inside, counter, step, bound, stays })
    })
}
