//! The arithmetic intrinsics (`visitIntrinsicCall`, `visitUnaryIntrinsic`):
//! integer min and max, bit counts, `bswap`, `abs`, and the float functions
//! and `lrint`, each one node on its operands. The legalizer turns what the
//! target lacks into library calls.

use llrm_mir::intrinsics::{BitCount, FloatFunction, Intrinsic};
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::opcode::BinaryOp;

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
            Intrinsic::ByteSwap => self.dag.get_node(ISD::BSwap, dest, &[first]),
            Intrinsic::Abs => {
                let poison = matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if !self.is_zero(*id)
                );
                self.dag.get_node(if poison { ISD::AbsMinPoison } else { ISD::Abs }, dest, &[first])
            }
            Intrinsic::LRint => self.dag.get_node_flags(ISD::LRint, dest, &[first], flags),
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
                };
                self.dag.get_node_flags(code, dest, &[first], flags)
            }
            _ => return Err(Refused("intrinsic call")),
        };
        self.set_value(instruction.result.ok_or(Refused("an arithmetic intrinsic without a result"))?, node);
        Ok(())
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
