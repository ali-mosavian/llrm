use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::{LegalizeError, TypeLegalizer};
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

/// An `i64` no instruction can fold (an opaque constant), shifted, then
/// cut to a dword: `low` takes the low half, `high` the high half.
fn shifted(
    op: ISD,
    by: u128,
) -> (SelectionDAG, SDValue, SDValue) {
    let mut dag = dag();
    let x = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
    let amount = dag.get_constant(by, EVT::I8);
    let moved = dag.get_node(op, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    let thirty_two = dag.get_constant(32, EVT::I8);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[moved, thirty_two]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
    let both = dag.get_node(ISD::Add, EVT::I32, &[low, high]);
    dag.set_root(both);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    let (l, h) = (dag.operand(root, 0), dag.operand(root, 1));
    (dag, l, h)
}

fn amount_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> (Option<u128>, EVT) {
    let amount = dag.operand(value, dag.node(value).ops.len() - 1);
    (dag.constant(amount), dag.value_type(amount))
}

/// `x << 3` at 32 bits: the low dword is a shift, the high is the funnel
/// shift `shld` (legal on i486/P5), with an `i8` amount. A constant shift
/// no expansion handles was refused by name before.
#[test]
fn a_left_shift_by_a_small_constant_is_a_shift_and_a_funnel_shift() {
    let (dag, low, high) = shifted(ISD::Shl, 3);
    assert_eq!((dag.opcode(low), amount_of(&dag, low)), (ISD::Shl, (Some(3), EVT::I8)));
    assert_eq!((dag.opcode(high), amount_of(&dag, high)), (ISD::FShl, (Some(3), EVT::I8)));
}

/// A shift by 40 moves a whole dword: `x >> 40` is `hi >> 8` low, zero
/// high.
#[test]
fn a_right_shift_past_one_dword_shifts_the_high_dword() {
    let (dag, low, high) = shifted(ISD::Srl, 40);
    assert_eq!((dag.opcode(low), amount_of(&dag, low)), (ISD::Srl, (Some(8), EVT::I8)));
    assert!(dag.is_null_constant(high));
}

/// An arithmetic shift by 63 leaves only the sign: the same node in both
/// halves.
#[test]
fn an_arithmetic_shift_out_is_the_sign_in_both_halves() {
    let (dag, low, high) = shifted(ISD::Sra, 63);
    assert_eq!((dag.opcode(low), amount_of(&dag, low).0), (ISD::Sra, Some(31)));
    assert_eq!(low, high);
}

/// A variable amount is the double-width shift node: it takes both halves
/// and an `i8` amount and has two results.
#[test]
fn a_variable_shift_is_shl_parts() {
    let mut dag = dag();
    let x = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I8, align: 1, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 };
    let amount = dag.get_load(EVT::I8, chain, ptr, info);
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), root.res), (ISD::ShlParts, 0));
    assert_eq!(dag.node(root).values, vec![EVT::I32, EVT::I32]);
    assert_eq!(dag.node(root).ops.len(), 3);
}

/// The target's lowering of the double-width shift: the high dword of
/// `shl_parts` is `cmov(shld, shl)` on bit 5 of the amount.
#[test]
fn shl_parts_is_a_funnel_shift_a_shift_and_a_cmov_on_bit_five() {
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let load = |dag: &mut SelectionDAG, vt: EVT, slot: i32| {
        let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 };
        dag.get_load(vt, chain, ptr, info)
    };
    let (lo, hi, amount) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I8, 3));
    let parts = dag.get_node_multi(ISD::ShlParts, &[EVT::I32, EVT::I32], &[lo, hi, amount]);
    dag.set_root(SDValue { node: parts.node, res: 1 });
    let tli = p5();
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false }).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), root.res), (ISD::MergeValues, 1), "the merge of both halves; the combiner drops it");
    let high = dag.operand(root, 1);
    assert_eq!(dag.opcode(high), x86isd::CMOV);
    assert_eq!(dag.opcode(dag.operand(high, 0)), ISD::FShl, "the false value is the funnel shift");
    assert_eq!(dag.opcode(dag.operand(high, 1)), ISD::Shl, "the true value is the shift by amount & 31");
}

/// A shift with an amount and no `shl_parts` (the table empty) is refused.
#[test]
fn a_variable_shift_without_the_parts_node_is_refused() {
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::ShlParts, MVT::I32, Action::Expand);
    let mut dag = dag();
    let x = dag.get_constant_with(5, EVT::I64, false, true);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I8, align: 1, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 };
    let amount = dag.get_load(EVT::I8, chain, ptr, info);
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    let error = TypeLegalizer::new(&mut dag, &tli).run().unwrap_err();
    assert!(matches!(error, LegalizeError::Unsupported { op: ISD::Shl, .. }), "{error:?}");
}
