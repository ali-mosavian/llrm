//! Target DAG combines: a multiply by a constant of the form `2^N +- 1` (times a power of two) is
//! a shift and an add or subtract, not a call to the multiply routine.
//!
//! DEVIATE: the pin does this in `visitMUL` when `TLI.decomposeMulByConstant` says so
//! ([L4908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4908));
//! the shared combiner has no such hook, so it is a target combine on `Mul` that runs when the
//! generic `visitMUL` finds nothing. Only the one-shift forms of the pin's block: no `2^N +- 2^M`.
//! Exit: the hook in the shared `visit_mul`.

use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineHooks, CombineLevel};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;

pub struct MosCombines;

impl CombineHooks for MosCombines {
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        matches!(op, ISD::Mul | ISD::SignExtend)
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SignExtend => sign_extend_bit(dag, node),
            _ => decompose_mul(dag, node),
        }
    }
}

/// A sign-extended condition bit, `0 - zext(bit)`: `select c, 1, 2` is `sext(c) + 2`, and nothing
/// selects `SignExtend` from `i1`.
fn sign_extend_bit(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (vt, bit) = (dag.value_type(node), dag.operand(node, 0));
    if dag.value_type(bit) != EVT::I1 || !matches!(vt, EVT::I8 | EVT::I16) {
        return None;
    }
    let wide = dag.get_node(ISD::ZeroExtend, vt, &[bit]);
    let zero = dag.get_constant(0, vt);
    Some(dag.get_node(ISD::Sub, vt, &[zero, wide]))
}

/// `x * c` as `(x << n) +- x`, shifted by the factors of two in `c` and negated for a negative `c`.
fn decompose_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I8 && vt != EVT::I16 {
        return None;
    }
    let (x, c) = (dag.operand(node, 0), dag.operand(node, 1));
    let raw = dag.constant(c).filter(|_| !dag.is_opaque(c))?;
    let width = vt.size_in_bits();
    let signed = if raw >> (width - 1) & 1 == 1 { raw as i128 - (1i128 << width) } else { raw as i128 };
    let mut magnitude = signed.unsigned_abs();
    // a power of two is a shift, which `visitMUL` has already made
    if magnitude.is_power_of_two() {
        return None;
    }
    let zeros = magnitude.trailing_zeros();
    magnitude >>= zeros;
    let (add, bits) = match (magnitude - 1, magnitude + 1) {
        (below, _) if below.is_power_of_two() => (true, below),
        (_, above) if above.is_power_of_two() => (false, above),
        _ => return None,
    };
    let amount = bits.trailing_zeros() + zeros;
    if amount >= width {
        return None;
    }
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u32| {
        let by = dag.get_constant(u128::from(by), EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let high = shift(dag, x, amount);
    let low = if zeros == 0 { x } else { shift(dag, x, zeros) };
    let mut result = dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[high, low]);
    if signed < 0 {
        let zero = dag.get_constant(0, vt);
        result = dag.get_node(ISD::Sub, vt, &[zero, result]);
    }
    Some(result)
}
