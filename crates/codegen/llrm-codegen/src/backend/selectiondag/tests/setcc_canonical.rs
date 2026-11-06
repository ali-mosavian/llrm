use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner, NoCombines};
use super::*;

/// A target that takes the canonical compares (AArch64, M68k).
struct Canonical;

impl CombineHooks for Canonical {
    fn canonicalize_ge_le(&self) -> bool {
        true
    }
}

/// `SimplifySetCC` makes `x >= c` into `x > c - 1` and `x <= c` into
/// `x < c + 1` before operations are legal; `x >= 0` stayed `setge` and an
/// AArch64 `x >= 0` select was `cmp x, #0`, not the pin's `cmn x, #1`.
#[test]
fn greater_equal_and_less_equal_become_strict_compares_on_the_next_constant() {
    for (cond, constant, want_cond, want_constant) in [
        (CondCode::SetGe, 0u128, CondCode::SetGt, 0xFFFF_FFFFu128),
        (CondCode::SetGe, 5, CondCode::SetGt, 4),
        (CondCode::SetUGe, 2, CondCode::SetUGt, 1),
        (CondCode::SetLe, 5, CondCode::SetLt, 6),
        (CondCode::SetULe, 7, CondCode::SetULt, 8),
    ] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let c = dag.get_constant(constant, EVT::I32);
        let root = dag.get_setcc(EVT::I1, x, c, cond);
        dag.set_root(root);
        DagCombiner::new(&mut dag, &mut Canonical, CombineLevel::BeforeLegalizeTypes).run();
        let root = dag.get_root();
        assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(want_cond), "{cond:?} {constant}");
        assert_eq!(dag.constant(dag.operand(root, 1)), Some(want_constant), "{cond:?} {constant}");
    }
}

/// `x >=u 0` and `x <=s MAX` are true.
#[test]
fn a_compare_against_the_extreme_is_true() {
    for (cond, constant) in [(CondCode::SetUGe, 0u128), (CondCode::SetLe, 0x7FFF_FFFF)] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let c = dag.get_constant(constant, EVT::I32);
        let root = dag.get_setcc(EVT::I1, x, c, cond);
        dag.set_root(root);
        DagCombiner::new(&mut dag, &mut Canonical, CombineLevel::BeforeLegalizeTypes).run();
        assert_eq!(dag.constant(dag.get_root()), Some(1), "{cond:?} {constant}");
    }
}

/// The rewrite moved x86's range check `cmp n; jae` to `cmp n - 1; ja`, and 35
/// of the 424 judged rows (`switch` in every language, +1018 instructions)
/// went above their baseline. A target that does not ask keeps `x >= c`.
#[test]
fn a_target_that_does_not_ask_keeps_greater_equal() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let c = dag.get_constant(12, EVT::I32);
    let root = dag.get_setcc(EVT::I1, x, c, CondCode::SetUGe);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetUGe));
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(12));
}
