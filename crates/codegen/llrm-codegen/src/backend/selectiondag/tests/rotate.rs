//! `visitRotate`: rotates by zero, a multiple of the width, or an amount past
//! it, `rotl i16 x, 8`, and two constant rotates stayed as written.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

const SAMPLES: [u32; 6] = [0, 1, 0x80, 0x1234, 0x8001, 0xFFFF];

fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u32,
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    let rot = |left: bool| {
        let by = arg(1) % 16;
        let by = if left { by } else { (16 - by) % 16 };
        ((arg(0) << by) | (arg(0) >> ((16 - by) % 16).max(if by == 0 { 16 } else { 0 }))) & 0xFFFF
    };
    match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::Rotl => rot(true),
        ISD::Rotr => rot(false),
        ISD::BSwap => ((arg(0) & 0xFF) << 8) | (arg(0) >> 8),
        other => panic!("unexpected {other:?}"),
    }
}

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let x = dag.get_register(EVT::I16, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let before: Vec<u32> = SAMPLES.iter().map(|&s| eval(&dag, root, s)).collect();
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let after: Vec<u32> = SAMPLES.iter().map(|&s| eval(&dag, root, s)).collect();
    assert_eq!(before, after, "the combine changed the value");
    (dag, root)
}

fn rot(
    dag: &mut SelectionDAG,
    op: ISD,
    x: SDValue,
    by: u128,
) -> SDValue {
    let by = dag.get_constant(by, EVT::I8);
    dag.get_node(op, EVT::I16, &[x, by])
}

/// A rotate by 0, 16 or 32 was kept as a `rol` instead of the operand.
#[test]
fn a_rotate_by_a_multiple_of_the_width_is_the_value() {
    for by in [0, 16, 32] {
        let (dag, root) = combined(|dag, x| rot(dag, ISD::Rotl, x, by));
        assert_eq!(dag.opcode(root), ISD::Register, "by {by}");
    }
}

/// `rotl i16 x, 8` stayed a `rol ax, 8`; the pin selects `xchg al, ah`.
#[test]
fn a_word_rotate_by_eight_is_a_byte_swap() {
    let (dag, root) = combined(|dag, x| rot(dag, ISD::Rotr, x, 8));
    assert_eq!(dag.opcode(root), ISD::BSwap);
}

/// An amount past the width wrapped at run time only; nested constant
/// rotates stayed two instructions.
#[test]
fn a_big_amount_wraps_and_nested_rotates_are_one() {
    let (dag, root) = combined(|dag, x| rot(dag, ISD::Rotl, x, 19));
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(3));
    let (dag, root) = combined(|dag, x| {
        let inner = rot(dag, ISD::Rotl, x, 3);
        rot(dag, ISD::Rotr, inner, 5)
    });
    assert_eq!(dag.opcode(root), ISD::Rotr);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(2));
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
}
