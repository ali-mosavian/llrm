use super::super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::super::known_bits::{KnownBits, known_bits};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn byte(
    dag: &mut SelectionDAG,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(EVT::I8, chain, ptr, info)
}

/// The promoted `i1` condition is `and (setcc i8), 1`; the pin drops the
/// `and` because a compare is 0 or 1 (42 of 159 shadow functions had the
/// extra `AND8ri` before a branch).
#[test]
fn and_one_of_a_compare_is_the_compare() {
    let mut dag = dag();
    let (a, b) = (byte(&mut dag, 1), byte(&mut dag, 2));
    let test = dag.get_setcc(EVT::I8, a, b, CondCode::SetLt);
    let one = dag.get_constant(1, EVT::I8);
    let masked = dag.get_node(ISD::And, EVT::I8, &[test, one]);
    dag.set_root(masked);
    combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
    assert_eq!(dag.get_root(), test);
}

/// A load can be anything: `and x, 1` stays.
#[test]
fn and_one_of_an_unknown_byte_stays() {
    let mut dag = dag();
    let x = byte(&mut dag, 1);
    let one = dag.get_constant(1, EVT::I8);
    let masked = dag.get_node(ISD::And, EVT::I8, &[x, one]);
    dag.set_root(masked);
    combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
    assert_eq!(dag.opcode(dag.get_root()), ISD::And);
}

/// `and x, 0xff` of a zero-extended byte is the extension;
/// `or`/`xor`/`select` and the depth cap are what the known bits say.
#[test]
fn known_bits_of_the_nodes_the_folds_ask_about() {
    let mut dag = dag();
    let x = byte(&mut dag, 1);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[x]);
    assert_eq!(known_bits(&dag, wide).zero, 0xFFFF_FF00);
    let ff = dag.get_constant(0xff, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[wide, ff]);
    dag.set_root(masked);
    combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
    // The `and` is gone; the extension of the load became the load's own.
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::I32));
    let c = dag.get_constant(6, EVT::I8);
    assert_eq!(known_bits(&dag, c), KnownBits::constant(6, 8));
    assert!(known_bits(&dag, c).is_constant());
    let three = dag.get_constant(3, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I8, &[x, three]);
    assert_eq!(known_bits(&dag, shifted).zero, 0b1110_0000);
}

/// `zext i32 (setcc i8)` promotes to `and (anyext (setcc i8)), 1`; the pin
/// leaves one `movzx` of the `setcc`, ours kept the `and` (`ours only
/// AND32ri`, R_WALK_LAYOUT_OK of the m32 shadow).
#[test]
fn and_one_of_an_extended_compare_is_a_zero_extension() {
    let mut dag = dag();
    let (a, b) = (byte(&mut dag, 1), byte(&mut dag, 2));
    let test = dag.get_setcc(EVT::I8, a, b, CondCode::SetEq);
    let wide = dag.get_node(ISD::AnyExtend, EVT::I32, &[test]);
    let one = dag.get_constant(1, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[wide, one]);
    dag.set_root(masked);
    combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.operand(root, 0)), (ISD::ZeroExtend, test));
}

/// A mask that keeps a bit above the source, or leaves out a bit the source
/// may set, is not a zero extension.
#[test]
fn and_of_an_extended_value_with_another_mask_stays() {
    for mask in [0x1FFu128, 0x7F] {
        let mut dag = dag();
        let x = byte(&mut dag, 1);
        let wide = dag.get_node(ISD::AnyExtend, EVT::I32, &[x]);
        let c = dag.get_constant(mask, EVT::I32);
        let masked = dag.get_node(ISD::And, EVT::I32, &[wide, c]);
        dag.set_root(masked);
        combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
        assert_eq!(dag.opcode(dag.get_root()), ISD::And, "mask {mask:#x}");
    }
}

/// `umax`/`umin` of two zero-extended bytes is a byte: the `and x, 0xff` of a promoted `umax i8` stayed,
/// so `umax(a, b) - b` was not seen after type legalization (`w8`: `subs`, `csel`, `subs` where the pin
/// has `subs`, `csel`).
#[test]
fn the_mask_of_a_umax_of_bytes_is_dropped_and_the_subtract_becomes_a_usubsat() {
    let mut dag = dag();
    let (a, b) = (byte(&mut dag, 1), byte(&mut dag, 2));
    let (wa, wb) = (dag.get_node(ISD::ZeroExtend, EVT::I32, &[a]), dag.get_node(ISD::ZeroExtend, EVT::I32, &[b]));
    let max = dag.get_node(ISD::UMax, EVT::I32, &[wa, wb]);
    assert_eq!(known_bits(&dag, max).zero, 0xFFFF_FF00);
    let min = dag.get_node(ISD::UMin, EVT::I32, &[wa, wb]);
    assert_eq!(known_bits(&dag, min).zero, 0xFFFF_FF00);
    let ff = dag.get_constant(0xff, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[max, ff]);
    let diff = dag.get_node(ISD::Sub, EVT::I32, &[masked, wb]);
    dag.set_root(diff);
    combine(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes);
    assert_eq!(dag.opcode(dag.get_root()), ISD::USubSat);
}

/// An unknown operand makes the `umax` unknown above what the other operand bounds.
#[test]
fn a_umax_with_an_unknown_operand_keeps_nothing_it_cannot_prove() {
    let mut dag = dag();
    let a = byte(&mut dag, 1);
    let wa = dag.get_node(ISD::ZeroExtend, EVT::I32, &[a]);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(9), ext: None, space: 0, ordering: None };
    let wide = dag.get_load(EVT::I32, chain, ptr, info);
    let max = dag.get_node(ISD::UMax, EVT::I32, &[wa, wide]);
    assert_eq!(known_bits(&dag, max).zero, 0);
}
