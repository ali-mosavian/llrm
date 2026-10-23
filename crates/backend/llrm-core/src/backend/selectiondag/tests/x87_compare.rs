use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;

const F64: EVT = EVT::Simple(MVT::F64);

fn legalized(
    build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue
) -> (SelectionDAG, SDValue, [SDValue; 2]) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (dag.get_register(F64, 5), dag.get_register(F64, 6));
    let root = build(&mut dag, a, b);
    dag.set_root(root);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false }).run().unwrap();
    let root = dag.get_root();
    (dag, root, [a, b])
}

fn code(
    dag: &SelectionDAG,
    node: SDValue,
) -> u128 {
    dag.constant(dag.operand(node, 0)).expect("a condition code")
}

/// `a < b` (ordered) on doubles read no flags: it must be `fcmp b, a`, then
/// "above" (the flags are CF=1 only for a below, and unordered sets CF).
#[test]
fn an_ordered_less_than_compares_swapped_and_tests_above() {
    let (dag, root, [a, b]) = legalized(|dag, a, b| dag.get_setcc(EVT::I8, a, b, CondCode::SetOLt));
    assert_eq!((dag.opcode(root), code(&dag, root)), (x86isd::SETCC, cond::A));
    let flags = dag.operand(root, 1);
    assert_eq!((dag.opcode(flags), dag.operand(flags, 0), dag.operand(flags, 1)), (x86isd::FCMP, b, a));
}

/// `a == b` (ordered) is equal and not unordered; a single `e` is true for
/// NaN operands.
#[test]
fn an_ordered_equal_is_two_tests_anded() {
    let (dag, root, _) = legalized(|dag, a, b| dag.get_setcc(EVT::I8, a, b, CondCode::SetOEq));
    assert_eq!(dag.opcode(root), ISD::And);
    let codes = [code(&dag, dag.operand(root, 0)), code(&dag, dag.operand(root, 1))];
    assert_eq!(codes, [cond::E, cond::NP]);
}

/// A double select on a double compare is a `cmov` of the compare's flags.
#[test]
fn a_float_select_on_a_float_compare_is_a_cmov_of_fcmp() {
    let (dag, root, _) = legalized(|dag, a, b| {
        let cond = dag.get_setcc(EVT::I8, a, b, CondCode::SetOGt);
        dag.get_select(F64, cond, a, b)
    });
    assert_eq!(dag.opcode(root), x86isd::CMOV);
    let flags = dag.operand(root, 3);
    assert_eq!((dag.opcode(flags), dag.constant(dag.operand(root, 2))), (x86isd::FCMP, Some(cond::A)));
}

/// A branch on a double compare is a conditional branch on its flags.
#[test]
fn a_branch_on_a_float_compare_is_a_brcond_of_fcmp() {
    let (dag, root, _) = legalized(|dag, a, b| {
        let cond = dag.get_setcc(EVT::I8, a, b, CondCode::SetOGe);
        let entry = dag.entry_node();
        let dest = dag.get_basic_block(1);
        dag.get_node(ISD::BrCond, EVT::OTHER, &[entry, cond, dest])
    });
    assert_eq!(dag.opcode(root), x86isd::BRCOND);
    assert_eq!((dag.constant(dag.operand(root, 2)), dag.opcode(dag.operand(root, 3))), (Some(cond::AE), x86isd::FCMP));
}
