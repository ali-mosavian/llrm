//! Floating point: arithmetic, comparisons and the conversions between
//! floats and integers.

use llrm_mir::{BinaryOp, CastOp, FloatPredicate, Instruction, Operand};

use super::val::{G, Known, Val, width_of};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;

/// The condition (or two, for a predicate no single one expresses) that holds
/// after `fcmp` where the predicate does, `changeFPCCToAArch64CC`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp>).
pub(super) fn conditions(predicate: FloatPredicate) -> (Cond, Option<Cond>) {
    match predicate {
        FloatPredicate::Oeq => (Cond::Eq, None),
        FloatPredicate::Ogt => (Cond::Gt, None),
        FloatPredicate::Oge => (Cond::Ge, None),
        FloatPredicate::Olt => (Cond::Mi, None),
        FloatPredicate::Ole => (Cond::Ls, None),
        FloatPredicate::One => (Cond::Mi, Some(Cond::Gt)),
        FloatPredicate::Ord => (Cond::Vc, None),
        FloatPredicate::Ueq => (Cond::Eq, Some(Cond::Vs)),
        FloatPredicate::Ugt => (Cond::Hi, None),
        FloatPredicate::Uge => (Cond::Pl, None),
        FloatPredicate::Ult => (Cond::Lt, None),
        FloatPredicate::Ule => (Cond::Le, None),
        FloatPredicate::Une => (Cond::Ne, None),
        FloatPredicate::Uno => (Cond::Vs, None),
        FloatPredicate::True => (Cond::Al, None),
        FloatPredicate::False => (Cond::Nv, None),
    }
}

impl Selector<'_> {
    pub(super) fn float_operand(
        &mut self,
        operand: Operand,
    ) -> Selected<(Vreg, FpSize)> {
        let src = self.src(operand)?;
        match self.materialize(src)? {
            Val::Float { reg, size } => Ok((reg, size)),
            _ => refuse("a float operation on an integer"),
        }
    }

    pub(super) fn float_binary(
        &mut self,
        op: BinaryOp,
        instruction: &Instruction,
    ) -> Selected<()> {
        if op == BinaryOp::FMul {
            if self.mul_folds_into_conversion(instruction) {
                // The float-to-integer conversion that reads it scales the
                // value itself.
                let inst = super::value_def(self.function, instruction.result.expect("a multiply has a result"))
                    .expect("an instruction");
                self.fused.insert(inst);
                return Ok(());
            }
            if self.fixed_from_integer(instruction)? {
                return Ok(());
            }
        }
        let fp = match op {
            BinaryOp::FAdd => FpBinary::Add,
            BinaryOp::FSub => FpBinary::Sub,
            BinaryOp::FMul => FpBinary::Mul,
            BinaryOp::FDiv => FpBinary::Div,
            _ => return refuse("frem"),
        };
        let ((a, size), (b, _)) =
            (self.float_operand(instruction.operands[0])?, self.float_operand(instruction.operands[1])?);
        let rd = self.mf.fpr();
        self.inst(Inst::FpBinary { op: fp, size, rd, rn: a, rm: b });
        self.define(instruction.result, Src::Val(Val::Float { reg: rd, size }))
    }

    pub(super) fn fneg(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let (a, size) = self.float_operand(instruction.operands[0])?;
        let rd = self.mf.fpr();
        self.inst(Inst::FpUnary { op: FpUnary::Neg, size, rd, rn: a });
        self.define(instruction.result, Src::Val(Val::Float { reg: rd, size }))
    }

    /// Sets the flags for `a predicate b` and names the condition that holds
    /// (the first of two, where the predicate needs two).
    pub(crate) fn float_compare(
        &mut self,
        predicate: FloatPredicate,
        a: Operand,
        b: Operand,
    ) -> Selected<Cond> {
        let ((left, size), right) = (self.float_operand(a)?, self.src(b)?);
        let zero = matches!(right, Src::Float { bits: 0, .. });
        let rm = if zero {
            None
        } else {
            let Val::Float { reg, .. } = self.materialize(right)? else {
                return refuse("a float compared with an integer");
            };
            Some(reg)
        };
        self.inst(Inst::FpCompare { signalling: false, size, rn: left, rm });
        Ok(conditions(predicate).0)
    }

    pub(super) fn fcmp_value(
        &mut self,
        predicate: FloatPredicate,
        instruction: &Instruction,
    ) -> Selected<()> {
        let result = instruction.result.expect("an fcmp has a result");
        if self.fusable(result) {
            return Ok(());
        }
        let first = self.float_compare(predicate, instruction.operands[0], instruction.operands[1])?;
        let (_, second) = conditions(predicate);
        let rd = self.mf.gpr();
        self.cset(rd, first);
        if let Some(second) = second {
            let other = self.mf.gpr();
            self.cset(other, second);
            self.inst(Inst::LogicShifted {
                op: Logic::Orr,
                invert: false,
                width: Width::W,
                rd,
                rn: rd,
                rm: other,
                shift: Shift::Lsl,
                amount: 0,
            });
        }
        self.define(Some(result), Src::Val(Val::Int(G { reg: rd, bits: 1, known: Known::Zero })))
    }

    /// `rd = cond ? 1 : 0` (`csinc rd, wzr, wzr, !cond`).
    pub(crate) fn cset(
        &mut self,
        rd: Gpr,
        cond: Cond,
    ) {
        self.inst(Inst::CondSelect {
            op: Select::Csinc,
            width: Width::W,
            rd,
            rn: Gpr::R31,
            rm: Gpr::R31,
            cond: cond.inverted(),
        });
    }

    pub(super) fn float_cast(
        &mut self,
        op: CastOp,
        instruction: &Instruction,
    ) -> Selected<()> {
        let result = instruction.result;
        let to = instruction.ty;
        let source = instruction.operands[0];
        match op {
            CastOp::FPExt | CastOp::FPTrunc => {
                let (rn, from) = self.float_operand(source)?;
                let target = match self.ty(to) {
                    llrm_mir::Type::Float(llrm_mir::FloatKind::Float) => FpSize::S,
                    llrm_mir::Type::Float(llrm_mir::FloatKind::Double) => FpSize::D,
                    other => return refuse(format!("a float conversion to {other:?}")),
                };
                let rd = self.mf.fpr();
                self.inst(Inst::FpToFp { to: target, from, rd, rn });
                self.define(result, Src::Val(Val::Float { reg: rd, size: target }))
            }
            // BASIC's store of a SINGLE or DOUBLE rounds it to its type; the
            // machine's floats are that type already.
            CastOp::FPRound => {
                let (reg, size) = self.float_operand(source)?;
                self.define(result, Src::Val(Val::Float { reg, size }))
            }
            CastOp::SIToFP | CastOp::UIToFP if self.conversion_folds_into_mul(instruction) => {
                // The multiply that reads it converts with fractional bits.
                let inst = super::value_def(self.function, result.expect("a conversion has a result"))
                    .expect("an instruction");
                self.fused.insert(inst);
                Ok(())
            }
            CastOp::FPToSI | CastOp::FPToUI if self.fixed_to_integer(instruction)? => Ok(()),
            CastOp::SIToFP | CastOp::UIToFP => {
                let signed = op == CastOp::SIToFP;
                let g = self.operand_g(source)?;
                // A narrow integer is extended to 32 or 64 bits first.
                let g = self.extended(g, if signed { Known::Sign } else { Known::Zero });
                let size = match self.ty(to) {
                    llrm_mir::Type::Float(llrm_mir::FloatKind::Float) => FpSize::S,
                    llrm_mir::Type::Float(llrm_mir::FloatKind::Double) => FpSize::D,
                    other => return refuse(format!("an integer conversion to {other:?}")),
                };
                let rd = self.mf.fpr();
                self.inst(Inst::FpInt {
                    op: FpConvert::ToFp { signed },
                    width: width_of(g.bits),
                    size,
                    gpr: g.reg,
                    fpr: rd,
                });
                self.define(result, Src::Val(Val::Float { reg: rd, size }))
            }
            CastOp::FPToSI | CastOp::FPToUI => {
                let signed = op == CastOp::FPToSI;
                let (rn, size) = self.float_operand(source)?;
                let Some(bits) = self.types().int_bits(to) else {
                    return refuse("a float conversion to a non-integer");
                };
                let rd = self.mf.gpr();
                self.inst(Inst::FpInt {
                    op: FpConvert::ToIntZero { signed },
                    width: width_of(bits),
                    size,
                    gpr: rd,
                    fpr: rn,
                });
                let known = if bits == 32 {
                    Known::Zero
                } else if bits < 32 {
                    Known::Hi32Zero
                } else {
                    Known::Nothing
                };
                self.define(result, Src::Val(Val::Int(G { reg: rd, bits, known })))
            }
            CastOp::BitCast => {
                // Between an integer and a float of one size.
                match self.src(source)? {
                    Src::Val(Val::Float { reg, size }) => {
                        let Some(bits) = self.types().int_bits(to) else {
                            return refuse("a float bitcast to a non-integer");
                        };
                        let rd = self.mf.gpr();
                        self.inst(Inst::FpInt {
                            op: FpConvert::Bits { to_fp: false },
                            width: width_of(bits),
                            size,
                            gpr: rd,
                            fpr: reg,
                        });
                        self.define(result, Src::Val(Val::Int(G { reg: rd, bits, known: Known::Zero })))
                    }
                    other => {
                        let g = self.gpr_of(other)?;
                        let size = if g.bits == 32 { FpSize::S } else { FpSize::D };
                        let rd = self.mf.fpr();
                        self.inst(Inst::FpInt {
                            op: FpConvert::Bits { to_fp: true },
                            width: width_of(g.bits),
                            size,
                            gpr: g.reg,
                            fpr: rd,
                        });
                        self.define(result, Src::Val(Val::Float { reg: rd, size }))
                    }
                }
            }
            other => refuse(format!("the cast {other:?}")),
        }
    }
}
