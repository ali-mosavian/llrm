use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 7, 0xF0, 0xFF, 0x0FF0, 0x8000_0000, 0xFFFF_FFFF];

/// The value of `v` when register 5 holds `x` and register 6 holds `y`.
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
        ISD::Add => arg(0).wrapping_add(arg(1)),
        ISD::Sub => arg(0).wrapping_sub(arg(1)),
        ISD::Xor => arg(0) ^ arg(1),
        other => panic!("unexpected {other:?}"),
    }
}

/// `build(x, y)` combined, with the value of the original and of the folded
/// DAG checked for every pair of samples.
fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let root = build(&mut dag, x, y);
    dag.set_root(root);
    let values = |dag: &SelectionDAG, root| -> Vec<u32> {
        SAMPLES.iter().flat_map(|&p| SAMPLES.iter().map(move |&q| (p, q))).map(|pair| eval(dag, root, pair)).collect()
    };
    let original = values(&dag, root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(original, values(&dag, root), "the fold changed the value");
    (dag, root)
}

fn k(
    dag: &mut SelectionDAG,
    value: u128,
) -> SDValue {
    dag.get_constant(value, EVT::I32)
}

/// `(add (add x, 3), 4)` stayed two adds; `(add (sub x, 3), 4)` too.
#[test]
fn constants_merge_through_an_add_and_a_sub() {
    let (dag, root) = combined(|dag, x, _| {
        let (a, b) = (k(dag, 3), k(dag, 4));
        let inner = dag.get_node(ISD::Add, EVT::I32, &[x, a]);
        dag.get_node(ISD::Add, EVT::I32, &[inner, b])
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(7));
    let (dag, root) = combined(|dag, x, _| {
        let (a, b) = (k(dag, 3), k(dag, 4));
        let inner = dag.get_node(ISD::Sub, EVT::I32, &[x, a]);
        dag.get_node(ISD::Add, EVT::I32, &[inner, b])
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(1));
}

/// `x + ~x` is -1; a constant on the left moves right.
#[test]
fn a_value_and_its_complement_sum_to_all_ones() {
    let (dag, root) = combined(|dag, x, _| {
        let ones = k(dag, 0xFFFF_FFFF);
        let not = dag.get_node(ISD::Xor, EVT::I32, &[x, ones]);
        dag.get_node(ISD::Add, EVT::I32, &[not, x])
    });
    assert_eq!(dag.constant(root), Some(0xFFFF_FFFF));
    let (dag, root) = combined(|dag, x, _| {
        let c = k(dag, 9);
        dag.get_node(ISD::Add, EVT::I32, &[c, x])
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(9));
}

/// `y + (0 - x)` is `y - x`; `x + (b - x)` is `b`; `(a - b) + (c - a)` is
/// `c - b`.
#[test]
fn an_add_of_a_negation_or_a_sub_of_the_other_side_simplifies() {
    let (dag, root) = combined(|dag, x, y| {
        let zero = k(dag, 0);
        let neg = dag.get_node(ISD::Sub, EVT::I32, &[zero, x]);
        dag.get_node(ISD::Add, EVT::I32, &[y, neg])
    });
    assert_eq!(dag.opcode(root), ISD::Sub);
    let (dag, root) = combined(|dag, x, y| {
        let diff = dag.get_node(ISD::Sub, EVT::I32, &[y, x]);
        dag.get_node(ISD::Add, EVT::I32, &[x, diff])
    });
    assert_eq!(dag.node(root).data, NodeData::Register(6));
    let (dag, root) = combined(|dag, x, y| {
        let one = k(dag, 1);
        let ab = dag.get_node(ISD::Sub, EVT::I32, &[x, y]);
        let ca = dag.get_node(ISD::Sub, EVT::I32, &[one, x]);
        dag.get_node(ISD::Add, EVT::I32, &[ab, ca])
    });
    assert_eq!(dag.opcode(root), ISD::Sub);
    assert_eq!(dag.constant(dag.operand(root, 0)), Some(1));
}
