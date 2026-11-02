//! The m68k custom lowering of conditions on hand-built DAGs, against the pin's
//! legalized DAG (`llc -mtriple=m68k-linux-gnu -O2 -debug-only=isel`).

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{DagConfig, SDValue, SelectionDAG};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::lowering::{M68kLowering, cond, m68kisd, target_lowering};

fn legalize(dag: &mut SelectionDAG) -> SDValue {
    let tli = target_lowering(Cpu::M68000);
    DagLegalizer::new(dag, &tli, &mut M68kLowering::default()).run().expect("legalizes");
    dag.get_root()
}

/// The pin turns `select (setlt a, b), x, y` into `CMOV y, x, 13, (CMP b, a)`:
/// the false value first, `COND_LT` is 13 and the compare takes its operands
/// reversed. Before the lowering there was no m68k table of actions at all and
/// the select stayed a generic node.
#[test]
fn select_on_a_signed_compare_is_cmov_on_cmp() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b, x, y] = [1, 2, 3, 4].map(|n| dag.get_register(EVT::I32, n));
    let test = dag.get_setcc(EVT::I8, a, b, CondCode::SetLt);
    let select = dag.get_node(ISD::Select, EVT::I32, &[test, x, y]);
    dag.set_root(select);

    let root = legalize(&mut dag);

    assert_eq!(dag.opcode(root), m68kisd::CMOV);
    assert_eq!([dag.operand(root, 0), dag.operand(root, 1)], [y, x]);
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(cond::LT));
    let cmp = dag.operand(root, 3);
    assert_eq!(dag.opcode(cmp), m68kisd::CMP);
    assert_eq!([dag.operand(cmp, 0), dag.operand(cmp, 1)], [b, a]);
}

/// `x > -1` is `PL` on a compare with zero (`TranslateM68kCC`), not `GT`
/// against -1.
#[test]
fn greater_than_minus_one_is_plus_on_a_test_against_zero() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = dag.get_register(EVT::I32, 1);
    let minus_one = dag.get_constant(u128::MAX, EVT::I32);
    let test = dag.get_setcc(EVT::I8, a, minus_one, CondCode::SetGt);
    dag.set_root(test);

    let root = legalize(&mut dag);

    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (m68kisd::SETCC, Some(cond::PL)));
    let cmp = dag.operand(root, 1);
    assert_eq!(dag.opcode(cmp), m68kisd::CMP);
    assert!(dag.is_null_constant(dag.operand(cmp, 0)));
}

/// The 68000 has no 32-bit multiply: the action table says `LibCall`, the
/// 68020's says `Legal`.
#[test]
fn a_32_bit_multiply_is_legal_only_on_the_68020() {
    use llrm_codegen::backend::selectiondag::Action;
    let (m68000, m68020) = (target_lowering(Cpu::M68000), target_lowering(Cpu::M68020));
    assert_eq!(m68000.operation_action(ISD::Mul, EVT::I32), Action::LibCall);
    assert_eq!(m68020.operation_action(ISD::Mul, EVT::I32), Action::Legal);
}
