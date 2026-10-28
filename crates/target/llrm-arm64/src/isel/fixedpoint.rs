//! A multiply by a power of two beside an integer-float conversion is a
//! fixed-point conversion (`scvtf d0, w0, #1`; `fcvtzs x0, d0, #4`;
//! `AArch64DAGToDAGISel::SelectCVTFixedPosOperand`): `(double)(int)x * 2^-k`
//! and `(long)(d * 2^k)`, for `k` up to the integer's width.
//!
//! The conversion (or the multiply) is skipped where it is and the reader
//! emits the one instruction. Which pairs fold is decided from the IR alone,
//! so each side agrees: an integer-to-float conversion folds into the
//! multiply that reads it unless that multiply folds into a float-to-integer
//! conversion first.

use llrm_mir::{BinaryOp, CastOp, Instruction, Opcode, Operand};

use super::val::{G, Known, Val, width_of};
use super::{Selected, Selector, Src};
use crate::mc::inst::*;

impl Selector<'_> {
    /// The exponent of a float constant that is exactly a power of two.
    fn power_of_two(
        &self,
        operand: Operand,
    ) -> Option<i32> {
        let Operand::Constant(id) = operand else { return None };
        let llrm_mir::ConstantKind::Float(bits) = self.module.context.get(id).kind else { return None };
        let value = match self.ty(self.module.context.get(id).ty) {
            llrm_mir::Type::Float(llrm_mir::FloatKind::Float) => f64::from(f32::from_bits(bits as u32)),
            _ => f64::from_bits(bits),
        };
        let exponent = value.log2();
        (value > 0.0 && value.is_finite() && exponent.fract() == 0.0 && 2f64.powi(exponent as i32) == value)
            .then_some(exponent as i32)
    }

    /// `value`'s one reader, when it is in the block of `value`'s definition.
    fn reader_in_block(
        &self,
        value: llrm_mir::ValueId,
    ) -> Option<&Instruction> {
        if self.use_count(value) != 1 {
            return None;
        }
        let user = self.function.users(value).into_iter().next()?.user;
        let own = super::value_def(self.function, value).and_then(|inst| self.function.parent(inst));
        (own == self.function.parent(user)).then(|| self.function.instruction(user))
    }

    /// The width of the register a conversion of an integer of `bits` bits works in.
    fn conversion_width(bits: u32) -> u32 {
        if bits <= 32 { 32 } else { 64 }
    }

    /// The one operand of `mul` that is not the constant, and the constant's exponent.
    fn scaled(
        &self,
        mul: &Instruction,
    ) -> Option<(Operand, i32)> {
        match (self.power_of_two(mul.operands[0]), self.power_of_two(mul.operands[1])) {
            (None, Some(exponent)) => Some((mul.operands[0], exponent)),
            (Some(exponent), None) => Some((mul.operands[1], exponent)),
            _ => None,
        }
    }

    /// Whether the float multiply `mul` is read only by a float-to-integer conversion of 32 or 64
    /// bits, and by a power of two that fits its fractional bits.
    pub(super) fn mul_folds_into_conversion(
        &self,
        mul: &Instruction,
    ) -> bool {
        let Some(result) = mul.result else { return false };
        let Some(reader) = self.reader_in_block(result) else { return false };
        let Some((_, exponent)) = self.scaled(mul) else { return false };
        matches!(reader.opcode, Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI))
            && matches!(self.types().int_bits(reader.ty), Some(32 | 64))
            && (1..=self.types().int_bits(reader.ty).unwrap_or(0) as i32).contains(&exponent)
    }

    /// Whether the integer-to-float conversion `conv` is read only by a multiply by a power of two below
    /// one that does not itself fold into a float-to-integer conversion.
    pub(super) fn conversion_folds_into_mul(
        &self,
        conv: &Instruction,
    ) -> bool {
        let Some(result) = conv.result else { return false };
        let Some(reader) = self.reader_in_block(result) else { return false };
        if reader.opcode != Opcode::Binary(BinaryOp::FMul) || self.mul_folds_into_conversion(reader) {
            return false;
        }
        let Some((other, exponent)) = self.scaled(reader) else { return false };
        let Some(bits) = self.operand_type(conv.operands[0]).ok().and_then(|ty| self.types().int_bits(ty)) else {
            return false;
        };
        other == Operand::Value(result) && (1..=Self::conversion_width(bits) as i32).contains(&-exponent)
    }

    /// `mul` as a fixed-point conversion of the fused integer-to-float conversion it reads, if it reads one.
    pub(super) fn fixed_from_integer(
        &mut self,
        mul: &Instruction,
    ) -> Selected<bool> {
        let Some((Operand::Value(value), exponent)) = self.scaled(mul) else { return Ok(false) };
        let Some(def) = super::value_def(self.function, value).filter(|def| self.fused.contains(def)) else {
            return Ok(false);
        };
        let conv = self.function.instruction(def).clone();
        let signed = match conv.opcode {
            Opcode::Cast(CastOp::SIToFP) => true,
            Opcode::Cast(CastOp::UIToFP) => false,
            _ => return Ok(false),
        };
        let size = match self.ty(mul.ty) {
            llrm_mir::Type::Float(llrm_mir::FloatKind::Float) => FpSize::S,
            _ => FpSize::D,
        };
        let g = self.operand_g(conv.operands[0])?;
        let g = self.extended(g, if signed { Known::Sign } else { Known::Zero });
        let rd = self.mf.fpr();
        self.inst(Inst::FpFixed {
            op: FpConvert::ToFp { signed },
            width: width_of(g.bits),
            size,
            gpr: g.reg,
            fpr: rd,
            fbits: (-exponent) as u8,
        });
        self.define(mul.result, Src::Val(Val::Float { reg: rd, size }))?;
        Ok(true)
    }

    /// The float-to-integer `conversion` as a fixed-point one of the fused multiply it reads, if it reads one.
    pub(super) fn fixed_to_integer(
        &mut self,
        conversion: &Instruction,
    ) -> Selected<bool> {
        let Operand::Value(value) = conversion.operands[0] else { return Ok(false) };
        let Some(def) = super::value_def(self.function, value).filter(|def| self.fused.contains(def)) else {
            return Ok(false);
        };
        let mul = self.function.instruction(def).clone();
        if mul.opcode != Opcode::Binary(BinaryOp::FMul) {
            return Ok(false);
        }
        let Some((float, exponent)) = self.scaled(&mul) else { return Ok(false) };
        let signed = conversion.opcode == Opcode::Cast(CastOp::FPToSI);
        let bits = self.types().int_bits(conversion.ty).unwrap_or(64);
        let (rn, size) = self.float_operand(float)?;
        let rd = self.mf.gpr();
        self.inst(Inst::FpFixed {
            op: FpConvert::ToIntZero { signed },
            width: width_of(bits),
            size,
            gpr: rd,
            fpr: rn,
            fbits: exponent as u8,
        });
        let known = if bits == 32 { Known::Zero } else { Known::Nothing };
        self.define(conversion.result, Src::Val(Val::Int(G { reg: rd, bits, known })))?;
        Ok(true)
    }
}
