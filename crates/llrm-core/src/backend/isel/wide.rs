//! i64 as two dwords, low first, as LLVM's type legalizer expands an
//! integer no register holds. What a value's sign bits prove lets a product
//! or quotient of 32-bit values take one 32-bit instruction.

use std::sync::Arc;

use llrm_mir::module::{InstId, Operand};
use llrm_mir::valuetracking::sign_bits;
use llrm_mir::{BinaryOp, CastOp};

use super::{insn, refuse, semantics, Selector, Unselected};
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::Insn;

type Pair = (Held, Held);

impl Selector<'_, '_> {
    pub(super) fn is_wide(&self, ty: llrm_mir::TypeId) -> bool {
        self.types().int_bits(ty) == Some(64)
    }

    /// Whether `operand` is an i32 sign-extended.
    fn narrow(&self, operand: Operand) -> bool {
        sign_bits(&self.module.context, self.function, operand) > 32
    }

    fn put(&mut self, what: crate::model::ir::Semantics, at: i64, out: &mut Vec<Arc<Insn>>) {
        out.push(insn(at, what));
    }

    fn dword(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn count(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 1, address: None })
    }

    /// `into` made `from`.
    fn made(&mut self, operation: Operation, name: &str, sources: Vec<Loc>, at: i64, out: &mut Vec<Arc<Insn>>) -> Held {
        let into = self.fresh_held(4);
        self.put(semantics(operation, name, vec![Loc::Held(into)], sources), at, out);
        into
    }

    /// An i64 operand's halves, each in a register.
    fn wide(&mut self, operand: Operand, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Pair, Unselected> {
        if let Some(bits) = self.constant(operand, 8) {
            let low = self.made(Operation::Move, "mov", vec![Self::dword(bits as u32 as i64)], at, out);
            let high = self.made(Operation::Move, "mov", vec![Self::dword((bits >> 32) as u32 as i64)], at, out);
            return Ok((low, high));
        }
        match operand {
            Operand::Value(value) => self.wides.get(&value).copied().ok_or(()).or_else(|()| refuse("an i64 from no expanded instruction")),
            _ => refuse("an i64 constant of no bits"),
        }
    }

    /// A cast to or from i64.
    pub(super) fn wide_cast(&mut self, op: CastOp, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (operand, to) = (instruction.operands[0], instruction.ty);
        let from = self.function.operand_type(&self.module.context, operand).expect("a typed operand");
        let result = instruction.result.expect("a cast's value");
        if !self.is_wide(to) {
            let (low, _) = self.wide(operand, at, out)?;
            let into = Held { value: self.value(result), width: self.width(to)? };
            let what = if self.types().int_bits(to) == Some(1) {
                semantics(Operation::Binary, "and", vec![Loc::Held(into)], vec![Loc::Held(Held { width: 1, ..low }), Self::count(1)])
            } else {
                match op {
                    CastOp::Trunc => semantics(Operation::Move, "mov", vec![Loc::Held(into)], vec![Loc::Held(Held { width: into.width, ..low })]),
                    _ => return refuse(format!("{op:?} from an i64")),
                }
            };
            self.put(what, at, out);
            return Ok(());
        }
        let width = self.width(from)?;
        if self.types().int_bits(from) == Some(1) {
            return refuse("an i1 made i64");
        }
        let source = self.held(operand, from, at, out)?;
        let low = match (op, width) {
            (_, 4) => source,
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
    pub(super) fn wide_binary(&mut self, op: BinaryOp, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (left, right) = (instruction.operands[0], instruction.operands[1]);
        let result = instruction.result.expect("a result");
        let pair = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => {
                let (names, a, b) = (match op {
                    BinaryOp::Add => ["add", "adc"],
                    BinaryOp::Sub => ["sub", "sbb"],
                    BinaryOp::And => ["and", "and"],
                    BinaryOp::Or => ["or", "or"],
                    _ => ["xor", "xor"],
                }, self.wide(left, at, out)?, self.wide(right, at, out)?);
                // The carry runs from the low half's instruction to the high's.
                let low = self.made(Operation::Binary, names[0], vec![Loc::Held(a.0), Loc::Held(b.0)], at, out);
                let high = self.made(Operation::Binary, names[1], vec![Loc::Held(a.1), Loc::Held(b.1)], at, out);
                (low, high)
            }
            BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => {
                let Some(count) = self.constant(right, 1).map(|count| count & 63) else { return refuse("an i64 shifted by a variable") };
                let (low, high) = self.wide(left, at, out)?;
                self.shifted(op, low, high, count, at, out)
            }
            BinaryOp::Mul if self.narrow(left) && self.narrow(right) => {
                // Both are i32s: one signed widening multiply.
                let (a, b) = (self.wide(left, at, out)?.0, self.wide(right, at, out)?.0);
                let (low, high) = (self.fresh_held(4), self.fresh_held(4));
                self.put(semantics(Operation::Multiply, "imul", vec![Loc::Held(low), Loc::Held(high)], vec![Loc::Held(a), Loc::Held(b)]), at, out);
                (low, high)
            }
            BinaryOp::Mul => {
                // low*low widened, and each half by the other's low into the high.
                let (a, b) = (self.wide(left, at, out)?, self.wide(right, at, out)?);
                let (low, carried) = (self.fresh_held(4), self.fresh_held(4));
                self.put(semantics(Operation::Multiply, "mul", vec![Loc::Held(low), Loc::Held(carried)], vec![Loc::Held(a.0), Loc::Held(b.0)]), at, out);
                let first = self.made(Operation::Multiply, "imul", vec![Loc::Held(a.0), Loc::Held(b.1)], at, out);
                let second = self.made(Operation::Multiply, "imul", vec![Loc::Held(a.1), Loc::Held(b.0)], at, out);
                let crossed = self.made(Operation::Binary, "add", vec![Loc::Held(first), Loc::Held(second)], at, out);
                (low, self.made(Operation::Binary, "add", vec![Loc::Held(carried), Loc::Held(crossed)], at, out))
            }
            BinaryOp::SDiv | BinaryOp::SRem if self.narrow(right) => {
                let (dividend, divisor) = (self.wide(left, at, out)?, self.wide(right, at, out)?.0);
                self.divided(op == BinaryOp::SDiv, dividend, divisor, at, out)
            }
            _ => return refuse(format!("an i64 {}", instruction.opcode.mnemonic())),
        };
        self.wides.insert(result, pair);
        Ok(())
    }

    fn shifted(&mut self, op: BinaryOp, low: Held, high: Held, count: i64, at: i64, out: &mut Vec<Arc<Insn>>) -> Pair {
        let by = |count: i64| Self::count(count);
        match (op, count) {
            (_, 0) => (low, high),
            (BinaryOp::Shl, 1..32) => {
                let upper = self.made(Operation::Funnel, "shld", vec![Loc::Held(high), Loc::Held(low), by(count)], at, out);
                (self.made(Operation::Binary, "shl", vec![Loc::Held(low), by(count)], at, out), upper)
            }
            (BinaryOp::Shl, _) => {
                let upper = self.made(Operation::Binary, "shl", vec![Loc::Held(low), by(count - 32)], at, out);
                (self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out), upper)
            }
            (_, 1..32) => {
                let lower = self.made(Operation::Funnel, "shrd", vec![Loc::Held(low), Loc::Held(high), by(count)], at, out);
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

    /// A signed i64 divided by an i32 sign-extended: the magnitudes by two
    /// unsigned divides, high dword then low, and the sign put back. Neither
    /// divide overflows; a zero divisor faults as a 32-bit one does.
    fn divided(&mut self, quotient: bool, (low, high): Pair, divisor: Held, at: i64, out: &mut Vec<Arc<Insn>>) -> Pair {
        let sign = self.made(Operation::Binary, "sar", vec![Loc::Held(high), Self::count(31)], at, out);
        let (low, high) = self.negated_if(low, high, sign, at, out);
        let divisor_sign = self.made(Operation::Binary, "sar", vec![Loc::Held(divisor), Self::count(31)], at, out);
        let flipped = self.made(Operation::Binary, "xor", vec![Loc::Held(divisor), Loc::Held(divisor_sign)], at, out);
        let magnitude = self.made(Operation::Binary, "sub", vec![Loc::Held(flipped), Loc::Held(divisor_sign)], at, out);
        let zero = self.made(Operation::Move, "mov", vec![Self::dword(0)], at, out);
        let divide = |selector: &mut Self, over: Held, under: Held, out: &mut Vec<Arc<Insn>>| {
            let (quotient, remainder) = (selector.fresh_held(4), selector.fresh_held(4));
            let what = semantics(Operation::Divide, "div", vec![Loc::Held(quotient), Loc::Held(remainder)], vec![Loc::Held(over), Loc::Held(under), Loc::Held(magnitude)]);
            selector.put(what, at, out);
            (quotient, remainder)
        };
        let (upper, carried) = divide(self, zero, high, out);
        let (lower, remainder) = divide(self, carried, low, out);
        if quotient {
            let signed = self.made(Operation::Binary, "xor", vec![Loc::Held(sign), Loc::Held(divisor_sign)], at, out);
            return self.negated_if(lower, upper, signed, at, out);
        }
        // The remainder takes the dividend's sign, and is below 2^31.
        let flipped = self.made(Operation::Binary, "xor", vec![Loc::Held(remainder), Loc::Held(sign)], at, out);
        let low = self.made(Operation::Binary, "sub", vec![Loc::Held(flipped), Loc::Held(sign)], at, out);
        (low, sign)
    }

    /// The pair negated where `sign` is all ones, unchanged where it is 0.
    fn negated_if(&mut self, low: Held, high: Held, sign: Held, at: i64, out: &mut Vec<Arc<Insn>>) -> Pair {
        let low = self.made(Operation::Binary, "xor", vec![Loc::Held(low), Loc::Held(sign)], at, out);
        let high = self.made(Operation::Binary, "xor", vec![Loc::Held(high), Loc::Held(sign)], at, out);
        let low = self.made(Operation::Binary, "sub", vec![Loc::Held(low), Loc::Held(sign)], at, out);
        (low, self.made(Operation::Binary, "sbb", vec![Loc::Held(high), Loc::Held(sign)], at, out))
    }
}
