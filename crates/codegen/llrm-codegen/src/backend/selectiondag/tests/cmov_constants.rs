use super::super::dag_select::legalized;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

fn selected(
    on_true: u128,
    on_false: u128,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let test = dag.get_setcc(EVT::I8, a, b, CondCode::SetLt);
    let (t, f) = (dag.get_constant(on_true, EVT::I32), dag.get_constant(on_false, EVT::I32));
    let pick = dag.get_select(EVT::I32, test, t, f);
    dag.set_root(pick);
    legalized(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }, &llrm_x86_m32::M32)
        .unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// `c ? 8 : 0` was a `cmov` of two constants (two loads of immediates and a
/// branch on a 486); the pin makes `zext(setcc) << 3`.
#[test]
fn a_power_of_two_or_zero_is_a_shifted_setcc() {
    let (dag, root) = selected(8, 0);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 1))), (ISD::Shl, Some(3)));
    let widened = dag.operand(root, 0);
    assert_eq!((dag.opcode(widened), dag.opcode(dag.operand(widened, 0))), (ISD::ZeroExtend, x86isd::SETCC));
}

/// `c ? k + 1 : k` is `zext(setcc) + k`.
#[test]
fn one_more_than_the_other_constant_is_a_setcc_plus_it() {
    let (dag, root) = selected(5, 4);
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(4));
}

/// A difference an `lea` multiplies (3 here) is a setcc scaled; one that is
/// not (7) stays a `cmov`; the smaller constant first flips the condition.
#[test]
fn only_a_fast_multiplier_difference_is_folded() {
    let (dag, root) = selected(13, 10);
    assert_ne!(dag.opcode(root), x86isd::CMOV, "{}", dag.dump());
    let (dag, root) = selected(17, 10);
    assert_eq!(dag.opcode(root), x86isd::CMOV, "{}", dag.dump());
    let (dag, root) = selected(0, 8);
    assert_eq!(dag.opcode(root), ISD::Shl, "swapped: {}", dag.dump());
}
