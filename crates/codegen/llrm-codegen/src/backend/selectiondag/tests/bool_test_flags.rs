use super::super::dag_select::legalized;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;

fn lowered(code: CondCode) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let inner = dag.get_setcc(EVT::I8, a, b, CondCode::SetLt);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[inner]);
    let zero = dag.get_constant(0, EVT::I32);
    let outer = dag.get_setcc(EVT::I8, wide, zero, code);
    dag.set_root(outer);
    legalized(
        &mut dag,
        &tli,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false },
        &llrm_x86_m32::M32,
    )
    .unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// `((int)(a < b)) != 0` kept a second compare of the widened `setcc`
/// (`setl`, `movzx`, `test`, `setne`); the pin tests the inner compare's own
/// flags: one `setl`.
#[test]
fn a_nonzero_test_of_a_widened_setcc_is_the_setcc() {
    let (dag, root) = lowered(CondCode::SetNe);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (x86isd::SETCC, Some(cond::L)));
    let flags = dag.operand(root, 1);
    assert_eq!((dag.opcode(flags), flags.res), (x86isd::SUB, 1), "the inner compare's flags");
}

/// `== 0` is the opposite condition.
#[test]
fn a_zero_test_of_a_widened_setcc_is_the_opposite_condition() {
    let (dag, root) = lowered(CondCode::SetEq);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (x86isd::SETCC, Some(cond::GE)));
}
