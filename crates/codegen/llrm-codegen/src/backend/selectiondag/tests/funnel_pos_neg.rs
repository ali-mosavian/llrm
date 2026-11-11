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
        imul_if_smaller: false,
        share_products: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load: false,
        zext_free: false,
        branch_constant_selects: false,
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
