//! Folds the pin makes for every target, on only for a target that asks
//! (`CombineHooks`), so x86's rows are untouched until the judge has seen them.

use super::super::actions::LoadExt;
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
