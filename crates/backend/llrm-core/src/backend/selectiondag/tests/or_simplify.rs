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
        ISD::Srl => arg(0) >> arg(1),
        ISD::Shl => arg(0) << arg(1),
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

/// `(or (or x, 0xF0), 0x0F)` stayed two `or`s: one instruction and a constant
/// more than `or x, 0xFF`.
#[test]
fn two_or_masks_in_a_row_are_one_mask() {
    let (dag, root) = combined(|dag, x| {
        let (a, b) = (k(dag, 0xF0), k(dag, 0x0F));
        let inner = dag.get_node(ISD::Or, EVT::I32, &[x, a]);
        dag.get_node(ISD::Or, EVT::I32, &[inner, b])
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xFF));
}

/// `(or (and x, 0xFF), 0xFF)` is the constant `0xFF` (the `and` has no bit
/// the constant lacks); `(or x, -1)` is `-1`; a constant on the left moves
/// right; `x | x` is `x`.
#[test]
fn an_or_that_sets_every_bit_is_the_constant() {
    let (dag, root) = combined(|dag, x| {
        let a = k(dag, 0xFF);
        let masked = dag.get_node(ISD::And, EVT::I32, &[x, a]);
        dag.get_node(ISD::Or, EVT::I32, &[masked, a])
    });
    assert_eq!(dag.constant(root), Some(0xFF));
    let (dag, root) = combined(|dag, x| {
        let ones = k(dag, 0xFFFF_FFFF);
        dag.get_node(ISD::Or, EVT::I32, &[x, ones])
    });
    assert_eq!(dag.constant(root), Some(0xFFFF_FFFF));
    let (dag, root) = combined(|dag, x| {
        let c = k(dag, 0xF0);
        dag.get_node(ISD::Or, EVT::I32, &[c, x])
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xF0));
    let (dag, root) = combined(|dag, x| dag.get_node(ISD::Or, EVT::I32, &[x, x]));
    assert_eq!(dag.opcode(root), ISD::Register);
}

/// `(or (and x, 0x0FF0), 0x00F0)` shares bits with the mask: it is
/// `(and (or x, 0xF0), 0x0FF0)`, one constant fewer in the chain.
#[test]
fn an_or_of_an_overlapping_mask_is_an_and_of_an_or() {
    let (dag, root) = combined(|dag, x| {
        let (a, b) = (k(dag, 0x0FF0), k(dag, 0x00F0));
        let masked = dag.get_node(ISD::And, EVT::I32, &[x, a]);
        dag.get_node(ISD::Or, EVT::I32, &[masked, b])
    });
    assert_eq!(dag.opcode(root), ISD::And);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Or);
}

/// `(or (and x, 0xFFFF), (shl (srl x, 16), 16))` is `x` split and put back
/// together; it stayed four nodes (a far pointer made from a call's dword:
/// `mov; shr; shl; movzx; add` where `x` itself was meant).
#[test]
fn a_dword_split_into_halves_and_joined_is_the_dword() {
    let (dag, root) = combined(|dag, x| {
        let (low, sixteen) = (k(dag, 0xFFFF), dag.get_constant(16, EVT::I8));
        let lo = dag.get_node(ISD::And, EVT::I32, &[x, low]);
        let hi = dag.get_node(ISD::Srl, EVT::I32, &[x, sixteen]);
        let back = dag.get_node(ISD::Shl, EVT::I32, &[hi, sixteen]);
        dag.get_node(ISD::Or, EVT::I32, &[lo, back])
    });
    assert_eq!(dag.opcode(root), ISD::Register, "the or of the halves is x");
}
