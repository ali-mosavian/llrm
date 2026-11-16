//! `or x, 0` and `and x, -1` of a vector were kept: a `select` of a vector by
//! its mask against zero expanded to `(m & a) | (~m & 0)` and arm64 printed
//! `movi` + `and` + `orr` where the pin has the `and` alone.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn combined(
    op: ISD,
    lane: u128,
) -> (SelectionDAG, SDValue, SDValue) {
    let v2i64 = EVT::Simple(crate::backend::selectiondag::MVT::V2I64);
    let mut dag = dag();
    let x = dag.get_register(v2i64, 9);
    let c = dag.get_constant(lane, EVT::I64);
    let splat = dag.get_build_vector(v2i64, &[c, c]);
    let root = dag.get_node(op, v2i64, &[x, splat]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    (dag, x, splat)
}

#[test]
fn or_of_a_zero_vector_is_the_vector_and_of_an_all_ones_vector_too() {
    let (dag, x, _) = combined(ISD::Or, 0);
    assert_eq!(dag.get_root(), x);
    let (dag, x, _) = combined(ISD::And, u128::from(u64::MAX));
    assert_eq!(dag.get_root(), x);
}

#[test]
fn or_of_an_all_ones_vector_and_and_of_a_zero_vector_are_that_vector() {
    let (dag, _, ones) = combined(ISD::Or, u128::from(u64::MAX));
    assert_eq!(dag.get_root(), ones);
    let (dag, _, zero) = combined(ISD::And, 0);
    assert_eq!(dag.get_root(), zero);
}
