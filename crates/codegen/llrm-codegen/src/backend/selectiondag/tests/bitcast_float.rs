use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;

const F64: EVT = EVT::Simple(MVT::F64);
const F32: EVT = EVT::Simple(MVT::F32);

/// `union { double d; unsigned long w[2]; }` read as `w[0]`: a `bitcast
/// double -> i64` was taken as an integer shift of an x87 register, so the
/// matcher saw `and i32 <fdiv result>, 0xFFFFFF` (sdag-d's repro,
/// float_conversions_loop.c). The bits go through memory: the double stored to
/// a stack slot, the low dword loaded at the slot and the high at +4.
#[test]
fn a_double_read_as_an_i64_goes_through_a_stack_slot() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(F64, 5);
    let bits = dag.get_node(ISD::BitCast, EVT::I64, &[x]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[bits]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let low = dag.get_root();
    assert_eq!((dag.opcode(low), dag.value_type(low)), (ISD::Load, EVT::I32));
    let store = SDValue { node: dag.operand(low, 0).node, res: 0 };
    assert_eq!((dag.opcode(store), dag.operand(store, 1)), (ISD::Store, x), "the double is stored whole");
    assert_eq!(dag.operand(low, 1), dag.operand(store, 2), "the low dword is at the slot");
}

/// The high dword is at the slot plus four.
#[test]
fn the_high_dword_of_a_double_is_at_the_slot_plus_four() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(F64, 5);
    let bits = dag.get_node(ISD::BitCast, EVT::I64, &[x]);
    let shift = dag.get_constant(32, EVT::I8);
    let top = dag.get_node(ISD::Srl, EVT::I64, &[bits, shift]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[top]);
    dag.set_root(high);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let high = dag.get_root();
    assert_eq!(dag.opcode(high), ISD::Load);
    let address = dag.operand(high, 1);
    assert_eq!((dag.opcode(address), dag.constant(dag.operand(address, 1))), (ISD::Add, Some(4)));
}

/// An `i64` read as a double: the halves are stored and the double loaded.
#[test]
fn an_i64_read_as_a_double_is_stored_in_halves_and_loaded() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (lo, hi) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let wide = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    let value = dag.get_node(ISD::BitCast, F64, &[wide]);
    dag.set_root(value);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let load = dag.get_root();
    assert_eq!((dag.opcode(load), dag.value_type(load)), (ISD::Load, F64));
    let stores = dag.operand(load, 0);
    assert_eq!(dag.opcode(stores), ISD::TokenFactor);
    let values: Vec<_> = (0..2).map(|i| dag.operand(dag.operand(stores, i), 1)).collect();
    assert_eq!(values, [lo, hi]);
}

/// `bitcast float -> i32` (no SSE) is a store and a load, not a no-op.
#[test]
fn a_float_read_as_an_i32_goes_through_a_stack_slot() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(F32, 5);
    let bits = dag.get_node(ISD::BitCast, EVT::I32, &[x]);
    dag.set_root(bits);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false }).run().unwrap();
    let load = dag.get_root();
    assert_eq!((dag.opcode(load), dag.value_type(load)), (ISD::Load, EVT::I32));
    let store = SDValue { node: dag.operand(load, 0).node, res: 0 };
    assert_eq!((dag.opcode(store), dag.operand(store, 1)), (ISD::Store, x));
}
