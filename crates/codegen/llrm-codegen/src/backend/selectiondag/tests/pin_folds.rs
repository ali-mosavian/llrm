//! Folds the pin makes for every target, on only for a target that asks
//! (`CombineHooks`), so x86's rows are untouched until the judge has seen them.

use super::super::actions::{LoadExt, TargetLowering};
use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner, NoCombines};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

struct Asks;

impl CombineHooks for Asks {
    fn fold_xor_of_and_with_mask(&self) -> bool {
        true
    }
    fn fold_sext_in_reg_of_load(&self) -> bool {
        true
    }
    fn push_freeze_through_operands(&self) -> bool {
        true
    }
    fn merge_stores_via_trunc_store(&self) -> bool {
        true
    }
    fn fold_or_absorption(&self) -> bool {
        true
    }
}

fn xor_of_masked(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let mask = dag.get_constant(0xf0f0, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
    let root = dag.get_node(ISD::Xor, EVT::I32, &[masked, mask]);
    dag.set_root(root);
}

/// `(a & c) ^ c` stayed an `and` and an `xor`; the pin makes it `(~a) & c`,
/// which AArch64 selects as one `bic`.
#[test]
fn xor_of_a_masked_value_with_its_mask_is_the_masked_complement() {
    let mut dag = dag();
    xor_of_masked(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::And);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Xor, "the complement of a");
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xf0f0));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_xor_of_the_masked_value() {
    let mut dag = dag();
    xor_of_masked(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Xor);
}

fn sext_in_reg_of_extload(dag: &mut SelectionDAG) {
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info =
        MemInfo { mem_vt: EVT::I8, align: 1, volatile: false, base: MemBase::Stack(0), ext: Some(LoadExt::ExtLoad), space: 0 };
    let load = dag.get_ext_load(LoadExt::ExtLoad, EVT::I32, chain, ptr, info);
    let narrow = dag.get_value_type_node(EVT::I8);
    let root = dag.get_node(ISD::SignExtendInReg, EVT::I32, &[load, narrow]);
    dag.set_root(root);
}

/// A stack byte read as `extload i8` and sign-extended in the register stayed
/// two operations; the pin makes it one `sextload i8`.
#[test]
fn sext_in_reg_of_an_extending_load_is_a_sign_extending_load() {
    let mut dag = dag();
    sext_in_reg_of_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Load);
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a memory node") };
    assert_eq!(info.ext, Some(LoadExt::SExtLoad));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_sign_extension_in_the_register() {
    let mut dag = dag();
    sext_in_reg_of_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::SignExtendInReg);
}

fn frozen_compare(dag: &mut SelectionDAG) {
    let a = dag.get_register(EVT::I32, 5);
    // A shift by a variable amount may be poison, so the combiner cannot drop the freeze.
    let amount = dag.get_register(EVT::I32, 6);
    let loaded = dag.get_node(ISD::Shl, EVT::I32, &[a, amount]);
    let setcc = dag.get_setcc(EVT::I32, a, loaded, CondCode::SetLt);
    let root = dag.get_node(ISD::Freeze, EVT::I32, &[setcc]);
    dag.set_root(root);
}

/// `freeze (setcc a, x)` stayed a freeze of a compare; the pin freezes the
/// operand it cannot prove defined, so the compare is a bare `setcc` again.
#[test]
fn a_frozen_compare_is_a_compare_of_a_frozen_operand() {
    let mut dag = dag();
    frozen_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SetCC);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_frozen_compare() {
    let mut dag = dag();
    frozen_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Freeze);
}

fn two_byte_stores(dag: &mut SelectionDAG) {
    let (ptr, chain) = (dag.get_register(EVT::I64, 7), dag.entry_node());
    let info = |offset| MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(offset),
        ext: None,
        space: 0,
    };
    let (lo, hi) = (dag.get_constant(1, EVT::I8), dag.get_constant(0, EVT::I8));
    let one = dag.get_constant(1, EVT::I64);
    let next = dag.get_node(ISD::Add, EVT::I64, &[ptr, one]);
    let first = dag.get_store(chain, lo, ptr, info(0));
    let second = dag.get_store(first, hi, next, info(1));
    dag.set_root(second);
}

/// A target with no 16-bit register that can still store one from an `i32`.
fn no_halfword_register() -> TargetLowering {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    tli.types = TargetLoweringInfo::new(&[MVT::I8, MVT::I32], false);
    tli.set_trunc_store_action(MVT::I32, MVT::I16, Action::Legal);
    tli
}

fn is_one_halfword_store(dag: &SelectionDAG) -> bool {
    let root = dag.get_root();
    let NodeData::Mem(info) = dag.node(root).data else { return false };
    dag.opcode(root) == ISD::Store && info.mem_vt == EVT::I16 && dag.constant(dag.operand(root, 1)) == Some(1)
}

/// Two adjacent byte stores of constants stayed two stores on AArch64, which
/// has no 16-bit register: the pin joins them into one truncating `i16` store
/// of the promoted `i32` constant (`strh`).
#[test]
fn two_byte_constant_stores_are_one_truncating_halfword_store() {
    let tli = no_halfword_register();
    let mut dag = dag();
    two_byte_stores(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(is_one_halfword_store(&dag));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_two_byte_stores() {
    let tli = no_halfword_register();
    let mut dag = dag();
    two_byte_stores(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(!is_one_halfword_store(&dag));
}


/// Declines the bitwise form of a logic of compares, as AArch64 does.
struct Declines;

impl CombineHooks for Declines {
    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        false
    }
}

fn and_of_equalities(dag: &mut SelectionDAG) {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let (c1, c2) = (dag.get_constant(65, EVT::I32), dag.get_constant(66, EVT::I32));
    let left = dag.get_setcc(EVT::I1, a, c1, CondCode::SetEq);
    let right = dag.get_setcc(EVT::I1, b, c2, CondCode::SetEq);
    let root = dag.get_node(ISD::And, EVT::I1, &[left, right]);
    dag.set_root(root);
}

/// Two byte compares joined by `and` were one `xor`/`or` compare on AArch64 (x86's
/// `convertSetCCLogicToBitwiseLogic`); the pin's AArch64 leaves them two compares.
#[test]
fn a_target_that_declines_keeps_the_and_of_two_equalities() {
    let mut dag = dag();
    and_of_equalities(&mut dag);
    DagCombiner::new(&mut dag, &mut Declines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::And);
}

#[test]
fn a_default_target_joins_the_two_equalities_in_one_compare() {
    let mut dag = dag();
    and_of_equalities(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::SetCC);
}

fn or_of_and_with_one_operand(dag: &mut SelectionDAG) -> SDValue {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let masked = dag.get_node(ISD::And, EVT::I32, &[a, b]);
    let root = dag.get_node(ISD::Or, EVT::I32, &[masked, a]);
    dag.set_root(root);
    a
}

/// `(a & b) | a` stayed an `and` and an `or`; the pin's `visitORCommutative` makes it `a`.
#[test]
fn an_or_of_an_and_with_one_of_its_operands_is_that_operand() {
    let mut dag = dag();
    let a = or_of_and_with_one_operand(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.get_root(), a);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_or_of_the_and() {
    let mut dag = dag();
    or_of_and_with_one_operand(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Or);
}
