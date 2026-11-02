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
//! on the integers of its operands, named by `libcall_name`. A compare is the
//! call and an integer compare of its result against zero
//! (`softenSetCCOperands`).
//!
//! DEVIATE: `setcc` of `ueq` and `one`, which the pin makes with two calls, is
//! not translated. Reason: no source reaches them yet. Exit: the first
//! corpus row that does.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::SoftenFloatResult` and `SoftenFloatOperand`
//! ([LegalizeFloatTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeFloatTypes.cpp)).

use super::{LegalizeError, TypeLegalizer};
use crate::backend::selectiondag::dag::{NodeData, SDValue};
use crate::backend::selectiondag::isd::{CondCode, ISD};
use crate::backend::selectiondag::mvt::{EVT, MVT};

/// `RTLIB::OEQ_F32` and the others: the routine, for a float and a double,
/// and the integer compare of its result against zero.
fn compare_routine(code: CondCode) -> Option<(&'static str, &'static str, CondCode)> {
    Some(match code {
        CondCode::SetOEq => ("__eqsf2", "__eqdf2", CondCode::SetEq),
        CondCode::SetUNe => ("__nesf2", "__nedf2", CondCode::SetNe),
        CondCode::SetOGe => ("__gesf2", "__gedf2", CondCode::SetGe),
        CondCode::SetOLt => ("__ltsf2", "__ltdf2", CondCode::SetLt),
        CondCode::SetOLe => ("__lesf2", "__ledf2", CondCode::SetLe),
        CondCode::SetOGt => ("__gtsf2", "__gtdf2", CondCode::SetGt),
        CondCode::SetUo => ("__unordsf2", "__unorddf2", CondCode::SetNe),
        CondCode::SetO => ("__unordsf2", "__unorddf2", CondCode::SetEq),
        // the unordered forms are the inverse of an ordered one: a NaN makes
        // the ordered routine's result fail its own compare
        CondCode::SetUGt => ("__lesf2", "__ledf2", CondCode::SetGt),
        CondCode::SetUGe => ("__ltsf2", "__ltdf2", CondCode::SetGe),
        CondCode::SetULt => ("__gesf2", "__gedf2", CondCode::SetLt),
        CondCode::SetULe => ("__gtsf2", "__gtdf2", CondCode::SetLe),
        _ => return None,
    })
}

/// The sign bit of a float of `bits` bits.
fn sign_bit(bits: u32) -> u128 {
    1u128 << (bits - 1)
}

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

    fn unsupported(
        &self,
        op: ISD,
        ty: EVT,
        what: &'static str,
    ) -> LegalizeError {
        LegalizeError::Unsupported { op, ty, what }
    }

    /// `makeLibCall`: the target's call of `name`, its result a `ret`.
    fn lib_call(
        &mut self,
        op: ISD,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        signed: bool,
    ) -> Result<SDValue, LegalizeError> {
        let Some(hooks) = self.hooks.as_mut() else {
            return Err(self.unsupported(op, ret, "a float routine without a target"));
        };
        hooks
            .make_lib_call(self.dag, name, args, ret, signed)
            .ok_or(
                self.unsupported(
                    op,
                    ret,
                    "a target that makes no calls",
                ),
            )
    }

    /// `SoftenFloatRes_Binary`: the routine for `op` on the softened operands.
    fn soften_binary(
        &mut self,
        value: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let (op, vt) = (self.dag.opcode(value), self.dag.value_type(value));
        let to = self.tli.types.type_conversion(vt).1;
        let name = self
            .tli
            .libcall_name(op, vt)
            .ok_or(
                self.unsupported(
                    op,
                    vt,
                    "a float routine the target does not name",
                ),
            )?;
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let args = [self.softened_float(lhs), self.softened_float(rhs)];
        self.lib_call(op, name, &args, to, false)
    }

    /// `SoftenFloatRes_XINT_TO_FP`: the integer widened to `i32` or `i64`,
    /// signed or not, and the routine for that width.
    fn soften_int_to_float(
        &mut self,
        value: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let (op, vt) = (self.dag.opcode(value), self.dag.value_type(value));
        let to = self.tli.types.type_conversion(vt).1;
        let source = self.dag.operand(value, 0);
        let signed = op == ISD::SIntToFp;
        let width = if self.dag.value_type(source).size_in_bits() <= 32 { EVT::I32 } else { EVT::I64 };
        let source = if self.dag.value_type(source) == width {
            source
        } else {
            self.dag.get_node(if signed { ISD::SignExtend } else { ISD::ZeroExtend }, width, &[source])
        };
        let name = self
            .tli
            .conv_libcall_name(op, width, vt)
            .ok_or(
                self.unsupported(
                    op,
                    vt,
                    "a conversion routine the target does not name",
                ),
            )?;
        self.lib_call(op, name, &[source], to, signed)
    }

    /// `SoftenFloatRes_FP_EXTEND` and `FP_ROUND`.
    fn soften_float_resize(
        &mut self,
        value: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let (op, vt) = (self.dag.opcode(value), self.dag.value_type(value));
        let to = self.tli.types.type_conversion(vt).1;
        let source = self.dag.operand(value, 0);
        let name = self
            .tli
            .conv_libcall_name(op, self.dag.value_type(source), vt)
            .ok_or(
                self.unsupported(
                    op,
                    vt,
                    "a conversion routine the target does not name",
                ),
            )?;
        let arg = self.softened_float(source);
        self.lib_call(op, name, &[arg], to, false)
    }

    /// `SoftenFloatResult`: `value` is a float result of its node; the node is
    /// replaced by one that makes the integer of the same width.
    pub(super) fn soften_float_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let to = self.tli.types.type_conversion(vt).1;
        let result = match self.dag.opcode(value) {
            // `SoftenFloatRes_BITCAST`: the integer it was made from.
            ISD::BitCast if self.dag.value_type(self.dag.operand(value, 0)).is_integer() => self.dag.operand(value, 0),
            // `SoftenFloatRes_ConstantFP`: its bits.
            ISD::ConstantFP => {
                let NodeData::ConstFp { bits } = self.dag.node(value).data else {
                    unreachable!("a float constant has bits")
                };
                self.dag.get_constant(bits as u128, to)
            }
            // `SoftenFloatRes_LOAD`: the load of the integer.
            ISD::Load => {
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("a load has a memory operand")
                };
                let load =
                    self.dag.get_load(to, chain, ptr, crate::backend::selectiondag::MemInfo { mem_vt: to, ..info });
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: load.node, res: 1 });
                load
            }
            // `SoftenFloatRes_FNEG`, `FABS`: the sign bit flipped or cleared.
            ISD::FNeg => {
                let operand = self.softened_float(self.dag.operand(value, 0));
                let sign = self.dag.get_constant(sign_bit(vt.size_in_bits()), to);
                self.dag.get_node(ISD::Xor, to, &[operand, sign])
            }
            ISD::FAbs => {
                let operand = self.softened_float(self.dag.operand(value, 0));
                let mask = self.dag.get_constant(sign_bit(vt.size_in_bits()) - 1, to);
                self.dag.get_node(ISD::And, to, &[operand, mask])
            }
            // `SoftenFloatRes_SELECT`
            ISD::Select => {
                let (cond, then, other) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let (then, other) = (self.softened_float(then), self.softened_float(other));
                self.dag.get_select(to, cond, then, other)
            }
            ISD::FAdd | ISD::FSub | ISD::FMul | ISD::FDiv => self.soften_binary(value)?,
            ISD::SIntToFp | ISD::UIntToFp => self.soften_int_to_float(value)?,
            ISD::FpExtend | ISD::FpRound => self.soften_float_resize(value)?,
            op => return Err(self.unsupported(op, vt, "SoftenFloatResult")),
        };
        self.set_softened_float(value, result);
        Ok(())
    }

    /// `softenSetCCOperands`: the routine for `code` on the softened operands
    /// and the integer compare of its result against zero.
    fn soften_setcc(
        &mut self,
        node: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let vt = self.dag.value_type(node);
        let operand_vt = self.dag.value_type(self.dag.operand(node, 0));
        let code =
            self.setcc_code(node).ok_or(self.unsupported(ISD::SetCC, operand_vt, "a compare with no condition"))?;
        let (single, double, compare) =
            compare_routine(code).ok_or(self.unsupported(ISD::SetCC, operand_vt, "a float compare of two routines"))?;
        let name = if operand_vt == EVT::Simple(MVT::F32) { single } else { double };
        let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let args = [self.softened_float(lhs), self.softened_float(rhs)];
        let result = self.lib_call(ISD::SetCC, name, &args, EVT::I32, true)?;
        let zero = self.dag.get_constant(0, EVT::I32);
        Ok(self.dag.get_setcc(vt, result, zero, compare))
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
            ISD::SetCC => self.soften_setcc(node)?,
            // `SoftenFloatOp_STORE`: the integer stored.
            ISD::Store if at == 1 => {
                let value = self.softened_float(self.dag.operand(node, 1));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 2));
                let to = self.dag.value_type(value);
                self.dag.get_store(chain, value, ptr, crate::backend::selectiondag::MemInfo { mem_vt: to, ..info })
            }
            // `SoftenFloatOp_FP_TO_XINT`
            op @ (ISD::FpToSInt | ISD::FpToUInt) => {
                let vt = self.dag.value_type(node);
                let source = self.dag.operand(node, 0);
                let wide = if vt.size_in_bits() <= 32 { EVT::I32 } else { EVT::I64 };
                let name = self
                    .tli
                    .conv_libcall_name(op, self.dag.value_type(source), wide)
                    .ok_or(self.unsupported(op, vt, "a conversion routine the target does not name"))?;
                let arg = self.softened_float(source);
                let result = self.lib_call(op, name, &[arg], wide, op == ISD::FpToSInt)?;
                if wide == vt { result } else { self.dag.get_node(ISD::Truncate, vt, &[result]) }
            }
            op => {
                let ty = self.dag.value_type(self.dag.operand(node, at));
                return Err(self.unsupported(op, ty, "SoftenFloatOperand"));
            }
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }
}
