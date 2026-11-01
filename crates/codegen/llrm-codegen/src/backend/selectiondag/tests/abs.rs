//! `visitABS`: `abs (abs x)` and `abs` of a value with a clear sign bit stayed
//! a second `abs` (a `neg`, `cmov` pair on x86).

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

#[test]
fn an_abs_of_an_abs_or_of_a_non_negative_value_is_the_operand() {
    let (dag, root) = combined(|dag, x| {
        let inner = dag.get_node(ISD::Abs, EVT::I32, &[x]);
        dag.get_node(ISD::Abs, EVT::I32, &[inner])
    });
    assert_eq!(dag.opcode(root), ISD::Abs);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
    let (dag, root) = combined(|dag, x| {
        let mask = dag.get_constant(0xFF, EVT::I32);
        let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        dag.get_node(ISD::Abs, EVT::I32, &[and])
    });
    assert_eq!(dag.opcode(root), ISD::And);
    let (dag, root) = combined(|dag, x| dag.get_node(ISD::Abs, EVT::I32, &[x]));
    assert_eq!(dag.opcode(root), ISD::Abs, "an unknown sign keeps its abs");
}
