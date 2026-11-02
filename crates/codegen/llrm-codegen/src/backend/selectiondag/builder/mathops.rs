//! The arithmetic intrinsics (`visitIntrinsicCall`, `visitUnaryIntrinsic`):
//! integer min and max, bit counts, `bswap`, `abs`, and the float functions
//! and `lrint`, each one node on its operands. The legalizer turns what the
//! target lacks into library calls.

use llrm_mir::intrinsics::{BitCount, FloatFunction, Intrinsic};
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::opcode::BinaryOp;

use super::super::dag::SDValue;
use super::super::isd::ISD;
use super::super::mvt::EVT;
use super::{Builder, Refused, flags_of};

impl Builder<'_> {
    pub(super) fn visit_math(
        &mut self,
        instruction: &Instruction,
        intrinsic: &Intrinsic,
    ) -> Result<(), Refused> {
        let flags = flags_of(instruction.flags);
        let first = self.get_value(instruction.operands[0])?;
        // `{iN, i1}`: the node's two results, which `extractvalue` picks from
        if let Intrinsic::WithOverflow { op, signed } = intrinsic {
            let second = self.get_value(instruction.operands[1])?;
            let code = match (op, signed) {
                (BinaryOp::Add, false) => ISD::UAddO,
                (BinaryOp::Add, true) => ISD::SAddO,
                (BinaryOp::Sub, false) => ISD::USubO,
                (BinaryOp::Sub, true) => ISD::SSubO,
                (BinaryOp::Mul, false) => ISD::UMulO,
                (BinaryOp::Mul, true) => ISD::SMulO,
                _ => return Err(Refused("overflow of an operation with no overflow node")),
            };
            let vt = self.dag.value_type(first);
            let node = self.dag.get_node_multi(code, &[vt, EVT::I1], &[first, second]);
            self.set_value(instruction.result.ok_or(Refused("an overflow intrinsic without a result"))?, node);
            return Ok(());
        }
        let dest = self.type_of(instruction.ty)?;
        let node = match intrinsic {
            Intrinsic::Compare { signed } => {
                let second = self.get_value(instruction.operands[1])?;
                self.dag.get_node(if *signed { ISD::SCmp } else { ISD::UCmp }, dest, &[first, second])
            }
            // `visitIntrinsicCall`: x86 has no fused multiply-add before FMA3
            // (`isFMAFasterThanFMulAndFAdd`), so a multiply and an add
            Intrinsic::FMulAdd => {
                let second = self.get_value(instruction.operands[1])?;
                let third = self.get_value(instruction.operands[2])?;
                let product = self.dag.get_node_flags(ISD::FMul, dest, &[first, second], flags);
                self.dag.get_node_flags(ISD::FAdd, dest, &[product, third], flags)
            }
            Intrinsic::CopySign => {
                let second = self.get_value(instruction.operands[1])?;
                self.dag.get_node(ISD::FCopySign, dest, &[first, second])
            }
            Intrinsic::Fixed { divide: false } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                self.dag.get_node(ISD::SMulFix, dest, &[first, second, scale])
            }
            Intrinsic::Fixed { divide: true } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                self.expand_div_fix(first, second, scale)
            }
            Intrinsic::FixedMul { signed, saturating } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                let code = match (*signed, *saturating) {
                    (true, _) => ISD::SMulFixSat,
                    (false, false) => ISD::UMulFix,
                    (false, true) => ISD::UMulFixSat,
                };
                self.dag.get_node(code, dest, &[first, second, scale])
            }
            Intrinsic::ShiftSat { signed } => {
                let second = self.get_value(instruction.operands[1])?;
                self.dag.get_node(if *signed { ISD::SShlSat } else { ISD::UShlSat }, dest, &[first, second])
            }
            Intrinsic::Saturating { op, signed } => {
                let second = self.get_value(instruction.operands[1])?;
                let code = match (op, signed) {
                    (BinaryOp::Add, true) => ISD::SAddSat,
                    (BinaryOp::Add, false) => ISD::UAddSat,
                    (_, true) => ISD::SSubSat,
                    (_, false) => ISD::USubSat,
                };
                self.dag.get_node(code, dest, &[first, second])
            }
            Intrinsic::MinMax { signed, max } => {
                let second = self.get_value(instruction.operands[1])?;
                let code = match (signed, max) {
                    (true, true) => ISD::SMax,
                    (true, false) => ISD::SMin,
                    (false, true) => ISD::UMax,
                    (false, false) => ISD::UMin,
                };
                self.dag.get_node(code, dest, &[first, second])
            }
            Intrinsic::BitCount(count) => {
                // The zero-is-poison flag picks the `_ZERO_UNDEF` form.
                let poison = !matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if self.is_zero(*id)
                );
                let code = match (count, poison) {
                    (BitCount::Population, _) => ISD::CtPop,
                    (BitCount::LeadingZeros, false) => ISD::CtLz,
                    (BitCount::LeadingZeros, true) => ISD::CtLzZeroPoison,
                    (BitCount::TrailingZeros, false) => ISD::CtTz,
                    (BitCount::TrailingZeros, true) => ISD::CtTzZeroPoison,
                };
                self.dag.get_node(code, dest, &[first])
            }
            // `visitIntrinsicCall`: an AND with the mask taken to the pointer's
            // width
            Intrinsic::PtrMask => {
                let mask = self.get_value(instruction.operands[1])?;
                let mask = self.dag.get_zext_or_trunc(mask, dest);
                self.dag.get_node(ISD::And, dest, &[first, mask])
            }
            Intrinsic::ByteSwap => self.dag.get_node(ISD::BSwap, dest, &[first]),
            Intrinsic::BitReverse => self.dag.get_node(ISD::BitReverse, dest, &[first]),
            Intrinsic::Abs => {
                let poison = matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if !self.is_zero(*id)
                );
                self.dag.get_node(if poison { ISD::AbsMinPoison } else { ISD::Abs }, dest, &[first])
            }
            Intrinsic::LRint => self.dag.get_node_flags(ISD::LRint, dest, &[first], flags),
            Intrinsic::LLRint => self.dag.get_node_flags(ISD::LLRint, dest, &[first], flags),
            Intrinsic::Unary(function) => {
                let code = match function {
                    FloatFunction::Fabs => ISD::FAbs,
                    FloatFunction::Sqrt => ISD::FSqrt,
                    FloatFunction::Sin => ISD::FSin,
                    FloatFunction::Cos => ISD::FCos,
                    FloatFunction::Atan => ISD::FAtan,
                    FloatFunction::Log2 => ISD::FLog2,
                    FloatFunction::Exp2 => ISD::FExp2,
                    FloatFunction::Log => ISD::FLog,
                    FloatFunction::Exp => ISD::FExp,
                    FloatFunction::Rint => ISD::FRint,
                    // Calls of the C library before selection (`libmcalls`).
                    FloatFunction::Floor
                    | FloatFunction::Ceil
                    | FloatFunction::Trunc
                    | FloatFunction::Round
                    | FloatFunction::RoundEven
                    | FloatFunction::NearbyInt => return Err(Refused("a rounding intrinsic without its library call")),
                };
                self.dag.get_node_flags(code, dest, &[first], flags)
            }
            _ => return Err(Refused("intrinsic call")),
        };
        self.set_value(instruction.result.ok_or(Refused("an arithmetic intrinsic without a result"))?, node);
        Ok(())
    }

    /// `expandDivFix` for `sdiv.fix`: x86 has no fixed-point divide, so the
    /// operands are widened by one bit and the node is promoted early (a scale
    /// of 0 stays at the type).
    fn expand_div_fix(
        &mut self,
        lhs: SDValue,
        rhs: SDValue,
        scale: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(lhs);
        if self.dag.constant(scale).unwrap_or(0) == 0 {
            return self.dag.get_node(ISD::SDivFix, vt, &[lhs, rhs, scale]);
        }
        let wide = EVT::integer(vt.size_in_bits() + 1);
        let lhs = self.dag.get_sext_or_trunc(lhs, wide);
        let rhs = self.dag.get_sext_or_trunc(rhs, wide);
        let result = self.dag.get_node(ISD::SDivFix, wide, &[lhs, rhs, scale]);
        self.dag.get_zext_or_trunc(result, vt)
    }

    /// Whether the constant is the integer zero.
    fn is_zero(
        &self,
        id: llrm_mir::context::ConstantId,
    ) -> bool {
        matches!(
            self.module.context.get(id).kind,
            llrm_mir::context::ConstantKind::Int(0)
        )
    }
}
