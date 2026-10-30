//! Shift-amount constants take the target's shift type
//! (`getShiftAmountConstant`), not a fixed `i8`: on a target whose amounts have
//! the shifted type (m68k, wasm), an `i8` count never matched the shift
//! patterns.

use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

struct ShiftsBy32;

impl CombineHooks for ShiftsBy32 {
    fn shift_amount_type(
        &self,
        _shifted: EVT,
    ) -> EVT {
        EVT::I32
    }
}

fn shifts_by_value() -> TargetLowering {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    tli.types = tli.types.with_shift_by_value_type();
    tli
}

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

/// The shifts reachable from `v`: the type each one shifts and its amount's.
fn amounts(
    dag: &SelectionDAG,
    v: SDValue,
    found: &mut Vec<(ISD, EVT, EVT)>,
) {
    let op = dag.opcode(v);
    if matches!(
        op,
        ISD::Shl | ISD::Srl | ISD::Sra | ISD::FShl | ISD::FShr | ISD::ShlParts | ISD::SrlParts | ISD::SraParts
    ) {
        let amount = dag.operand(v, dag.node(v).ops.len() - 1);
        found.push((op, dag.value_type(dag.operand(v, 0)), dag.value_type(amount)));
    }
    for at in 0..dag.node(v).ops.len() {
        amounts(dag, dag.operand(v, at), found);
    }
}

fn assert_by_value(
    dag: &SelectionDAG,
    what: &str,
) {
    let mut found = Vec::new();
    amounts(dag, dag.get_root(), &mut found);
    assert!(!found.is_empty(), "{what}: no shift was made");
    for (op, shifted, ty) in found {
        assert_eq!(ty, shifted, "{what}: a {op:?} of {shifted:?} has an amount of {ty:?}");
    }
}

/// `sra` of an `i12` sign-fills with `shl`/`sra` by `16 - 12`.
#[test]
fn a_promoted_sra_fills_by_an_amount_of_the_target_type() {
    let tli = shifts_by_value();
    let mut dag = dag();
    let (x, by) = (load(&mut dag, EVT::Int(12), 1), load(&mut dag, EVT::Int(12), 2));
    let shifted = dag.get_node(ISD::Sra, EVT::Int(12), &[x, by]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[shifted]);
    dag.set_root(byte);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    assert_by_value(&dag, "promote sra");
}

/// An `i64` shifted by a constant expands to half-width shifts by constants.
#[test]
fn an_expanded_constant_shift_uses_an_amount_of_the_target_type() {
    let tli = shifts_by_value();
    let mut dag = dag();
    let x = load(&mut dag, EVT::I64, 1);
    for (op, by) in [(ISD::Shl, 5), (ISD::Srl, 40), (ISD::Sra, 63)] {
        let amount = dag.get_constant(by, EVT::I32);
        let moved = dag.get_node(op, EVT::I64, &[x, amount]);
        let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
        dag.set_root(low);
        TypeLegalizer::new(&mut dag, &tli).run().unwrap();
        assert_by_value(&dag, &format!("expand {op:?} by {by}"));
    }
}

/// The amount of an expanded variable `i64` shift takes the shifted type.
#[test]
fn an_expanded_variable_shift_takes_an_amount_of_the_target_type() {
    let tli = shifts_by_value();
    let mut dag = dag();
    let (x, by) = (load(&mut dag, EVT::I64, 1), load(&mut dag, EVT::I8, 2));
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, by]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    assert_by_value(&dag, "expand variable shl");
}

fn combined(
    mut dag: SelectionDAG,
    root: SDValue,
) -> SelectionDAG {
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut ShiftsBy32, CombineLevel::BeforeLegalizeTypes).run();
    dag
}

/// `select c, 4, 0` is `zext(c) << 2`.
#[test]
fn a_select_of_a_power_of_two_shifts_by_an_amount_of_the_target_type() {
    let mut dag = dag();
    let cond = dag.get_register(EVT::I1, 5);
    let (four, zero) = (dag.get_constant(4, EVT::I32), dag.get_constant(0, EVT::I32));
    let root = dag.get_select(EVT::I32, cond, four, zero);
    assert_by_value(&combined(dag, root), "select pow2");
}

/// `select (x < 0), 5, ~5` is a sign mask: `x >>s 31`, xor.
#[test]
fn a_select_on_the_sign_smears_by_an_amount_of_the_target_type() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let zero = dag.get_constant(0, EVT::I32);
    let cond = dag.get_setcc(EVT::I1, x, zero, CondCode::SetLt);
    let (then, other) = (dag.get_constant(5, EVT::I32), dag.get_constant(!5, EVT::I32));
    let root = dag.get_select(EVT::I32, cond, then, other);
    assert_by_value(&combined(dag, root), "select sign");
}

/// `select ((x & 4) == 0), 0, a` is `a & ((x << 29) >>s 31)`.
#[test]
fn a_select_on_a_single_bit_shifts_by_amounts_of_the_target_type() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let (mask, zero) = (dag.get_constant(4, EVT::I32), dag.get_constant(0, EVT::I32));
    let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
    let cond = dag.get_setcc(EVT::I1, and, zero, CondCode::SetEq);
    let poly = dag.get_register(EVT::I32, 6);
    let root = dag.get_select(EVT::I32, cond, zero, poly);
    assert_by_value(&combined(dag, root), "select single bit");
}

/// `bswap (zext i8)` is a shift left by 24.
#[test]
fn a_bswap_of_one_byte_shifts_by_an_amount_of_the_target_type() {
    let mut dag = dag();
    let byte = dag.get_register(EVT::I8, 5);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[byte]);
    let root = dag.get_node(ISD::BSwap, EVT::I32, &[wide]);
    assert_by_value(&combined(dag, root), "bswap");
}
