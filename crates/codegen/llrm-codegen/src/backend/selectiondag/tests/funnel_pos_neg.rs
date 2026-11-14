//! `MatchFunnelPosNeg`: the expansion of an `i64` shift by a small amount is
//! `(or (shl hi, y), (srl (srl lo, 1), (xor y, 31)))`, which the pin folds to one
//! `fshl` (x86 `shld`). Not matched here (DEVIATE SD321), x86_32 -Os
//! `call_cleanup_keeps_pair` went 1016 -> 1040 B and x86_16 -O2
//! `shift_int64_masked_amount` 463 -> 518 B once the shift expanded without the funnel.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, z) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let y = dag.get_register(EVT::I8, 7);
    let root = build(&mut dag, x, z, y);
    dag.set_root(root);
    let mut hooks = X86Lowering {
        imul_if_smaller: false, hot_chains: false,
        share_products: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load: false,
        zext_free: false,
        branch_constant_selects: false,
        status_word_tests: false,
        multiply_early_out: None,
        profile: None,
    };
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

/// `(or (shl x, y), (srl (srl z, 1), (xor y, 31)))` is `(fshl x, z, y)`.
#[test]
fn a_shift_pair_through_one_and_xor_is_a_left_funnel_shift() {
    let (dag, root) = combined(|dag, x, z, y| {
        let (one, last) = (dag.get_constant(1, EVT::I8), dag.get_constant(31, EVT::I8));
        let high = dag.get_node(ISD::Shl, EVT::I32, &[x, y]);
        let by_one = dag.get_node(ISD::Srl, EVT::I32, &[z, one]);
        let inverse = dag.get_node(ISD::Xor, EVT::I8, &[y, last]);
        let low = dag.get_node(ISD::Srl, EVT::I32, &[by_one, inverse]);
        dag.get_node(ISD::Or, EVT::I32, &[high, low])
    });
    assert_eq!(dag.opcode(root), ISD::FShl);
}

/// `(or (shl (shl x, 1), (xor y, 31)), (srl z, y))` is `(fshr x, z, y)`.
#[test]
fn a_shift_pair_through_one_and_xor_is_a_right_funnel_shift() {
    let (dag, root) = combined(|dag, x, z, y| {
        let (one, last) = (dag.get_constant(1, EVT::I8), dag.get_constant(31, EVT::I8));
        let by_one = dag.get_node(ISD::Shl, EVT::I32, &[x, one]);
        let inverse = dag.get_node(ISD::Xor, EVT::I8, &[y, last]);
        let high = dag.get_node(ISD::Shl, EVT::I32, &[by_one, inverse]);
        let low = dag.get_node(ISD::Srl, EVT::I32, &[z, y]);
        dag.get_node(ISD::Or, EVT::I32, &[high, low])
    });
    assert_eq!(dag.opcode(root), ISD::FShr);
}

/// A constant `z` folds `(srl z, 1)` away before the pair is matched; x86_16 -Os
/// `call_cleanup_keeps_pair` then kept `shl/not/shr/or` for the folded shape
/// where `shld` was, 307 -> 327 B. The folded constant is `z >> 1` for a `z` that
/// is left to the fold's other half, so the pair is still `(fshl x, z, y)`.
#[test]
fn a_left_pair_whose_srl_of_a_constant_folded_is_a_left_funnel_shift() {
    let (dag, root) = combined(|dag, x, _, y| {
        let last = dag.get_constant(31, EVT::I8);
        let folded = dag.get_constant(0x3a5c_0f11, EVT::I32);
        let high = dag.get_node(ISD::Shl, EVT::I32, &[x, y]);
        let inverse = dag.get_node(ISD::Xor, EVT::I8, &[y, last]);
        let low = dag.get_node(ISD::Srl, EVT::I32, &[folded, inverse]);
        dag.get_node(ISD::Or, EVT::I32, &[high, low])
    });
    assert_eq!(dag.opcode(root), ISD::FShl);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0x3a5c_0f11 << 1));
}

/// The right-hand twin: `(shl (shl z, 1), inv)` folds to `(shl (z << 1), inv)`.
#[test]
fn a_right_pair_whose_shl_of_a_constant_folded_is_a_right_funnel_shift() {
    let (dag, root) = combined(|dag, _, z, y| {
        let last = dag.get_constant(31, EVT::I8);
        let folded = dag.get_constant(0x3a5c_0f10, EVT::I32);
        let inverse = dag.get_node(ISD::Xor, EVT::I8, &[y, last]);
        let high = dag.get_node(ISD::Shl, EVT::I32, &[folded, inverse]);
        let low = dag.get_node(ISD::Srl, EVT::I32, &[z, y]);
        dag.get_node(ISD::Or, EVT::I32, &[high, low])
    });
    assert_eq!(dag.opcode(root), ISD::FShr);
    assert_eq!(dag.constant(dag.operand(root, 0)), Some(0x3a5c_0f10 >> 1));
}

/// The folded constant leaves one bit free; x86_16 -Os `call_cleanup_keeps_pair` took the
/// even one while the shl side held the odd constant, so the pair needed a second `mov` of
/// 32 bits: 316 B against 307 B before the fold. The free bit follows a constant already in the DAG.
#[test]
fn a_folded_constant_takes_the_free_bit_of_a_constant_already_in_the_dag() {
    let (dag, root) = combined(|dag, x, _, y| {
        let last = dag.get_constant(31, EVT::I8);
        let shared = dag.get_constant(0x74b8_1e23, EVT::I32);
        let folded = dag.get_constant(0x74b8_1e23 >> 1, EVT::I32);
        let high = dag.get_node(ISD::Shl, EVT::I32, &[x, y]);
        let inverse = dag.get_node(ISD::Xor, EVT::I8, &[y, last]);
        let low = dag.get_node(ISD::Srl, EVT::I32, &[folded, inverse]);
        let pair = dag.get_node(ISD::Or, EVT::I32, &[high, low]);
        dag.get_node(ISD::Add, EVT::I32, &[pair, shared])
    });
    let funnel = dag.operand(root, 0);
    assert_eq!(dag.opcode(funnel), ISD::FShl);
    assert_eq!(dag.constant(dag.operand(funnel, 1)), Some(0x74b8_1e23));
}
