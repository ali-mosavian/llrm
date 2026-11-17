// SPDX-License-Identifier: GPL-3.0-or-later
//! Known bits of a value along a path, for [`crate::backwardthreading`].
//!
//! GCC's backward threader asks ranger for the value of a compare's inputs
//! on a path (`gimple-range-path.cc`); an `irange` carries a bitmask
//! (`irange_bitmask`) that the range operators of `+`, `*`, `&` and `>>`
//! propagate, so `x & 1` is known to be 0 after `x = 3 * y + 1` where
//! `y & 1` was 1. Here the path is the join `p` and the block `bb` whose
//! branch is decided; `bb`'s phis read the value `p` sends along the edge,
//! and a phi of `p` reads the one `q` sends. The assumption is the compare
//! (or select condition) that sent the path to `q`.
//!
//! DEVIATE (no LLVM pass; GCC `gimple-range-path.cc`): the bits come from
//! [`KnownBits`] of the instructions of `p`, `bb` and `q` only, evaluated
//! again on the path, with the facts the one assumption gives to the values
//! it compares (`(v & mask) ==/!= c`, `v ==/!= c`). Nothing outside the path
//! is read beyond those facts.

use llrm_analysis::occurrence::arms;
use llrm_mir::context::Context;
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};

use crate::backwardthreading::{constant_of, width_of};

/// The deepest chain of instructions evaluated.
const DEPTH: u32 = 8;

pub struct PathBits<'a> {
    context: &'a Context,
    function: &'a Function,
    /// The join the path enters `bb` from.
    p: BlockId,
    bb: BlockId,
    /// The predecessor of `p` the path comes through; none where the select
    /// that decides it is still in `p`.
    q: Option<BlockId>,
    /// The compare that holds on the path, and whether it is true.
    assumed: Option<(Operand, bool)>,
    /// What the assumption says of the values it compares.
    refined: Vec<(ValueId, KnownBits)>,
}

impl<'a> PathBits<'a> {
    pub fn new(
        context: &'a Context,
        function: &'a Function,
        p: BlockId,
        bb: BlockId,
        q: Option<BlockId>,
        assumed: Option<(Operand, bool)>,
    ) -> Self {
        let mut bits = Self { context, function, p, bb, q, assumed, refined: Vec::new() };
        if let Some((cond, holds)) = assumed {
            bits.refined = bits.facts(cond, holds);
        }
        bits
    }

    /// What `cond` being `holds` says of the values it compares.
    fn facts(
        &self,
        cond: Operand,
        holds: bool,
    ) -> Vec<(ValueId, KnownBits)> {
        let Operand::Value(id) = cond else { return Vec::new() };
        let ValueDef::Instruction(icmp) = self.function.value(id).def else { return Vec::new() };
        let Opcode::ICmp(pred) = self.function.instruction(icmp).opcode else { return Vec::new() };
        let equal = match (pred, holds) {
            (IntPredicate::Eq, true) | (IntPredicate::Ne, false) => true,
            (IntPredicate::Eq, false) | (IntPredicate::Ne, true) => false,
            _ => return Vec::new(),
        };
        let ops = &self.function.instruction(icmp).operands;
        let (Some(width), Some(constant)) = (width_of(self.context, self.function, ops[0]), constant_of(self.context, ops[1]))
        else {
            return Vec::new();
        };
        let all = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        let mut out = Vec::new();
        // `v == c`, or `v != c` for a one-bit compare: the value itself.
        let Operand::Value(masked) = ops[0] else { return out };
        if equal || width == 1 {
            let known = if equal { constant } else { !constant & all };
            out.push((masked, KnownBits::constant(width, known)));
        }
        // `(v & mask) == c`: the masked bits of `v` are those of `c`; and
        // `(v & bit) != 0` with one bit in the mask: that bit is set.
        let ValueDef::Instruction(and) = self.function.value(masked).def else { return out };
        if self.function.instruction(and).opcode != Opcode::Binary(BinaryOp::And) {
            return out;
        }
        let and_ops = &self.function.instruction(and).operands;
        let (Operand::Value(value), Some(mask)) = (and_ops[0], constant_of(self.context, and_ops[1])) else {
            return out;
        };
        if equal {
            if constant & !mask == 0 {
                out.push((value, KnownBits::from_masks(width, !constant & mask, constant & mask)));
            }
        } else if constant == 0 && mask.count_ones() == 1 {
            out.push((value, KnownBits::from_masks(width, 0, mask)));
        } else if constant == mask && mask.count_ones() == 1 {
            out.push((value, KnownBits::from_masks(width, mask, 0)));
        }
        out
    }

    /// The arm of a compare `pred lhs, rhs` of `bb` (read as it stands in
    /// `bb`) that the bits along the path decide.
    pub fn decides(
        &self,
        pred: IntPredicate,
        lhs: Operand,
        rhs: Operand,
    ) -> Option<bool> {
        let lhs = self.eval(lhs, true, DEPTH)?;
        let rhs = self.eval(rhs, true, DEPTH)?;
        match pred {
            IntPredicate::Eq => KnownBits::eq(&lhs, &rhs),
            IntPredicate::Ne => KnownBits::ne(&lhs, &rhs),
            _ => None,
        }
    }

    fn on_path(
        &self,
        block: BlockId,
    ) -> bool {
        block == self.bb || block == self.p || Some(block) == self.q
    }

    /// The bits of `value`; `next` is the trip through `bb` that reads it
    /// (the one the compare is in), not the trip `p` closes.
    fn eval(
        &self,
        value: Operand,
        next: bool,
        depth: u32,
    ) -> Option<KnownBits> {
        let width = width_of(self.context, self.function, value)?;
        if let Some(constant) = constant_of(self.context, value) {
            return Some(KnownBits::constant(width, constant));
        }
        let unknown = KnownBits::unknown(width);
        let Operand::Value(id) = value else { return Some(unknown) };
        let ValueDef::Instruction(inst) = self.function.value(id).def else { return Some(unknown) };
        let mut known = unknown.clone();
        if !next && let Some((_, bits)) = self.refined.iter().find(|(one, _)| *one == id) {
            known = bits.clone();
        }
        let block = self.function.parent(inst)?;
        if depth == 0 || !self.on_path(block) {
            return Some(known);
        }
        let instruction = self.function.instruction(inst);
        let operand = |at: usize, next: bool| self.eval(instruction.operands[at], next, depth - 1);
        let computed = match &instruction.opcode {
            Opcode::Phi => {
                let from = if next && block == self.bb {
                    Some(self.p)
                } else if !next && block == self.p {
                    self.q
                } else {
                    None
                };
                let incoming = from.and_then(|from| arms(self.function, inst).into_iter().find(|&(_, pred)| pred == from));
                match incoming {
                    Some((value, _)) => self.eval(value, false, depth - 1)?,
                    None => unknown,
                }
            }
            Opcode::Select => {
                let (yes, no) = (operand(1, next)?, operand(2, next)?);
                match self.assumed {
                    Some((cond, holds)) if cond == instruction.operands[0] && !next => {
                        if holds { yes } else { no }
                    }
                    _ => yes.intersect_with(&no),
                }
            }
            Opcode::Freeze => operand(0, next)?,
            Opcode::Cast(CastOp::ZExt) => operand(0, next)?.zext(width),
            Opcode::Cast(CastOp::Trunc) => operand(0, next)?.trunc(width),
            Opcode::Binary(op) => {
                let (lhs, rhs) = (operand(0, next)?, operand(1, next)?);
                let nuw = instruction.flags.contains(Flags::NUW);
                let nsw = instruction.flags.contains(Flags::NSW);
                let bits = |zero: u128, one: u128| KnownBits::from_masks(width, zero, one);
                match op {
                    BinaryOp::And => bits(lhs.zero | rhs.zero, lhs.one & rhs.one),
                    BinaryOp::Or => bits(lhs.zero & rhs.zero, lhs.one | rhs.one),
                    BinaryOp::Xor => bits((lhs.zero & rhs.zero) | (lhs.one & rhs.one), (lhs.zero & rhs.one) | (lhs.one & rhs.zero)),
                    BinaryOp::Add => KnownBits::add(&lhs, &rhs, nsw, nuw, false),
                    BinaryOp::Sub => KnownBits::sub(&lhs, &rhs, nsw, nuw),
                    BinaryOp::Mul => KnownBits::mul(&lhs, &rhs, false),
                    BinaryOp::Shl => KnownBits::shl(&lhs, &rhs, nuw, nsw, false),
                    BinaryOp::LShr => KnownBits::lshr(&lhs, &rhs, false, instruction.flags.contains(Flags::EXACT)),
                    _ => unknown,
                }
            }
            _ => unknown,
        };
        Some(known.union_with(&computed))
    }
}

/// The instruction `inst`'s block, when it is a select of `function`.
pub fn select_parts(
    function: &Function,
    inst: InstId,
) -> Option<(Operand, Operand, Operand)> {
    let instruction = function.instruction(inst);
    (instruction.opcode == Opcode::Select).then(|| (instruction.operands[0], instruction.operands[1], instruction.operands[2]))
}
