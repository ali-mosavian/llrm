//! `TypeSoftenFloat`: a float on a target without an FPU is carried as the
//! integer of its width, and its operations are library calls.
//!
//! # How it works
//! [`TypeLegalizer::soften_float_result`] gives a node with a float result an
//! integer result (`SoftenFloatResult`);
//! [`TypeLegalizer::soften_float_operand`] deals with a node that reads a
//! softened float (`SoftenFloatOperand`). The integer of a float is recorded in
//! `softened`, as `promoted` records the promoted integers.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::SoftenFloatResult` and `SoftenFloatOperand`
//! ([LegalizeFloatTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeFloatTypes.cpp)).

use super::{LegalizeError, TypeLegalizer};
use crate::backend::selectiondag::dag::SDValue;

impl TypeLegalizer<'_> {
    /// `SoftenFloatResult`: `value` is a float result of its node; the node is
    /// replaced by one that makes the integer of the same width.
    pub(super) fn soften_float_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let op = self.dag.opcode(value);
        let ty = self.dag.value_type(value);
        Err(LegalizeError::Unsupported { op, ty, what: "SoftenFloatResult" })
    }

    /// `SoftenFloatOperand`: operand `at` of `n` is a softened float. Returns
    /// whether `n` must be analysed again.
    pub(super) fn soften_float_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let op = self.dag.opcode(SDValue { node: n, res: 0 });
        let ty = self.dag.value_type(self.dag.node(SDValue { node: n, res: 0 }).ops[at]);
        Err(LegalizeError::Unsupported { op, ty, what: "SoftenFloatOperand" })
    }
}
