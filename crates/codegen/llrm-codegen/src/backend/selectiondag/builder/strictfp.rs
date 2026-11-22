//! `visitConstrainedFPIntrinsic`: `llvm.experimental.constrained.*` becomes
//! one `STRICT_*` node on a chain, with the value and the out chain as its
//! results.
//!
//! # How it works
//! The metadata arguments are read, not passed: the exception behavior picks
//! the chain the node hangs on (`getFPOperationRoot`) and the list its out
//! chain waits in (`pushFPOpOutChain`); `fpexcept.ignore` sets `NoFPExcept`.
//! The rounding mode is not in the node: the target reads the control word.
//!
//! # Runs
//! At instruction selection of a call to a constrained intrinsic.
//!
//! DEVIATE CFP1: the operations `ConstrainedOp` names (add, sub, mul, div,
//! the conversions, rint, fcmp and fcmps), not the whole of
//! `ConstrainedOps.def`. Reason: the front end emits no other. Exit: a
//! `ConstrainedOp` is added with its node. Owner w9-tests. Review 2026-11-27.
//!
//! # See also
//! [`SelectionDAGBuilder::visitConstrainedFPIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L8737).

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::ConstrainedOp;
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::opcode::FloatPredicate;

use super::super::dag::{NodeData, SDNodeFlags, SDValue};
use super::super::isd::ISD;
use super::super::mvt::EVT;
use super::{Builder, Refused, float_cond_code};

/// `fp::ExceptionBehavior`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExceptionBehavior {
    Ignore,
    MayTrap,
    Strict,
}

impl ExceptionBehavior {
    /// `convertStrToExceptionBehavior`.
    fn parse(text: &str) -> Option<ExceptionBehavior> {
        match text {
            "fpexcept.ignore" => Some(ExceptionBehavior::Ignore),
            "fpexcept.maytrap" => Some(ExceptionBehavior::MayTrap),
            "fpexcept.strict" => Some(ExceptionBehavior::Strict),
            _ => None,
        }
    }
}

/// `ConstrainedFPCmpIntrinsic::getPredicate`, by the metadata text.
fn predicate(text: &str) -> Option<FloatPredicate> {
    use FloatPredicate as P;
    Some(match text {
        "oeq" => P::Oeq,
        "ogt" => P::Ogt,
        "oge" => P::Oge,
        "olt" => P::Olt,
        "ole" => P::Ole,
        "one" => P::One,
        "ord" => P::Ord,
        "ueq" => P::Ueq,
        "ugt" => P::Ugt,
        "uge" => P::Uge,
        "ult" => P::Ult,
        "ule" => P::Ule,
        "une" => P::Une,
        "uno" => P::Uno,
        _ => return None,
    })
}

/// The node of an operation and how many of its arguments are values (the
/// rest are metadata).
fn node_of(op: ConstrainedOp) -> (ISD, usize) {
    match op {
        ConstrainedOp::FAdd => (ISD::StrictFAdd, 2),
        ConstrainedOp::FSub => (ISD::StrictFSub, 2),
        ConstrainedOp::FMul => (ISD::StrictFMul, 2),
        ConstrainedOp::FDiv => (ISD::StrictFDiv, 2),
        ConstrainedOp::FpExt => (ISD::StrictFpExtend, 1),
        ConstrainedOp::FpTrunc => (ISD::StrictFpRound, 1),
        ConstrainedOp::SiToFp => (ISD::StrictSIntToFp, 1),
        ConstrainedOp::UiToFp => (ISD::StrictUIntToFp, 1),
        ConstrainedOp::FpToSi => (ISD::StrictFpToSInt, 1),
        ConstrainedOp::FpToUi => (ISD::StrictFpToUInt, 1),
        ConstrainedOp::Rint => (ISD::StrictFRint, 1),
        ConstrainedOp::FCmp => (ISD::StrictFSetCC, 2),
        ConstrainedOp::FCmps => (ISD::StrictFSetCCS, 2),
    }
}

impl Builder<'_> {
    fn metadata_text(
        &self,
        operand: Operand,
    ) -> Result<&str, Refused> {
        match operand {
            Operand::Constant(id) => match &self.module.context.get(id).kind {
                ConstantKind::MetadataString(text) => Ok(text),
                _ => Err(Refused("a constrained intrinsic's metadata argument is not a string")),
            },
            _ => Err(Refused("a constrained intrinsic's metadata argument is not a string")),
        }
    }

    /// `getFPOperationRoot`.
    fn fp_operation_root(
        &mut self,
        behavior: ExceptionBehavior,
    ) -> SDValue {
        match behavior {
            // Not observed, so their order is free; but never between
            // `strict` ones.
            ExceptionBehavior::Ignore | ExceptionBehavior::MayTrap => {
                if !self.pending_constrained_fp_strict.is_empty() {
                    let strict = std::mem::take(&mut self.pending_constrained_fp_strict);
                    self.update_root(strict);
                }
            }
            // Without trapping the flags are read only at a barrier, so the
            // order between barriers is free; with it each is a point to
            // observe. Trapping is not modelled: both chain up the others.
            ExceptionBehavior::Strict => {
                if !self.pending_constrained_fp.is_empty() {
                    let ignored = std::mem::take(&mut self.pending_constrained_fp);
                    self.update_root(ignored);
                }
            }
        }
        self.dag.get_root()
    }

    /// `pushFPOpOutChain`.
    fn push_fp_op_out_chain(
        &mut self,
        result: SDValue,
        behavior: ExceptionBehavior,
    ) {
        let chain = SDValue { node: result.node, res: 1 };
        match behavior {
            ExceptionBehavior::Ignore | ExceptionBehavior::MayTrap => self.pending_constrained_fp.push(chain),
            ExceptionBehavior::Strict => self.pending_constrained_fp_strict.push(chain),
        }
    }

    pub(super) fn visit_constrained_fp(
        &mut self,
        instruction: &Instruction,
        op: ConstrainedOp,
    ) -> Result<(), Refused> {
        let (_callee, arguments) = instruction.operands.split_last().expect("a call names its callee");
        let (opcode, values) = node_of(op);
        // The exception behavior is the last argument; a predicate precedes
        // it, rounding precedes both.
        let except = arguments.last().ok_or(Refused("a constrained intrinsic with no arguments"))?;
        let behavior = ExceptionBehavior::parse(self.metadata_text(*except)?)
            .ok_or(Refused("a constrained intrinsic's exception behavior is not one LLVM names"))?;

        let chain = self.fp_operation_root(behavior);
        let mut operands = vec![chain];
        for argument in &arguments[..values] {
            operands.push(self.get_value(*argument)?);
        }
        let vt = self.type_of(instruction.ty)?;
        match opcode {
            // The second operand of `FP_ROUND`: the value is not known to be
            // exact in the narrower type.
            ISD::StrictFpRound => operands.push(self.dag.get_constant_with(0, self.target.dag.pointer, true, false)),
            ISD::StrictFSetCC | ISD::StrictFSetCCS => {
                let text = self.metadata_text(arguments[values])?;
                let mut code = float_cond_code(
                    predicate(text).ok_or(Refused("a constrained compare's predicate is not one LLVM names"))?,
                );
                if self.dag.is_known_never_nan(operands[1], false) && self.dag.is_known_never_nan(operands[2], false) {
                    code = super::mergedcond::without_nan(code);
                }
                operands.push(self.dag.get_cond_code(code));
            }
            _ => {}
        }
        let flags = SDNodeFlags::default().with(SDNodeFlags::NO_FP_EXCEPT, behavior == ExceptionBehavior::Ignore);
        let result = self.dag.intern(opcode, &[vt, EVT::OTHER], &operands, flags, NodeData::None);
        self.push_fp_op_out_chain(result, behavior);
        self.set_value(
            instruction.result.ok_or(Refused("a constrained intrinsic without a result"))?,
            SDValue { node: result.node, res: 0 },
        );
        Ok(())
    }
}
