use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// The value of `v` when register 5 holds `x`, 6 `y`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (x, y): (u32, u32),
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), (x, y));
    match dag.opcode(v) {
        ISD::Register => match dag.node(v).data {
            NodeData::Register(5) => x,
            _ => y,
        },
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => (arg(0) ^ arg(1)) & 1,
        ISD::Freeze => arg(0),
        ISD::SetCC => {
            let NodeData::CondCode(cc) = dag.node(dag.operand(v, 2)).data else { panic!("a code") };
            u32::from(match cc {
                CondCode::SetEq => arg(0) == arg(1),
                CondCode::SetNe => arg(0) != arg(1),
                CondCode::SetULt => arg(0) < arg(1),
                CondCode::SetUGe => arg(0) >= arg(1),
                CondCode::SetUGt => arg(0) > arg(1),
                CondCode::SetULe => arg(0) <= arg(1),
                other => panic!("code {other:?}"),
            })
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn has(
    dag: &SelectionDAG,
    v: SDValue,
    op: ISD,
) -> bool {
    dag.opcode(v) == op || (0..dag.node(v).ops.len()).any(|i| has(dag, dag.operand(v, i), op))
}

/// `not (x == y || y < 3)` stayed an `or`, a `xor 1`, a `test 1` and a branch
/// (the queens tail `or dl, cl; xor dl, 1; test dl, 1; je`): it is
/// `x != y && y >= 3`, two compares and an `and`, no `xor`.
#[test]
fn the_not_of_an_or_of_compares_is_the_and_of_the_inverted_compares() {
    for (outer, inner) in [(ISD::Or, ISD::And), (ISD::And, ISD::Or)] {
        let mut dag = dag();
        let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
        let first = dag.get_setcc(EVT::I1, x, y, CondCode::SetEq);
        let three = dag.get_constant(3, EVT::I32);
        let second = dag.get_setcc(EVT::I1, y, three, CondCode::SetULt);
        let join = dag.get_node(outer, EVT::I1, &[first, second]);
        let one = dag.get_constant(1, EVT::I1);
        let root = dag.get_node(ISD::Xor, EVT::I1, &[join, one]);
        dag.set_root(root);
        let pairs = || [0u32, 1, 2, 7].into_iter().flat_map(|a| [0u32, 1, 3, 7].into_iter().map(move |b| (a, b)));
        let before: Vec<u32> = pairs().map(|p| eval(&dag, root, p)).collect();
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
        let root = dag.get_root();
        let after: Vec<u32> = pairs().map(|p| eval(&dag, root, p)).collect();
        assert_eq!(before, after, "{outer:?}: the fold changed the value");
        assert_eq!(dag.opcode(root), inner, "{outer:?}");
        assert!(!has(&dag, root, ISD::Xor), "{outer:?}: no xor left");
    }
}

/// `select a, true, b` of compares is `or a, freeze(b)`; the freeze of a
/// compare of registers is the compare (nothing there can be undef), so the
/// `or` of two compares carries no `freeze` node and the De Morgan fold sees
/// the compares (queens: `x == 0 || y == 1 || z == 2` kept a `xor 1; test 1`).
#[test]
fn a_freeze_of_a_compare_of_registers_is_dropped() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let first = dag.get_setcc(EVT::I1, x, y, CondCode::SetEq);
    let three = dag.get_constant(3, EVT::I32);
    let second = dag.get_setcc(EVT::I1, y, three, CondCode::SetULt);
    let one = dag.get_constant(1, EVT::I1);
    let select = dag.get_select(EVT::I1, first, one, second);
    let root = dag.get_node(ISD::Xor, EVT::I1, &[select, one]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert!(!has(&dag, root, ISD::Freeze), "a compare of registers is not undef");
    assert!(!has(&dag, root, ISD::Xor), "the not folded into the compares");
}

/// A node the combiner replaced still read its operands, so `hasOneUse` was
/// false for them: `x + 0` is `x`, yet the dead `add` kept a second use on
/// `x == y` and the not of that compare was never folded into it.
#[test]
fn a_replaced_node_no_longer_counts_as_a_user() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let compare = dag.get_setcc(EVT::I1, x, y, CondCode::SetEq);
    let zero = dag.get_constant(0, EVT::I1);
    let dead = dag.get_node(ISD::Or, EVT::I1, &[compare, zero]);
    let one = dag.get_constant(1, EVT::I1);
    let root = dag.get_node(ISD::Xor, EVT::I1, &[dead, one]);
    dag.set_root(root);
    assert_eq!(dag.use_count(compare.node), 1);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SetCC, "not (x == y) is x != y");
    assert_eq!(dag.use_count(compare.node), 0, "the replaced compare is unused, not a second user");
}
