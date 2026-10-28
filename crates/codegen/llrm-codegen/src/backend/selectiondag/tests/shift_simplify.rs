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
        ISD::BSwap => arg(0).swap_bytes(),
        ISD::BitReverse => arg(0).reverse_bits(),
        ISD::Shl => arg(0).checked_shl(arg(1)).unwrap_or(0),
        ISD::Srl => arg(0).checked_shr(arg(1)).unwrap_or(0),
        ISD::Sra => ((arg(0) as i32) >> arg(1).min(31)) as u32,
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

fn shift(
    dag: &mut SelectionDAG,
    op: ISD,
    x: SDValue,
    by: u128,
) -> SDValue {
    let amount = dag.get_constant(by, EVT::I8);
    dag.get_node(op, EVT::I32, &[x, amount])
}

/// `(shl (shl x, 3), 4)` stayed two shifts: one instruction more than
/// `shl x, 7`. Same for `srl` and `sra`.
#[test]
fn two_shifts_of_one_kind_are_one_shift_by_the_sum() {
    for (op, a, b, sum) in [(ISD::Shl, 3, 4, 7), (ISD::Srl, 5, 6, 11), (ISD::Sra, 2, 9, 11)] {
        let (dag, root) = combined(|dag, x| {
            let inner = shift(dag, op, x, a);
            shift(dag, op, inner, b)
        });
        assert_eq!(dag.constant(dag.operand(root, 1)), Some(sum), "{op:?}");
        assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register, "{op:?}");
    }
}

/// Shifting out every bit is 0 (`shl`, `srl`); `sra` saturates at the sign.
#[test]
fn shifts_that_add_up_past_the_width_clear_or_fill() {
    for op in [ISD::Shl, ISD::Srl] {
        let (dag, root) = combined(|dag, x| {
            let inner = shift(dag, op, x, 20);
            shift(dag, op, inner, 20)
        });
        assert_eq!(dag.constant(root), Some(0), "{op:?}");
    }
    let (dag, root) = combined(|dag, x| {
        let inner = shift(dag, ISD::Sra, x, 20);
        shift(dag, ISD::Sra, inner, 20)
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(31));
}

/// `(srl (and x, 0xF), 4)` has no bit left: 0.
#[test]
fn a_shift_of_known_zero_bits_is_zero() {
    let (dag, root) = combined(|dag, x| {
        let mask = dag.get_constant(0xF, EVT::I32);
        let low = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        shift(dag, ISD::Srl, low, 4)
    });
    assert_eq!(dag.constant(root), Some(0));
}

/// `visitBSWAP` did not exist: `bswap (x & 0xFF000000)`, what the bitreverse
/// expansion builds for a top byte, stayed a `bswap` (+19 B in x_popcount).
#[test]
fn a_bswap_of_one_byte_is_a_shift_and_of_a_shift_is_the_inverse_shift() {
    let (dag, root) = combined(|dag, x| {
        let mask = dag.get_constant(0xFF00_0000, EVT::I32);
        let top = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        dag.get_node(ISD::BSwap, EVT::I32, &[top])
    });
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 1))), (ISD::Srl, Some(24)));
    let (dag, root) = combined(|dag, x| {
        let up = shift(dag, ISD::Shl, x, 8);
        dag.get_node(ISD::BSwap, EVT::I32, &[up])
    });
    assert_eq!((dag.opcode(root), dag.opcode(dag.operand(root, 0))), (ISD::Srl, ISD::BSwap));
}

/// `visitBITREVERSE` did not exist: `bitreverse (bitreverse x)` and the shift
/// of a reversed value reversed back stayed three nodes (the pin: `x`,
/// `shl x, y` / `lshr x, y`).
#[test]
fn a_bitreverse_of_a_bitreverse_is_the_value_and_of_a_reversed_shift_the_opposite_shift() {
    let (dag, root) = combined(|dag, x| {
        let once = dag.get_node(ISD::BitReverse, EVT::I32, &[x]);
        dag.get_node(ISD::BitReverse, EVT::I32, &[once])
    });
    assert_eq!(dag.opcode(root), ISD::Register);
    for (op, want) in [(ISD::Srl, ISD::Shl), (ISD::Shl, ISD::Srl)] {
        let (dag, root) = combined(|dag, x| {
            let once = dag.get_node(ISD::BitReverse, EVT::I32, &[x]);
            let moved = shift(dag, op, once, 3);
            dag.get_node(ISD::BitReverse, EVT::I32, &[moved])
        });
        assert_eq!((dag.opcode(root), dag.opcode(dag.operand(root, 0))), (want, ISD::Register));
    }
}
