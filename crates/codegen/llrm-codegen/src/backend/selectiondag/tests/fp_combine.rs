use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

const F64: EVT = EVT::Simple(MVT::F64);
const F32: EVT = EVT::Simple(MVT::F32);

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
    dag.get_load(vt, chain, ptr, info)
}

fn double(
    dag: &mut SelectionDAG,
    value: f64,
) -> SDValue {
    dag.get_constant_fp(value.to_bits().into(), F64)
}

/// `root` after the generic combiner.
fn combined(
    dag: &mut SelectionDAG,
    root: SDValue,
) -> SDValue {
    dag.set_root(root);
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    dag.get_root()
}

fn bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> Option<u128> {
    match dag.node(value).data {
        NodeData::ConstFp { bits } => Some(bits),
        _ => None,
    }
}

/// `visitFADD` and `visitFMUL` did not exist: `x + -0.0` and `x * 1.0` stayed
/// as instructions.
#[test]
fn adding_minus_zero_and_multiplying_by_one_are_the_operand() {
    let mut dag = dag();
    let x = load(&mut dag, F64, 1);
    let minus_zero = double(&mut dag, -0.0);
    let add = dag.get_node(ISD::FAdd, F64, &[x, minus_zero]);
    assert_eq!(combined(&mut dag, add), x);
    let one = double(&mut dag, 1.0);
    let mul = dag.get_node(ISD::FMul, F64, &[x, one]);
    assert_eq!(combined(&mut dag, mul), x);
    // + 0.0 is not x: it turns -0.0 into +0.0 (an `fneg` user is sensitive to
    //   it)
    let zero = double(&mut dag, 0.0);
    let add = dag.get_node(ISD::FAdd, F64, &[x, zero]);
    let user = dag.get_node(ISD::FNeg, F64, &[add]);
    let root = combined(&mut dag, user);
    assert_eq!(dag.operand(root, 0), add);
}

/// Two constants were an instruction on the x87; the single's sum rounds as a
/// single (16777216 + 1 is 16777216).
#[test]
fn constants_fold_in_their_own_precision() {
    let mut dag = dag();
    let (a, b) = (double(&mut dag, 1.5), double(&mut dag, 2.25));
    let sum = dag.get_node(ISD::FAdd, F64, &[a, b]);
    assert_eq!(bits(&dag, sum), Some(3.75f64.to_bits().into()));
    let (big, one) =
        (dag.get_constant_fp(16777216f32.to_bits().into(), F32), dag.get_constant_fp(1f32.to_bits().into(), F32));
    let sum = dag.get_node(ISD::FAdd, F32, &[big, one]);
    assert_eq!(bits(&dag, sum), Some(16777216f32.to_bits().into()));
    // 0.0 / 0.0 is a NaN: left to run
    let zero = double(&mut dag, 0.0);
    let nan = dag.get_node(ISD::FDiv, F64, &[zero, zero]);
    assert_eq!(dag.opcode(nan), ISD::FDiv);
}

/// A constant on the left went to the right, so the matcher sees one shape.
#[test]
fn a_constant_goes_to_the_right_of_fadd_and_fmul() {
    let mut dag = dag();
    let x = load(&mut dag, F64, 1);
    let c = double(&mut dag, 3.0);
    for op in [ISD::FAdd, ISD::FMul] {
        let node = dag.get_node(op, F64, &[c, x]);
        let root = combined(&mut dag, node);
        assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (op, x, c));
    }
}

/// `x + (-y)` was an `fchs` and an add; it is a subtract, and `x - (-y)` an
/// add.
#[test]
fn an_fneg_operand_turns_add_into_sub_and_sub_into_add() {
    let mut dag = dag();
    let (x, y) = (load(&mut dag, F64, 1), load(&mut dag, F64, 2));
    let neg = dag.get_node(ISD::FNeg, F64, &[y]);
    let add = dag.get_node(ISD::FAdd, F64, &[x, neg]);
    let root = combined(&mut dag, add);
    assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (ISD::FSub, x, y));
    let sub = dag.get_node(ISD::FSub, F64, &[x, neg]);
    let root = combined(&mut dag, sub);
    assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (ISD::FAdd, x, y));
    let add = dag.get_node(ISD::FAdd, F64, &[neg, x]);
    let root = combined(&mut dag, add);
    assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (ISD::FSub, x, y));
}

/// `x * 2.0` was a multiply by a pool constant; it is `x + x`. `x * -1.0` is
/// `-0.0 - x`, which is `-x`.
#[test]
fn a_multiply_by_two_is_an_add_and_by_minus_one_a_negation() {
    let mut dag = dag();
    let x = load(&mut dag, F64, 1);
    let two = double(&mut dag, 2.0);
    let mul = dag.get_node(ISD::FMul, F64, &[x, two]);
    let root = combined(&mut dag, mul);
    assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (ISD::FAdd, x, x));
    let minus_one = double(&mut dag, -1.0);
    let mul = dag.get_node(ISD::FMul, F64, &[x, minus_one]);
    let root = combined(&mut dag, mul);
    assert_eq!((dag.opcode(root), dag.operand(root, 0)), (ISD::FNeg, x));
}

/// `(-a) * (-b)` is `a * b`.
#[test]
fn two_negated_operands_multiply_as_their_originals() {
    let mut dag = dag();
    let (a, b) = (load(&mut dag, F64, 1), load(&mut dag, F64, 2));
    let (na, nb) = (dag.get_node(ISD::FNeg, F64, &[a]), dag.get_node(ISD::FNeg, F64, &[b]));
    let mul = dag.get_node(ISD::FMul, F64, &[na, nb]);
    let root = combined(&mut dag, mul);
    assert_eq!((dag.opcode(root), dag.operand(root, 0), dag.operand(root, 1)), (ISD::FMul, a, b));
}

/// With `reassoc` and `nsz`, `(x * c) + x` is `x * (c + 1)`.
#[test]
fn a_reassociable_add_of_a_multiple_and_its_base_is_one_multiply() {
    let mut dag = dag();
    let x = load(&mut dag, F64, 1);
    let c = double(&mut dag, 2.5);
    let flags = SDNodeFlags::ALLOW_REASSOC.union(SDNodeFlags::NO_SIGNED_ZEROS);
    let mul = dag.get_node_flags(ISD::FMul, F64, &[x, c], flags);
    let add = dag.get_node_flags(ISD::FAdd, F64, &[mul, x], flags);
    let root = combined(&mut dag, add);
    assert_eq!((dag.opcode(root), dag.operand(root, 0)), (ISD::FMul, x));
    assert_eq!(bits(&dag, dag.operand(root, 1)), Some(3.5f64.to_bits().into()));
    // without the flags the sum rounds twice: it stays
    let mul = dag.get_node(ISD::FMul, F64, &[x, c]);
    let add = dag.get_node(ISD::FAdd, F64, &[mul, x]);
    assert_eq!(combined(&mut dag, add), add);
}

/// `visitFNEG` and `visitFABS` did not exist: `-(2.0)`, `-(-x)` and `abs(-2.0)`
/// stayed as nodes (an x87 `fchs`/`fabs` of a constant load).
#[test]
fn a_negated_constant_or_negation_and_an_abs_of_a_constant_fold() {
    let mut dag = dag();
    let two = double(&mut dag, 2.0);
    let neg = dag.get_node(ISD::FNeg, F64, &[two]);
    let root = combined(&mut dag, neg);
    assert_eq!(bits(&dag, root), Some((-2.0f64).to_bits().into()));
    let x = load(&mut dag, F64, 1);
    let inner = dag.get_node(ISD::FNeg, F64, &[x]);
    let outer = dag.get_node(ISD::FNeg, F64, &[inner]);
    assert_eq!(combined(&mut dag, outer), x);
    let minus_two = double(&mut dag, -2.0);
    let abs = dag.get_node(ISD::FAbs, F64, &[minus_two]);
    let root = combined(&mut dag, abs);
    assert_eq!(bits(&dag, root), Some(2.0f64.to_bits().into()));
}

/// `visitFP_ROUND`, `visitFP_EXTEND` and the int conversions did not exist:
/// `(float)(double)f` stayed a widen and a narrow, and `(double)3` a runtime
/// conversion.
#[test]
fn a_round_of_an_extend_and_conversions_of_constants_fold() {
    let mut dag = dag();
    let f = load(&mut dag, F32, 1);
    let wide = dag.get_node(ISD::FpExtend, F64, &[f]);
    let zero = dag.get_constant_with(0, EVT::I32, true, false);
    let narrow = dag.get_node(ISD::FpRound, F32, &[wide, zero]);
    assert_eq!(combined(&mut dag, narrow), f);
    let c = double(&mut dag, 0.1);
    let round = dag.get_node(ISD::FpRound, F32, &[c, zero]);
    let root = combined(&mut dag, round);
    assert_eq!(bits(&dag, root), Some(u128::from(0.1f32.to_bits())));
    let three = dag.get_constant(3, EVT::I32);
    let conv = dag.get_node(ISD::SIntToFp, F64, &[three]);
    let root = combined(&mut dag, conv);
    assert_eq!(bits(&dag, root), Some(3.0f64.to_bits().into()));
    let x = double(&mut dag, -2.75);
    let conv = dag.get_node(ISD::FpToSInt, EVT::I32, &[x]);
    let root = combined(&mut dag, conv);
    assert_eq!(dag.node(root).data, NodeData::Const { bits: u128::from((-2i32) as u32), opaque: false });
}

/// `visitFDIV` did not exist: `x / 2.0` stayed a divide (x87 `fdiv`) instead of
/// `x * 0.5`; `x / 3.0` is inexact and only folds with `arcp`.
#[test]
fn a_divide_by_a_constant_is_a_multiply_when_the_reciprocal_is_exact_or_arcp() {
    let mut dag = dag();
    let x = load(&mut dag, F64, 1);
    let two = double(&mut dag, 2.0);
    let div = dag.get_node(ISD::FDiv, F64, &[x, two]);
    let root = combined(&mut dag, div);
    assert_eq!((dag.opcode(root), dag.operand(root, 0)), (ISD::FMul, x));
    assert_eq!(bits(&dag, dag.operand(root, 1)), Some(0.5f64.to_bits().into()));
    let three = double(&mut dag, 3.0);
    let div = dag.get_node(ISD::FDiv, F64, &[x, three]);
    let root = combined(&mut dag, div);
    assert_eq!(dag.opcode(root), ISD::FDiv);
    let y = load(&mut dag, F64, 2);
    let div = dag.get_node_flags(ISD::FDiv, F64, &[y, three], SDNodeFlags::ALLOW_RECIPROCAL);
    let root = combined(&mut dag, div);
    assert_eq!(dag.opcode(root), ISD::FMul);
    assert_eq!(bits(&dag, dag.operand(root, 1)), Some((1.0f64 / 3.0).to_bits().into()));
}
