//! i64 as two dwords, low first, as LLVM's type legalizer expands an
//! integer no register holds. What a value's sign bits prove lets a product
//! or quotient of 32-bit values take one 32-bit instruction.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::sign_bits;
use llrm_mir::{BinaryOp, CastOp, IntPredicate};

use super::expand::{Half, expand_wide};
use super::{Selector, Test, Unselected, condition_code, insn, refuse, semantics, swapped};
use crate::backend::callregs::{call_clobbered_high, call_clobbers};
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::{Insn, LirBlock};

type Pair = (Held, Held);

impl Selector<'_, '_, '_> {
    pub(super) fn is_wide(
        &self,
        ty: llrm_mir::TypeId,
    ) -> bool {
        self.types().int_bits(ty) == Some(64)
    }

    /// Whether `operand` is an i32 sign-extended.
    fn narrow(
        &self,
        operand: Operand,
    ) -> bool {
        sign_bits(&self.module.context, self.function, operand) > 32
    }

    fn put(
        &mut self,
        what: crate::model::ir::Semantics,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) {
        out.push(insn(at, what));
    }

    fn dword(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn count(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 1, address: None })
    }

    /// A register for one half of a wide value, dropped where nothing reads it.
    pub(super) fn half(&mut self) -> Held {
        let half = self.fresh_held(4);
        self.halves.insert(half.value);
        half
    }

    /// `into` made `from`.
    fn made(
        &mut self,
        operation: Operation,
        name: &str,
        sources: Vec<Loc>,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Held {
        let into = self.half();
        self.put(semantics(operation, name, vec![Loc::Held(into)], sources), at, out);
        into
    }

    /// An i64 operand's halves, each in a register.
    pub(super) fn wide(
        &mut self,
        operand: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<Pair, Unselected> {
        if let Some(bits) = self.constant(operand, 8) {
            let low = self.made(Operation::Move, "mov", vec![Self::dword(bits as u32 as i64)], at, out);
            let high = self.made(Operation::Move, "mov", vec![Self::dword((bits >> 32) as u32 as i64)], at, out);
            return Ok((low, high));
        }
        match operand {
            Operand::Value(value) => {
                self.wides.get(&value).copied().ok_or(()).or_else(|()| refuse("an i64 from no expanded instruction"))
            }
            _ => refuse("an i64 constant of no bits"),
        }
    }

    /// An i64's two halves, low first, at the target's native width.
    pub(super) fn wide_halves(&self) -> Result<[Half; 2], Unselected> {
        self.wide_halves_of(false)
    }

    /// As `wide_halves`, the upper taking the carry of the lower where
    /// `chained`.
    fn wide_halves_of(
        &self,
        chained: bool,
    ) -> Result<[Half; 2], Unselected> {
        match expand_wide(chained, 64, self.layout.largest_legal_integer())[..] {
            [low, high] => Ok([low, high]),
            _ => refuse("an i64 on a target whose native integer is not half its width"),
        }
    }

    /// A binary operation on an i64 as its halves (`expand_wide`): `first` the
    /// form of the lowest, `rest` of each above, which takes its
    /// predecessor's carry where it is not the same form. A constant operand is
    /// an immediate in each half, as the form takes one.
    pub(super) fn wide_chain(
        &mut self,
        m: &super::matcher::Match,
        out: &mut Vec<Arc<Insn>>,
        first: &str,
        rest: &str,
        left: Operand,
        right: Operand,
    ) -> Result<(), Unselected> {
        let at = m.at;
        let halves = self.wide_halves_of(first != rest)?;
        let a = self.wide(left, at, out)?;
        let b = match self.constant(right, 8) {
            Some(bits) => [Self::dword(bits as u32 as i64), Self::dword((bits >> 32) as u32 as i64)],
            None => {
                let pair = self.wide(right, at, out)?;
                [Loc::Held(pair.0), Loc::Held(pair.1)]
            }
        };
        // The carry runs from the low half's instruction to the high's.
        let mut made = Vec::new();
        for half in &halves {
            let name = if half.carried { rest } else { first };
            let from = if half.index == 0 { a.0 } else { a.1 };
            made.push(self.made(Operation::Binary, name, vec![Loc::Held(from), b[half.index].clone()], at, out));
        }
        let result = self.function.instruction(m.inst).result.expect("a result");
        self.wides.insert(result, (made[0], made[1]));
        Ok(())
    }

    /// A cast to or from i64.
    pub(super) fn wide_cast(
        &mut self,
        op: CastOp,
        inst: InstId,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (operand, to) = (instruction.operands[0], instruction.ty);
        let from = self.function.operand_type(&self.module.context, operand).expect("a typed operand");
        let result = instruction.result.expect("a cast's value");
        // fild reads a qword: the pair stored, low dword first.
        if op == CastOp::SIToFP && self.is_float(to) {
            let (low, high) = self.wide(operand, at, out)?;
            let cell = self.temporary(8);
            for (half, by) in [(low, 0), (high, 4)] {
                self.put(
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![Loc::Mem(Self::memory(cell.moved(by), 4))],
                        vec![Loc::Held(half)],
                    ),
                    at,
                    out,
                );
            }
            self.integer_made_float(8, result, to, cell, 8, at, out)?;
            return Ok(());
        }
        // fistp stores a qword, truncating as the control word is set: read
        // back as the halves.
        if op == CastOp::FPToSI && self.is_float(from) {
            let held = self.float(operand, at, out)?;
            let cell = self.float_stored(held, "fisttp", 8, at, out);
            let (low, high) = (self.half(), self.half());
            for (half, by) in [(low, 0), (high, 4)] {
                self.put(
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![Loc::Held(half)],
                        vec![Loc::Mem(Self::memory(cell.moved(by), 4))],
                    ),
                    at,
                    out,
                );
            }
            self.wides.insert(result, (low, high));
            return Ok(());
        }
        // fild reads a signed qword, so an unsigned one is its two halves each
        // read as a qword of its own, the high one scaled by 2^32 (a
        // qword of (0, 1) read the same way), and added: every step exact in
        // the x87's 64-bit mantissa, one rounding when the sum is
        // stored as a float or a double.
        if op == CastOp::UIToFP && self.is_float(to) {
            let (low, high) = self.wide(operand, at, out)?;
            let loaded = |this: &mut Self, below: Loc, above: Loc, out: &mut Vec<Arc<Insn>>| -> Held {
                let cell = this.temporary(8);
                for (part, by) in [(below, 0), (above, 4)] {
                    this.put(
                        semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(cell.moved(by), 4))], vec![part]),
                        at,
                        out,
                    );
                }
                let value = this.fresh_held(super::FLOAT);
                this.float_loaded(value, "fild", cell, 8, false, at, out);
                value
            };
            let (zero, one) = (Self::dword(0), Self::dword(1));
            let (lower, upper) =
                (loaded(self, Loc::Held(low), zero.clone(), out), loaded(self, Loc::Held(high), zero.clone(), out));
            let scale = loaded(self, zero, one, out);
            let scaled = self.fresh_held(super::FLOAT);
            self.put(
                semantics(
                    Operation::FloatArith,
                    "fmul",
                    vec![Loc::Held(scaled)],
                    vec![Loc::Held(upper), Loc::Held(scale)],
                ),
                at,
                out,
            );
            let into = Held { value: self.value(result), width: super::FLOAT };
            self.put(
                semantics(
                    Operation::FloatArith,
                    "fadd",
                    vec![Loc::Held(into)],
                    vec![Loc::Held(lower), Loc::Held(scaled)],
                ),
                at,
                out,
            );
            return Ok(());
        }
        if !self.is_wide(to) {
            let (low, _) = self.wide(operand, at, out)?;
            let into = Held { value: self.value(result), width: self.width(to)? };
            let what = if self.types().int_bits(to) == Some(1) {
                semantics(
                    Operation::Binary,
                    "and",
                    vec![Loc::Held(into)],
                    vec![Loc::Held(Held { width: 1, ..low }), Self::count(1)],
                )
            } else {
                match op {
                    CastOp::Trunc => semantics(
                        Operation::Move,
                        "mov",
                        vec![Loc::Held(into)],
                        vec![Loc::Held(Held { width: into.width, ..low })],
                    ),
                    _ => return refuse(format!("{op:?} from an i64")),
                }
            };
            self.put(what, at, out);
            return Ok(());
        }
        let width = self.width(from)?;
        let source = self.held(operand, from, at, out)?;
        let low = match (op, width) {
            (_, 4) => source,
            // An i1 is a byte holding 0 or 1; made signed it is 0 or -1.
            (CastOp::SExt, _) if self.types().int_bits(from) == Some(1) => {
                let one = self.made(Operation::Extend, "movzx", vec![Loc::Held(source)], at, out);
                self.made(Operation::Unary, "neg", vec![Loc::Held(one)], at, out)
            }
            (CastOp::SExt, _) => self.made(Operation::Extend, "movsx", vec![Loc::Held(source)], at, out),
            (CastOp::ZExt, _) => self.made(Operation::Extend, "movzx", vec![Loc::Held(source)], at, out),
            _ => return refuse(format!("{op:?} to an i64")),
        };
        let high = match op {
            CastOp::SExt => self.made(Operation::Extend, "cdq", vec![Loc::Held(low)], at, out),
            CastOp::ZExt => self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out),
            _ => return refuse(format!("{op:?} to an i64")),
        };
        self.wides.insert(result, (low, high));
        Ok(())
    }

    /// An i64 binary operation, on the halves.
    pub(super) fn wide_binary(
        &mut self,
        op: BinaryOp,
        inst: InstId,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (left, right) = (instruction.operands[0], instruction.operands[1]);
        let result = instruction.result.expect("a result");
        let pair = match op {
            BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => {
                let (low, high) = self.wide(left, at, out)?;
                match self.constant_through_casts(right, 0).map(|count| count & 63) {
                    Some(count) => self.shifted(op, low, high, count, at, out),
                    None => self.shifted_by_variable(op, right, low, high, at, out)?,
                }
            }
            BinaryOp::Mul if self.narrow(left) && self.narrow(right) => {
                // Both are i32s: one signed widening multiply.
                let (a, b) = (self.wide(left, at, out)?.0, self.wide(right, at, out)?.0);
                let (low, high) = (self.half(), self.half());
                self.put(
                    semantics(
                        Operation::Multiply,
                        "imul",
                        vec![Loc::Held(low), Loc::Held(high)],
                        vec![Loc::Held(a), Loc::Held(b)],
                    ),
                    at,
                    out,
                );
                (low, high)
            }
            BinaryOp::Mul => {
                // low*low widened, and each half by the other's low into the
                // high.
                let (a, b) = (self.wide(left, at, out)?, self.wide(right, at, out)?);
                let (low, carried) = (self.half(), self.half());
                self.put(
                    semantics(
                        Operation::Multiply,
                        "mul",
                        vec![Loc::Held(low), Loc::Held(carried)],
                        vec![Loc::Held(a.0), Loc::Held(b.0)],
                    ),
                    at,
                    out,
                );
                let first = self.made(Operation::Multiply, "imul", vec![Loc::Held(a.0), Loc::Held(b.1)], at, out);
                let second = self.made(Operation::Multiply, "imul", vec![Loc::Held(a.1), Loc::Held(b.0)], at, out);
                let crossed = self.made(Operation::Binary, "add", vec![Loc::Held(first), Loc::Held(second)], at, out);
                (low, self.made(Operation::Binary, "add", vec![Loc::Held(carried), Loc::Held(crossed)], at, out))
            }
            BinaryOp::SDiv | BinaryOp::SRem | BinaryOp::UDiv | BinaryOp::URem => {
                let partner = self.paired.get(&inst).copied();
                let (mine, other) = self.wide_division(op, left, right, partner.is_some(), at, out)?;
                if let (Some(partner), Some(other)) = (partner, other) {
                    self.wides.insert(partner, other);
                }
                mine
            }
            _ => return refuse(format!("an i64 {}", instruction.opcode.mnemonic())),
        };
        self.wides.insert(result, pair);
        Ok(())
    }

    /// `blocks` without what makes a wide half nothing reads, as LLVM's
    /// DAG drops the dead half its legalizer split off, or a product an
    /// address's scale took. A carry an `adc` or `sbb` reads is made just
    /// before it and goes with it.
    pub(super) fn unread_halves_dropped(
        &self,
        mut blocks: Vec<LirBlock>,
    ) -> Vec<LirBlock> {
        loop {
            let read: BTreeSet<u32> = blocks
                .iter()
                .flat_map(|block| {
                    block
                        .insns
                        .iter()
                        .flat_map(|one| one.uses.iter().copied())
                        .chain(block.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)))
                })
                .collect();
            let dead = |one: &Insn| {
                !one.defines.is_empty()
                    && one.defines.iter().all(|value| {
                        (self.halves.contains(value) || self.folded.contains(value)) && !read.contains(value)
                    })
            };
            let carries =
                |one: &Insn| matches!(
                    one.what.as_ref().and_then(|what| what.name.as_deref()),
                    Some("adc" | "sbb")
                );
            let mut changed = false;
            for block in &mut blocks {
                let insns = &block.insns;
                let kept: Vec<Arc<Insn>> = insns
                    .iter()
                    .enumerate()
                    .filter(|&(at, one)| {
                        !dead(one) || insns.get(at + 1).is_some_and(|next| carries(next) && !dead(next))
                    })
                    .map(|(_, one)| Arc::clone(one))
                    .collect();
                if kept.len() != insns.len() {
                    changed = true;
                    *block = block.with_insns(kept);
                }
            }
            if !changed {
                return blocks;
            }
        }
    }

    /// An integer operand's value where it is a constant, or a cast of one: at
    /// -O0 a shift's count is `zext i32 30 to i64`.
    fn constant_through_casts(
        &self,
        operand: Operand,
        depth: u32,
    ) -> Option<i64> {
        if let Some(value) = self.constant(operand, 8) {
            return Some(value);
        }
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        let instruction = self.function.instruction(inst);
        let Opcode::Cast(cast) = instruction.opcode else { return None };
        if depth > 4 {
            return None;
        }
        let inner = self.constant_through_casts(instruction.operands[0], depth + 1)?;
        let from = self
            .function
            .operand_type(&self.module.context, instruction.operands[0])
            .and_then(|ty| self.types().int_bits(ty))?;
        let to = self.types().int_bits(instruction.ty)?;
        let narrow = |value: i64, bits: u32| if bits >= 64 { value } else { value & ((1_i64 << bits) - 1) };
        let signed = |value: i64, bits: u32| if bits >= 64 { value } else { (value << (64 - bits)) >> (64 - bits) };
        Some(match cast {
            CastOp::ZExt => narrow(inner, from),
            CastOp::SExt => signed(inner, from),
            CastOp::Trunc => narrow(inner, to),
            _ => return None,
        })
    }

    /// A shift by a count the program computes: the shift of each half by the
    /// count's low five bits (the machine's own reading of cl), and its
    /// sixth bit, spread over a register as a mask, choosing between the halves
    /// and the fill, for the counts 32 to 63 (LLVM's
    /// ExpandShiftWithUnknownAmountBit, without a select the 486 has none of).
    fn shifted_by_variable(
        &mut self,
        op: BinaryOp,
        right: Operand,
        low: Held,
        high: Held,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<Pair, Unselected> {
        let count = self.wide(right, at, out)?.0;
        let cl = Loc::Held(Held { width: 1, ..count });
        let by = |count: i64| Self::count(count);
        let all = |this: &mut Self, name: &str, a: Held, b: Held, out: &mut Vec<Arc<Insn>>| {
            this.made(Operation::Binary, name, vec![Loc::Held(a), Loc::Held(b)], at, out)
        };
        // Bit five of the count as a register of ones or zeros.
        let moved = self.made(Operation::Binary, "shl", vec![Loc::Held(count), by(26)], at, out);
        let mask = self.made(Operation::Binary, "sar", vec![Loc::Held(moved), by(31)], at, out);
        let keep = self.made(Operation::Binary, "xor", vec![Loc::Held(mask), Self::dword(-1)], at, out);
        let (low_part, high_part) = match op {
            BinaryOp::Shl => {
                let lower = self.made(Operation::Binary, "shl", vec![Loc::Held(low), cl.clone()], at, out);
                let upper = self.made(Operation::Funnel, "shld", vec![Loc::Held(high), Loc::Held(low), cl], at, out);
                // From 32: the high half is the low half shifted, the low half
                // is zero.
                let (kept_upper, from_lower) = (all(self, "and", upper, keep, out), all(self, "and", lower, mask, out));
                let new_high = all(self, "or", kept_upper, from_lower, out);
                (all(self, "and", lower, keep, out), new_high)
            }
            _ => {
                let name = if op == BinaryOp::AShr { "sar" } else { "shr" };
                let upper = self.made(Operation::Binary, name, vec![Loc::Held(high), cl.clone()], at, out);
                let lower = self.made(Operation::Funnel, "shrd", vec![Loc::Held(low), Loc::Held(high), cl], at, out);
                // From 32: the low half is the high half shifted, the high half
                // is the fill.
                let (kept_lower, from_upper) = (all(self, "and", lower, keep, out), all(self, "and", upper, mask, out));
                let new_low = all(self, "or", kept_lower, from_upper, out);
                let new_high = if op == BinaryOp::AShr {
                    let fill = self.made(Operation::Binary, "sar", vec![Loc::Held(high), by(31)], at, out);
                    let (kept_upper, filled) = (all(self, "and", upper, keep, out), all(self, "and", fill, mask, out));
                    all(self, "or", kept_upper, filled, out)
                } else {
                    all(self, "and", upper, keep, out)
                };
                (new_low, new_high)
            }
        };
        Ok((low_part, high_part))
    }

    fn shifted(
        &mut self,
        op: BinaryOp,
        low: Held,
        high: Held,
        count: i64,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Pair {
        let by = |count: i64| Self::count(count);
        match (op, count) {
            (_, 0) => (low, high),
            (BinaryOp::Shl, 1..32) => {
                let upper =
                    self.made(Operation::Funnel, "shld", vec![Loc::Held(high), Loc::Held(low), by(count)], at, out);
                (self.made(Operation::Binary, "shl", vec![Loc::Held(low), by(count)], at, out), upper)
            }
            (BinaryOp::Shl, _) => {
                let upper = self.made(Operation::Binary, "shl", vec![Loc::Held(low), by(count - 32)], at, out);
                (self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out), upper)
            }
            (_, 1..32) => {
                let lower =
                    self.made(Operation::Funnel, "shrd", vec![Loc::Held(low), Loc::Held(high), by(count)], at, out);
                let name = if op == BinaryOp::AShr { "sar" } else { "shr" };
                (lower, self.made(Operation::Binary, name, vec![Loc::Held(high), by(count)], at, out))
            }
            (BinaryOp::AShr, _) => {
                let lower = self.made(Operation::Binary, "sar", vec![Loc::Held(high), by(count - 32)], at, out);
                (lower, self.made(Operation::Binary, "sar", vec![Loc::Held(high), by(31)], at, out))
            }
            _ => {
                let lower = self.made(Operation::Binary, "shr", vec![Loc::Held(high), by(count - 32)], at, out);
                (lower, self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out))
            }
        }
    }

    /// An i64 comparison as flags: equality by the halves' differences
    /// or-ed, an order by `sub` and `sbb` of the halves, whose flags a
    /// less-than or at-least reads; a greater-than compares the other way.
    pub(super) fn wide_compare(
        &mut self,
        predicate: IntPredicate,
        a: Operand,
        b: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<Test, Unselected> {
        let predicate = match predicate {
            IntPredicate::Sgt | IntPredicate::Sle | IntPredicate::Ugt | IntPredicate::Ule => {
                return self.wide_compare(swapped(predicate), b, a, at, out);
            }
            predicate => predicate,
        };
        let a = self.wide(a, at, out)?;
        // A constant's halves are immediates.
        let b = match self.constant(b, 8) {
            Some(bits) => (Self::dword(bits as u32 as i64), Self::dword((bits >> 32) as u32 as i64)),
            None => {
                let (low, high) = self.wide(b, at, out)?;
                (Loc::Held(low), Loc::Held(high))
            }
        };
        let flags = |selector: &mut Self, name: &str, sources: [Loc; 2], out: &mut Vec<Arc<Insn>>| {
            let into = selector.fresh_held(4);
            selector.put(semantics(Operation::Binary, name, vec![Loc::Held(into)], sources.to_vec()), at, out);
            into
        };
        match predicate {
            IntPredicate::Eq | IntPredicate::Ne => {
                let low = flags(self, "xor", [Loc::Held(a.0), b.0], out);
                let high = flags(self, "xor", [Loc::Held(a.1), b.1], out);
                flags(self, "or", [Loc::Held(low), Loc::Held(high)], out);
            }
            _ => {
                flags(self, "sub", [Loc::Held(a.0), b.0], out);
                flags(self, "sbb", [Loc::Held(a.1), b.1], out);
            }
        }
        Ok(Test::One(condition_code(predicate)))
    }

    /// An i64 quotient or remainder, and with `both` the other: a signed
    /// division by a power of two shifts, by an i32 two dword divisions,
    /// and any other the helper, which makes both at once.
    fn wide_division(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        both: bool,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(Pair, Option<Pair>), Unselected> {
        let signed = matches!(op, BinaryOp::SDiv | BinaryOp::SRem);
        let quotient = matches!(op, BinaryOp::SDiv | BinaryOp::UDiv);
        let power = self
            .constant(right, 8)
            .filter(|&bits| bits > 1 && (bits as u64).is_power_of_two())
            .map(|bits| (bits as u64).trailing_zeros() as i64);
        if let Some(shift) = power.filter(|_| signed) {
            let dividend = self.wide(left, at, out)?;
            let mine = self.divided_by_power(quotient, dividend, shift, at, out);
            return Ok((mine, both.then(|| self.divided_by_power(!quotient, dividend, shift, at, out))));
        }
        // Under 2^31 in magnitude, the dividend over any nonzero dword
        // leaves a quotient that fits one: idiv cannot overflow.
        if signed && self.narrow(right) && sign_bits(&self.module.context, self.function, left) > 33 {
            let ((low, high), divisor) = (self.wide(left, at, out)?, self.wide(right, at, out)?.0);
            let (quotients, remainders) = (self.half(), self.half());
            let what = semantics(
                Operation::Divide,
                "idiv",
                vec![Loc::Held(quotients), Loc::Held(remainders)],
                vec![Loc::Held(high), Loc::Held(low), Loc::Held(divisor)],
            );
            self.put(what, at, out);
            let mut widened = |held: Held, selector: &mut Self| {
                (held, selector.made(Operation::Binary, "sar", vec![Loc::Held(held), Self::count(31)], at, out))
            };
            let (quotients, remainders) = (widened(quotients, self), widened(remainders, self));
            return Ok(if quotient {
                (quotients, both.then_some(remainders))
            } else {
                (remainders, both.then_some(quotients))
            });
        }
        if signed && self.narrow(right) {
            let (dividend, divisor) = (self.wide(left, at, out)?, self.wide(right, at, out)?.0);
            let mine = self.divided(quotient, dividend, divisor, at, out);
            return Ok((mine, both.then(|| self.divided(!quotient, dividend, divisor, at, out))));
        }
        let (quotients, remainders) = self.divided_by_helper(signed, left, right, at, out)?;
        Ok(if quotient { (quotients, Some(remainders)) } else { (remainders, Some(quotients)) })
    }

    /// An i64 divided by a variable, by the old route's inline 386 helpers
    /// (`lower_int64`): edx:eax by ecx:ebx, the quotient left in edx:eax and
    /// the remainder in ecx:ebx; a divisor whose high dword is 0 takes the
    /// helper for a dword, which reads no ecx.
    fn divided_by_helper(
        &mut self,
        signed: bool,
        left: Operand,
        right: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(Pair, Pair), Unselected> {
        use crate::abi::runtime::Reg;
        use crate::backend::lower_int64::{
            _SDIV, _SDIV_CONST32, _UDIV, _UDIV_CONST32, _four_clobbers, _four_inputs, _helper,
        };
        let dividend = self.wide(left, at, out)?;
        let dword = self.constant(right, 8).is_some_and(|bits| bits >> 32 == 0);
        let divisor = self.wide(right, at, out)?;
        let (name, code, requires, inputs) = if dword {
            let (name, code) = if signed { ("__I8D32", &*_SDIV_CONST32) } else { ("__U8D32", &*_UDIV_CONST32) };
            (
                name,
                code,
                vec![(dividend.0, Register::EAX), (divisor.0, Register::EBX), (dividend.1, Register::EDX)],
                std::collections::BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Dx]),
            )
        } else {
            let (name, code) = if signed { ("__I8D", &*_SDIV) } else { ("__U8D", &*_UDIV) };
            (
                name,
                code,
                vec![
                    (dividend.0, Register::EAX),
                    (divisor.0, Register::EBX),
                    (divisor.1, Register::ECX),
                    (dividend.1, Register::EDX),
                ],
                _four_inputs(),
            )
        };
        let contract = _helper(name, inputs, _four_clobbers());
        let (quotient, remainder) = ((self.half(), self.half()), (self.half(), self.half()));
        let delivers = vec![
            (quotient.0, Register::EAX),
            (quotient.1, Register::EDX),
            (remainder.0, Register::EBX),
            (remainder.1, Register::ECX),
        ];
        out.push(Arc::new(Insn {
            clobbers: call_clobbers(&contract, self.segments),
            clobbers_high: call_clobbered_high(&contract, self.segments),
            uses: requires.iter().map(|(held, _)| held.value).collect(),
            requires,
            defines: delivers.iter().map(|(held, _)| held.value).collect(),
            delivers,
            call: Some(self.listed(llrm_mir::memory::Effects::NONE, Default::default())),
            ..Insn::new(at, Some((at, at)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![])
        }));
        self.calls.insert(at, name.to_owned());
        // The bytes are 386 code for a 16-bit segment.
        let code =
            if self.arch.object().bitness == 32 { crate::backend::lower_int64::flat(code) } else { code.to_vec() };
        self.inline.insert(at, code);
        Ok((quotient, remainder))
    }

    /// `llvm.smul.fix` or `llvm.sdiv.fix` of i32s, as the old route lowered
    /// FixedMul and FixedDiv: the widening `imul`'s pair shifted down by
    /// `shrd`; the dividend's pair shifted up by `shld` and `shl`, then one
    /// `idiv` where the quotient fits a dword, else the wrapping division.
    pub(super) fn fixed(
        &mut self,
        divide: bool,
        inst: InstId,
        arguments: &[Operand],
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let ty = instruction.ty;
        let result = Held { value: self.value(instruction.result.expect("a result")), width: 4 };
        let scale = self.constant(arguments[2], 4).filter(|scale| (1..32).contains(scale));
        let (Some(scale), Some(32)) = (scale, self.types().int_bits(ty)) else {
            return refuse("a fixed point other than i32 by 1 to 31 bits");
        };
        let (a, b) = (self.held(arguments[0], ty, at, out)?, self.held(arguments[1], ty, at, out)?);
        if !divide {
            let (low, high) = (self.half(), self.half());
            self.put(
                semantics(
                    Operation::Multiply,
                    "imul",
                    vec![Loc::Held(low), Loc::Held(high)],
                    vec![Loc::Held(a), Loc::Held(b)],
                ),
                at,
                out,
            );
            self.put(
                semantics(
                    Operation::Funnel,
                    "shrd",
                    vec![Loc::Held(result)],
                    vec![Loc::Held(low), Loc::Held(high), Self::count(scale)],
                ),
                at,
                out,
            );
            return Ok(());
        }
        let sign = self.made(Operation::Extend, "cdq", vec![Loc::Held(a)], at, out);
        let high =
            self.made(Operation::Funnel, "shld", vec![Loc::Held(sign), Loc::Held(a), Self::count(scale)], at, out);
        let low = self.made(Operation::Binary, "shl", vec![Loc::Held(a), Self::count(scale)], at, out);
        // A divisor of at least 1.0 cannot enlarge the dividend's magnitude.
        let fits = self
            .constant(arguments[1], 4)
            .is_some_and(|divisor| i64::from(divisor as i32).unsigned_abs() >= 1 << scale);
        if fits {
            let remainder = self.half();
            self.put(
                semantics(
                    Operation::Divide,
                    "idiv",
                    vec![Loc::Held(result), Loc::Held(remainder)],
                    vec![Loc::Held(high), Loc::Held(low), Loc::Held(b)],
                ),
                at,
                out,
            );
            return Ok(());
        }
        let (quotient, _) = self.divided(true, (low, high), b, at, out);
        self.put(semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(quotient)]), at, out);
        Ok(())
    }

    /// A signed i64 divided by an i32 sign-extended: the magnitudes by two
    /// unsigned divides, high dword then low, and the sign put back. Neither
    /// divide overflows; a zero divisor faults as a 32-bit one does.
    fn divided(
        &mut self,
        quotient: bool,
        (low, high): Pair,
        divisor: Held,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Pair {
        let sign = self.made(Operation::Binary, "sar", vec![Loc::Held(high), Self::count(31)], at, out);
        let (low, high) = self.negated_if(low, high, sign, at, out);
        let divisor_sign = self.made(Operation::Binary, "sar", vec![Loc::Held(divisor), Self::count(31)], at, out);
        let flipped = self.made(Operation::Binary, "xor", vec![Loc::Held(divisor), Loc::Held(divisor_sign)], at, out);
        let magnitude = self.made(Operation::Binary, "sub", vec![Loc::Held(flipped), Loc::Held(divisor_sign)], at, out);
        let zero = self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out);
        let divide = |selector: &mut Self, over: Held, under: Held, out: &mut Vec<Arc<Insn>>| {
            let (quotient, remainder) = (selector.half(), selector.half());
            let what = semantics(
                Operation::Divide,
                "div",
                vec![Loc::Held(quotient), Loc::Held(remainder)],
                vec![Loc::Held(over), Loc::Held(under), Loc::Held(magnitude)],
            );
            selector.put(what, at, out);
            (quotient, remainder)
        };
        let (upper, carried) = divide(self, zero, high, out);
        let (lower, remainder) = divide(self, carried, low, out);
        if quotient {
            let signed = self.made(Operation::Binary, "xor", vec![Loc::Held(sign), Loc::Held(divisor_sign)], at, out);
            return self.negated_if(lower, upper, signed, at, out);
        }
        // The remainder takes the dividend's sign, and is below 2^31: its
        // high dword is its low's sign, 0 for a zero remainder of any dividend.
        let flipped = self.made(Operation::Binary, "xor", vec![Loc::Held(remainder), Loc::Held(sign)], at, out);
        let low = self.made(Operation::Binary, "sub", vec![Loc::Held(flipped), Loc::Held(sign)], at, out);
        (low, self.made(Operation::Binary, "sar", vec![Loc::Held(low), Self::count(31)], at, out))
    }

    /// A signed i64 divided by `2^shift`, as LLVM's BuildSDIVPow2: a
    /// negative dividend biased by `2^shift - 1` so that the arithmetic
    /// shift rounds toward zero; the remainder what the shift drops.
    fn divided_by_power(
        &mut self,
        quotient: bool,
        (low, high): Pair,
        shift: i64,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Pair {
        let sign = self.made(Operation::Binary, "sar", vec![Loc::Held(high), Self::count(31)], at, out);
        let (bias_low, bias_high) = match shift {
            1..32 => (
                self.made(Operation::Binary, "shr", vec![Loc::Held(sign), Self::count(32 - shift)], at, out),
                self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out),
            ),
            32 => (sign, self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out)),
            _ => (sign, self.made(Operation::Binary, "shr", vec![Loc::Held(sign), Self::count(64 - shift)], at, out)),
        };
        let biased_low = self.made(Operation::Binary, "add", vec![Loc::Held(low), Loc::Held(bias_low)], at, out);
        let biased_high = self.made(Operation::Binary, "adc", vec![Loc::Held(high), Loc::Held(bias_high)], at, out);
        if quotient {
            return self.shifted(BinaryOp::AShr, biased_low, biased_high, shift, at, out);
        }
        let mask = (-1_i64 << shift) as u64;
        let kept_low =
            self.made(Operation::Binary, "and", vec![Loc::Held(biased_low), Self::dword(mask as u32 as i64)], at, out);
        let kept_high = match (mask >> 32) as u32 {
            u32::MAX => biased_high,
            word => {
                self.made(Operation::Binary, "and", vec![Loc::Held(biased_high), Self::dword(i64::from(word))], at, out)
            }
        };
        let low = self.made(Operation::Binary, "sub", vec![Loc::Held(low), Loc::Held(kept_low)], at, out);
        (low, self.made(Operation::Binary, "sbb", vec![Loc::Held(high), Loc::Held(kept_high)], at, out))
    }

    /// The pair negated where `sign` is all ones, unchanged where it is 0.
    fn negated_if(
        &mut self,
        low: Held,
        high: Held,
        sign: Held,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Pair {
        let low = self.made(Operation::Binary, "xor", vec![Loc::Held(low), Loc::Held(sign)], at, out);
        let high = self.made(Operation::Binary, "xor", vec![Loc::Held(high), Loc::Held(sign)], at, out);
        let low = self.made(Operation::Binary, "sub", vec![Loc::Held(low), Loc::Held(sign)], at, out);
        (low, self.made(Operation::Binary, "sbb", vec![Loc::Held(high), Loc::Held(sign)], at, out))
    }
}
