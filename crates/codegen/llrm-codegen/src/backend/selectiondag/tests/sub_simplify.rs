use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// `x - 1` stayed a `sub`: the matcher's `dec` pattern reads `add x, -1`, so
/// 130 `sub r, 1` of QCport were `sub` + a post-RA rewrite (w-1's
/// `increments`), and `x - 4` never met the add-shaped folds.
#[test]
fn a_subtraction_of_a_constant_is_an_addition_of_its_negation() {
    for (c, negated) in [(1u128, 0xFFFF_FFFFu128), (4, 0xFFFF_FFFC), (0x8000_0000, 0x8000_0000)] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let k = dag.get_constant(c, EVT::I32);
        let root = dag.get_node(ISD::Sub, EVT::I32, &[x, k]);
        dag.set_root(root);
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::Add, "x - {c:#x}");
        assert_eq!(dag.constant(dag.operand(root, 1)), Some(negated), "x - {c:#x}");
    }
}

/// `x - x` is 0 whatever `x` is.
#[test]
fn a_value_minus_itself_is_zero() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let y = dag.get_node(ISD::Add, EVT::I32, &[x, x]);
    let root = dag.get_node(ISD::Sub, EVT::I32, &[y, y]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.constant(dag.get_root()), Some(0));
}
