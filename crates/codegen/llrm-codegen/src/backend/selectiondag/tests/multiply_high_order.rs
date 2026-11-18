//! DEVIATE SD444: where the multiply reads the length of its r/m operand (the
//! 486), `BuildUDIV` puts the magic first so the dividend is that operand.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn multiplies(
    dag: &SelectionDAG,
    v: SDValue,
    seen: &mut Vec<u32>,
    found: &mut Vec<SDValue>,
) {
    if seen.contains(&v.node) {
        return;
    }
    seen.push(v.node);
    if matches!(dag.opcode(v), ISD::MulHU | ISD::UMulLoHi) {
        found.push(v);
    }
    for i in 0..dag.node(v).ops.len() {
        multiplies(dag, dag.operand(v, i), seen, found);
    }
}

/// `(x urem 65537) urem 10`: the multiply-highs of both remainders, as
/// whether their first operand is the magic constant.
fn magic_first(early_out: Option<bool>) -> Vec<bool> {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let (a, b) = (dag.get_constant(65537, EVT::I32), dag.get_constant(10, EVT::I32));
    let inner = dag.get_node(ISD::URem, EVT::I32, &[x, a]);
    let outer = dag.get_node(ISD::URem, EVT::I32, &[inner, b]);
    dag.set_root(outer);
    let mut hooks = X86Lowering {
        multiply_early_out: early_out,
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
        profile: None,
    };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    let (mut seen, mut found) = (Vec::new(), Vec::new());
    multiplies(&dag, root, &mut seen, &mut found);
    found.iter().map(|&m| dag.constant(dag.operand(m, 0)).is_some()).collect()
}

/// `switch` at i486 -O2 left the magic of `r % 10` and `r % 14` as the r/m
/// operand of `mul` (36 clocks each of 2000 iterations) because the choice was
/// made at selection, when the inner remainder was already `x - q*c` with no
/// known length: 344,743 clocks against gcc's 328,143. Asked where the dividend
/// is still a `urem` (17 bits), both multiplies put the magic first.
#[test]
fn the_magic_goes_first_where_the_multiply_reads_the_length_of_its_r_m_operand() {
    let on = magic_first(Some(true));
    assert_eq!(on.len(), 2, "two multiply-highs expected, found {on:?}");
    assert!(on.iter().all(|&first| first), "magic not first with the early-out: {on:?}");
}

/// The pin's order (dividend, magic) stays where the multiply has no
/// early-out; this is also the control that the instrument sees the nodes.
#[test]
fn the_pins_order_stays_without_an_early_out() {
    let off = magic_first(None);
    assert_eq!(off.len(), 2, "two multiply-highs expected, found {off:?}");
    assert!(off.iter().all(|&first| !first), "magic first without the early-out: {off:?}");
}
