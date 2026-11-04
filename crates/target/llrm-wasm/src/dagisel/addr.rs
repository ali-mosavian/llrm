//! `SelectAddrOperands32` and `SelectAddrOperands64`: a load or store address
//! as an offset immediate and a base.
//!
//! DEVIATE W5: an `or` with a non-constant right side is not folded into the
//! offset. Reason: `Dag` has no `computeKnownBits`. The address is still
//! correct, the `or` stays a separate instruction. Exit: known bits in `Dag`.
//!
//! # See also
//! `WebAssemblyISelDAGToDAG.cpp`, pin d1106deb71cc: `SelectAddrOperands`
//! (L531), `SelectAddrAddOperands` (L493).

use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::generated::ids::isd;
use llrm_dagisel::matcher::Vt;

use super::generated::ids::{isd as wasm_isd, opcode};

/// Which address width a matcher serves.
#[derive(Clone, Copy)]
pub struct Width {
    pub vt: Vt,
    /// `CONST_I32` or `CONST_I64`.
    pub const_opcode: u16,
}

pub const ADDR32: Width = Width { vt: llrm_dagisel::generated::ids::mvt::i32, const_opcode: opcode::CONST_I32 };
pub const ADDR64: Width = Width { vt: llrm_dagisel::generated::ids::mvt::i64, const_opcode: opcode::CONST_I64 };

/// `Op` unwrapped from a `WebAssemblyISD::Wrapper`, when it is a
/// `TargetGlobalAddress`.
fn global_address<D: Dag>(
    dag: &D,
    mut op: SDValue,
) -> Option<SDValue> {
    if dag.opcode(op.node) == wasm_isd::Wrapper {
        op = dag.operand(op.node, 0);
    }
    (dag.opcode(op.node) == isd::TargetGlobalAddress).then_some(op)
}

/// The offset `0 + base`, with the base the constant zero address.
fn zero_base<D: Dag>(
    dag: &mut D,
    width: Width,
) -> SDValue {
    let zero = dag.target_constant(0, width.vt);
    SDValue::new(dag.machine_node(width.const_opcode, &[width.vt], &[zero]), 0)
}

/// `SelectAddrAddOperands`: a constant operand, or a global. An `add` folds
/// only with `nuw`: the offset is unsigned with infinite precision.
fn select_add_operands<D: Dag>(
    dag: &mut D,
    width: Width,
    n: SDValue,
) -> Option<(SDValue, SDValue)> {
    if dag.opcode(n.node) == isd::ADD && !dag.has_no_unsigned_wrap(n.node) {
        return None;
    }
    for i in 0..2 {
        let (op, other) = (dag.operand(n.node, i), dag.operand(n.node, 1 - i));
        if let Some(constant) = dag.constant(op) {
            let offset = dag.target_constant(constant_zext(constant, width), width.vt);
            return Some((offset, other));
        }
        if let Some(global) = global_address(dag, op) {
            return Some((global, other));
        }
    }
    None
}

/// `ConstantSDNode::getZExtValue` at the address width.
fn constant_zext(
    value: i64,
    width: Width,
) -> i64 {
    if width.const_opcode == opcode::CONST_I32 { i64::from(value as u32) } else { value }
}

/// `SelectAddrOperands`: `(offset, base)`.
pub fn select_addr_operands<D: Dag>(
    dag: &mut D,
    width: Width,
    n: SDValue,
) -> (SDValue, SDValue) {
    if let Some(global) = global_address(dag, n) {
        return (global, zero_base(dag, width));
    }
    if dag.opcode(n.node) == isd::ADD {
        if let Some(folded) = select_add_operands(dag, width, n) {
            return folded;
        }
    }
    if dag.opcode(n.node) == isd::OR {
        let disjoint = dag
            .constant(dag.operand(n.node, 1))
            .is_some_and(|mask| dag.masked_value_is_zero(dag.operand(n.node, 0), mask as u64));
        if disjoint {
            if let Some(folded) = select_add_operands(dag, width, n) {
                return folded;
            }
        }
    }
    if let Some(constant) = dag.constant(n) {
        let offset = dag.target_constant(constant_zext(constant, width), width.vt);
        return (offset, zero_base(dag, width));
    }
    (dag.target_constant(0, width.vt), n)
}
