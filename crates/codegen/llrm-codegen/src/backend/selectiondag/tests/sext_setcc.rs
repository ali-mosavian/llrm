use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner, NoCombines};
use super::*;

/// AArch64's answer: a select of two constants stays a select (`csel`).
struct KeepsConstantSelects;

impl CombineHooks for KeepsConstantSelects {
    fn sext_setcc_as_select(&self) -> bool {
        true
    }

    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }
}

fn sext_of_compare(dag: &mut SelectionDAG) {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetNe);
    let wide = dag.get_node(ISD::SignExtend, EVT::I32, &[test]);
    dag.set_root(wide);
}

/// `sext (setcc a, b)` stayed an extend of a flag, which arm64 made as `cset`
/// then `sbfm` (3 instructions with the `ands` before it); the pin makes it
/// `select (setcc a, b), -1, 0` and the `csetm` of the select (2).
#[test]
fn a_scalar_sext_of_a_compare_is_a_select_of_minus_one_where_selects_are_kept() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    sext_of_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut KeepsConstantSelects, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Select);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(u128::from(u32::MAX)));
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(0));
}

/// Where a target turns a select of constants into the extend, the extend
/// stays: the select would only be turned back.
#[test]
fn a_target_that_converts_selects_of_constants_keeps_the_sext() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    sext_of_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert_ne!(dag.opcode(dag.get_root()), ISD::Select);
}
