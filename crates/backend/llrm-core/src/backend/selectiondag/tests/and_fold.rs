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
    let info = MemInfo { mem_vt: EVT::I8, align: 1, volatile: false, base: MemBase::Stack(slot) };
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
    assert_eq!(dag.get_root(), wide);
    let c = dag.get_constant(6, EVT::I8);
    assert_eq!(known_bits(&dag, c), KnownBits::constant(6, 8));
    assert!(known_bits(&dag, c).is_constant());
    let three = dag.get_constant(3, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I8, &[x, three]);
    assert_eq!(known_bits(&dag, shifted).zero, 0b1110_0000);
}
