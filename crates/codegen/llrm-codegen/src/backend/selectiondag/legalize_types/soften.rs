//! `TypeSoftenFloat`: a float on a target without an FPU is carried as the
//! integer of its width, and its operations are library calls.
//!
//! # How it works
//! [`TypeLegalizer::soften_float_result`] gives a node with a float result an
//! integer result (`SoftenFloatResult`);
//! [`TypeLegalizer::soften_float_operand`] deals with a node that reads a
//! softened float (`SoftenFloatOperand`). The integer of a float is recorded in
//! `softened`, as `promoted` records the promoted integers. An operation is the
//! target's library call
//! ([`Hooks::make_lib_call`](crate::backend::selectiondag::legalize_dag::Hooks))
//! on the integers of its operands, named by `libcall_name`.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::SoftenFloatResult` and `SoftenFloatOperand`
//! ([LegalizeFloatTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeFloatTypes.cpp)).

use super::{LegalizeError, TypeLegalizer};
use crate::backend::selectiondag::dag::SDValue;
use crate::backend::selectiondag::isd::ISD;

impl TypeLegalizer<'_> {
    /// `GetSoftenedFloat`.
    pub(super) fn softened_float(
        &self,
        value: SDValue,
    ) -> SDValue {
        let value = self.remap(value);
        *self.softened.get(&value).expect("operand softened before its user")
    }

    /// `SetSoftenedFloat`.
    fn set_softened_float(
        &mut self,
        value: SDValue,
        result: SDValue,
    ) {
        let value = self.remap(value);
        let result = self.analyze_new_value(result);
        self.softened.insert(value, result);
    }

    /// `SoftenFloatRes_Binary`: the routine for `op` on the softened operands,
    /// its result an integer of the float's width.
    fn soften_binary(
        &mut self,
        value: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let (op, vt) = (self.dag.opcode(value), self.dag.value_type(value));
        let unsupported = |what| LegalizeError::Unsupported { op, ty: vt, what };
        let to = self.tli.types.type_conversion(vt).1;
        let name =
            self.tli.libcall_name(op, vt).ok_or_else(|| unsupported("a float routine the target does not name"))?;
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let args = [self.softened_float(lhs), self.softened_float(rhs)];
        let hooks = self.hooks.as_mut().ok_or_else(|| unsupported("a float routine without a target"))?;
        hooks.make_lib_call(self.dag, name, &args, to, false).ok_or_else(|| unsupported("a target that makes no calls"))
    }

    /// `SoftenFloatResult`: `value` is a float result of its node; the node is
    /// replaced by one that makes the integer of the same width.
    pub(super) fn soften_float_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let result = match self.dag.opcode(value) {
            // `SoftenFloatRes_BITCAST`: the integer it was made from.
            ISD::BitCast if self.dag.value_type(self.dag.operand(value, 0)).is_integer() => self.dag.operand(value, 0),
            ISD::FAdd | ISD::FSub | ISD::FMul | ISD::FDiv => self.soften_binary(value)?,
            op => {
                let ty = self.dag.value_type(value);
                return Err(LegalizeError::Unsupported { op, ty, what: "SoftenFloatResult" });
            }
        };
        self.set_softened_float(value, result);
        Ok(())
    }

    /// `SoftenFloatOperand`: operand `at` of `n` is a softened float. Returns
    /// whether `n` must be analysed again.
    pub(super) fn soften_float_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let result = match self.dag.opcode(node) {
            // `SoftenFloatOp_BITCAST`: the float read as the integer it is.
            ISD::BitCast => self.softened_float(self.dag.operand(node, at)),
            op => {
                let ty = self.dag.value_type(self.dag.operand(node, at));
                return Err(LegalizeError::Unsupported { op, ty, what: "SoftenFloatOperand" });
            }
        };
        self.replace_value_with(node, result);
        Ok(false)
    }
}
