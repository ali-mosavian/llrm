use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// The value of `v` when register `5 + i` holds bit `i` of `bits`
/// (i1 registers) and register 20 holds 7, register 21 holds 9 (i32).
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    bits: u32,
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), bits);
    let width = dag.value_type(v).size_in_bits();
    let out = match dag.opcode(v) {
        ISD::Register => match dag.node(v).data {
            NodeData::Register(r) if r >= 20 => r - 13,
            NodeData::Register(r) => (bits >> (r - 5)) & 1,
            _ => panic!("register"),
        },
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        ISD::Freeze => arg(0),
        ISD::Select => {
            if arg(0) != 0 {
                arg(1)
            } else {
                arg(2)
            }
        }
        other => panic!("unexpected {other:?}"),
    };
    out & if width >= 32 { u32::MAX } else { (1 << width) - 1 }
}

fn combined(build: impl FnOnce(&mut SelectionDAG, [SDValue; 3], [SDValue; 2]) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let c = [5, 6, 7].map(|r| dag.get_register(EVT::I1, r));
    let a = [20, 21].map(|r| dag.get_register(EVT::I32, r));
    let root = build(&mut dag, c, a);
    dag.set_root(root);
    let original: Vec<u32> = (0..8).map(|b| eval(&dag, root, b)).collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let folded: Vec<u32> = (0..8).map(|b| eval(&dag, root, b)).collect();
    assert_eq!(original, folded, "the fold changed the value");
    (dag, root)
}

/// `select (not c), a, b` kept its xor, so a branch on it tested a
/// materialized flipped boolean.
#[test]
fn a_select_on_a_flipped_condition_swaps_its_arms() {
    let (dag, root) = combined(|dag, c, a| {
        let one = dag.get_constant(1, EVT::I1);
        let not = dag.get_node(ISD::Xor, EVT::I1, &[c[0], one]);
        dag.get_select(EVT::I32, not, a[0], a[1])
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
    assert_eq!(dag.node(dag.operand(root, 1)).data, NodeData::Register(21));
}

/// An `i1` select against a constant arm is `and`/`or` of the condition.
#[test]
fn an_i1_select_with_a_constant_arm_is_logic() {
    for (then, other) in [(1, 0), (0, 0), (1, 1), (0, 1)] {
        let (dag, root) = combined(|dag, c, _| {
            let (t, o) = (dag.get_constant(then, EVT::I1), dag.get_constant(other, EVT::I1));
            let t = if then == 1 && other == 1 { c[1] } else { t };
            dag.get_select(EVT::I1, c[0], t, o)
        });
        assert_ne!(dag.opcode(root), ISD::Select, "{then} {other}");
    }
}

/// `select c0, (select c1, x, y), y` is one select on `c0 & c1`;
/// `select c0, x, (select c1, x, y)` one on `c0 | c1`.
#[test]
fn nested_selects_on_one_arm_merge_their_conditions() {
    let (dag, root) = combined(|dag, c, a| {
        let inner = dag.get_select(EVT::I32, c[1], a[0], a[1]);
        dag.get_select(EVT::I32, c[0], inner, a[1])
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::And);
    let (dag, root) = combined(|dag, c, a| {
        let inner = dag.get_select(EVT::I32, c[1], a[0], a[1]);
        dag.get_select(EVT::I32, c[0], a[0], inner)
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Or);
}
