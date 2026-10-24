use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 0x30, 0xF0, 0xFF, 0x0FF0, 0x8000_0000, 0xFFFF_FFFF];

/// The value of `v` when register 5 holds `x`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u32,
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        other => panic!("unexpected {other:?}"),
    }
}

/// `build(x)` combined, with the value of the original and of the folded DAG
/// checked for every sample.
fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let original: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let folded: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    assert_eq!(original, folded, "the fold changed the value");
    (dag, root)
}

fn k(
    dag: &mut SelectionDAG,
    value: u128,
) -> SDValue {
    dag.get_constant(value, EVT::I32)
}

/// `(and (and x, 0xFF0), 0x0FF)` stayed two `and`s: two instructions and a
/// mask register for what is `and x, 0x0F0`.
#[test]
fn two_masks_in_a_row_are_one_mask() {
    let (dag, root) = combined(|dag, x| {
        let (a, b) = (k(dag, 0xFF0), k(dag, 0x0FF));
        let inner = dag.get_node(ISD::And, EVT::I32, &[x, a]);
        dag.get_node(ISD::And, EVT::I32, &[inner, b])
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0x0F0));
}

/// `(and (or x, 0xF0), 0x30)` is the constant `0x30`: every tested bit is set
/// by the `or`.
#[test]
fn an_and_inside_the_bits_an_or_sets_is_the_mask() {
    let (dag, root) = combined(|dag, x| {
        let (c, d) = (k(dag, 0xF0), k(dag, 0x30));
        let or = dag.get_node(ISD::Or, EVT::I32, &[x, c]);
        dag.get_node(ISD::And, EVT::I32, &[or, d])
    });
    assert_eq!(dag.constant(root), Some(0x30));
}

/// `x & ~x` is 0.
#[test]
fn a_value_and_its_complement_is_zero() {
    let (dag, root) = combined(|dag, x| {
        let ones = k(dag, 0xFFFF_FFFF);
        let not = dag.get_node(ISD::Xor, EVT::I32, &[x, ones]);
        dag.get_node(ISD::And, EVT::I32, &[not, x])
    });
    assert_eq!(dag.constant(root), Some(0));
}

/// A constant on the left, and a mask that selects only known-zero bits
/// (`(x | 0xF0) & 0` aside, `((x & 0xF0) & 0x0F)`): constant right, zero.
#[test]
fn a_constant_moves_right_and_a_mask_of_known_zero_bits_is_zero() {
    let (dag, root) = combined(|dag, x| {
        let c = k(dag, 0xF0);
        dag.get_node(ISD::And, EVT::I32, &[c, x])
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xF0));
    let (dag, root) = combined(|dag, x| {
        let (a, b) = (k(dag, 0xF0), k(dag, 0x0F));
        let hi = dag.get_node(ISD::And, EVT::I32, &[x, a]);
        dag.get_node(ISD::And, EVT::I32, &[hi, b])
    });
    assert_eq!(dag.constant(root), Some(0));
}
