//! `visitTRUNCATE` narrowed no binary operation: `trunc (and y, 31)` stayed a
//! truncate of a dword `and`, so the shift pattern did not see the mask and
//! a 64-bit shift by `i & 31` kept `and ecx, 31` (x_ll_arith: +8 instructions).

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let y = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, y);
    dag.set_root(root);
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

#[test]
fn a_truncated_masked_value_is_the_mask_of_the_truncated_value() {
    let (dag, root) = combined(|dag, y| {
        let mask = dag.get_constant(31, EVT::I32);
        let and = dag.get_node(ISD::And, EVT::I32, &[y, mask]);
        dag.get_node(ISD::Truncate, EVT::I8, &[and])
    });
    assert_eq!(dag.opcode(root), ISD::And);
    assert_eq!(dag.value_type(root), EVT::I8);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Truncate);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(31));
}
