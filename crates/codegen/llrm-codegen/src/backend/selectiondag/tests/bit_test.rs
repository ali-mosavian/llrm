use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;

fn lowered(
    build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue
) -> (SelectionDAG, SDValue, [SDValue; 2]) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (x, n) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I8, 6));
    let root = build(&mut dag, x, n);
    dag.set_root(root);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }).run().unwrap();
    let root = dag.get_root();
    (dag, root, [x, n])
}

/// `(x & (1 << n)) == 0` was `shl`, `and`, `test`; the pin tests the bit:
/// `bt x, n` with the carry clear (`AE`).
#[test]
fn a_masked_variable_bit_equal_to_zero_is_a_bit_test_with_the_carry_clear() {
    let (dag, root, [x, n]) = lowered(|dag, x, n| {
        let one = dag.get_constant(1, EVT::I32);
        let bit = dag.get_node(ISD::Shl, EVT::I32, &[one, n]);
        let and = dag.get_node(ISD::And, EVT::I32, &[x, bit]);
        let zero = dag.get_constant(0, EVT::I32);
        dag.get_setcc(EVT::I8, and, zero, CondCode::SetEq)
    });
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (x86isd::SETCC, Some(cond::AE)));
    let flags = dag.operand(root, 1);
    assert_eq!(dag.opcode(flags), x86isd::BT);
    assert_eq!(dag.operand(flags, 0), x);
    // The shift amount is widened to the operand's width.
    assert_eq!(dag.value_type(dag.operand(flags, 1)), EVT::I32);
    let _ = n;
}

/// `((x >> n) & 1) != 0` is the same bit, carry set (`B`).
#[test]
fn a_shifted_bit_not_equal_to_zero_is_a_bit_test_with_the_carry_set() {
    let (dag, root, [x, _]) = lowered(|dag, x, n| {
        let amount = dag.get_node(ISD::AnyExtend, EVT::I32, &[n]);
        let shifted = dag.get_node(ISD::Srl, EVT::I32, &[x, amount]);
        let one = dag.get_constant(1, EVT::I32);
        let and = dag.get_node(ISD::And, EVT::I32, &[shifted, one]);
        let zero = dag.get_constant(0, EVT::I32);
        dag.get_setcc(EVT::I8, and, zero, CondCode::SetNe)
    });
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (x86isd::SETCC, Some(cond::B)));
    assert_eq!(dag.operand(dag.operand(root, 1), 0), x);
}

/// A flipped source flips the test: `(~x & (1 << n)) == 0` is `bt` with `B`.
#[test]
fn a_complemented_source_flips_the_condition() {
    let (dag, root, [x, _]) = lowered(|dag, x, n| {
        let not = dag.get_not(x, EVT::I32);
        let one = dag.get_constant(1, EVT::I32);
        let bit = dag.get_node(ISD::Shl, EVT::I32, &[one, n]);
        let and = dag.get_node(ISD::And, EVT::I32, &[not, bit]);
        let zero = dag.get_constant(0, EVT::I32);
        dag.get_setcc(EVT::I8, and, zero, CondCode::SetEq)
    });
    assert_eq!(dag.constant(dag.operand(root, 0)), Some(cond::B));
    assert_eq!(dag.operand(dag.operand(root, 1), 0), x);
}

/// A mask that is not a single variable bit keeps the `test`.
#[test]
fn another_mask_keeps_the_test() {
    let (dag, root, _) = lowered(|dag, x, _| {
        let mask = dag.get_constant(0x30, EVT::I32);
        let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        let zero = dag.get_constant(0, EVT::I32);
        dag.get_setcc(EVT::I8, and, zero, CondCode::SetEq)
    });
    assert_ne!(dag.opcode(dag.operand(root, 1)), x86isd::BT);
}
