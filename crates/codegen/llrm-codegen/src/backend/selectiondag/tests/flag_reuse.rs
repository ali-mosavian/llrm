use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;

/// `if (--n != 0)` with `n - 1` also live out: the pin's `dec` sets the flags
/// the branch reads; ours made a `test` of the result as well (`pin 1 ours
/// 2 TEST32rr`, QGLFACEFETCH of the m32 shadow).
fn counted(
    code: CondCode,
    second_user: bool,
) -> (SelectionDAG, SDValue, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let minus_one = dag.get_constant(0xFFFF_FFFF, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, minus_one]);
    let (zero, entry, dest) = (dag.get_constant(0, EVT::I32), dag.entry_node(), dag.get_basic_block(1));
    let mut chain = dag.get_copy_to_reg(entry, 9, sum);
    if second_user {
        let squared = dag.get_node(ISD::Mul, EVT::I32, &[sum, sum]);
        chain = dag.get_copy_to_reg(chain, 10, squared);
    }
    let test = dag.get_setcc(EVT::I8, sum, zero, code);
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, test, dest]);
    dag.set_root(branch);
    DagLegalizer::new(
        &mut dag,
        &tli,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
    )
    .run()
    .unwrap();
    let root = dag.get_root();
    (dag, root, x)
}

#[test]
fn a_zero_test_of_a_live_out_sum_reads_the_sums_flags() {
    let (dag, root, _) = counted(CondCode::SetNe, false);
    assert_eq!(dag.opcode(root), x86isd::BRCOND);
    let flags = dag.operand(root, 3);
    assert_eq!((dag.opcode(flags), flags.res), (x86isd::ADD, 1), "the branch reads the add's flags");
    // The register copy reads the same node's value.
    let copy = dag.operand(root, 0);
    assert_eq!(dag.operand(copy, 2), SDValue { node: flags.node, res: 0 });
}

/// The carry flag a `test` clears is not the add's: an unsigned compare against
/// zero stays a `cmp`.
#[test]
fn a_condition_that_reads_the_carry_keeps_the_test() {
    let (dag, root, _) = counted(CondCode::SetUGt, false);
    assert_eq!(dag.opcode(dag.operand(root, 3)), x86isd::CMP);
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(cond::A));
}

/// A sum read by another operation is not made flag-setting (the other
/// reader would need its value from a node with two results).
#[test]
fn a_sum_with_another_reader_keeps_the_test() {
    let (dag, root, _) = counted(CondCode::SetNe, true);
    assert_eq!(dag.opcode(dag.operand(root, 3)), x86isd::CMP);
}
