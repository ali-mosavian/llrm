use super::super::legalize_dag::{DagLegalizer, Hooks};
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

fn converted(
    op: ISD,
    to: EVT,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::Simple(MVT::F64), 5);
    let converted = dag.get_node(op, to, &[x]);
    dag.set_root(converted);
    DagLegalizer::new(
        &mut dag,
        &tli,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false },
    )
    .run()
    .unwrap();
    let root = dag.get_root();
    (dag, root)
}

fn stored(
    dag: &SelectionDAG,
    load: SDValue,
) -> (EVT, SDValue) {
    let fist = SDValue { node: dag.operand(load, 0).node, res: 0 };
    assert_eq!(dag.opcode(fist), x86isd::FP_TO_INT_IN_MEM);
    let NodeData::Mem(info) = dag.node(fist).data else { panic!("a memory node") };
    (info.mem_vt, dag.operand(fist, 1))
}

/// `(int)d` without SSE: `fist` of the double into a stack slot of the
/// result's size, read back. The shadow listed FpToSInt as a library call
/// (33 functions).
#[test]
fn a_signed_conversion_is_a_fist_to_a_slot_and_a_load() {
    for to in [EVT::I16, EVT::I32, EVT::I64] {
        let (dag, root) = converted(ISD::FpToSInt, to);
        assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, to));
        let (slot, value) = stored(&dag, root);
        assert_eq!(slot, to, "the slot has the result's size");
        assert_eq!(dag.opcode(value), ISD::Register, "the double is stored as is");
    }
}

/// An unsigned `i32` goes through a 64-bit slot (a signed `i64` holds every
/// `u32`), read back as the low dword.
#[test]
fn an_unsigned_i32_conversion_uses_an_i64_slot() {
    let (dag, root) = converted(ISD::FpToUInt, EVT::I32);
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::I32));
    assert_eq!(stored(&dag, root).0, EVT::I64);
}

/// An unsigned `i64` subtracts 2^63 from a value at or above it before the
/// `fist` and flips the sign bit of the result back. The float select and
/// compare it makes are lowered by the x87 compares (later), so the target's
/// hook is called alone.
#[test]
fn an_unsigned_i64_conversion_subtracts_two_to_the_63_and_flips_the_sign() {
    let mut dag = dag();
    let x = dag.get_register(EVT::Simple(MVT::F64), 5);
    let converted = dag.get_node(ISD::FpToUInt, EVT::I64, &[x]);
    let root = X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }
        .lower_operation(&mut dag, converted)
        .expect("lowered");
    assert_eq!(dag.opcode(root), ISD::Xor);
    let (load, adjust) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!(dag.opcode(load), ISD::Load);
    assert_eq!(dag.opcode(adjust), ISD::Shl, "the compare's bit shifted to bit 63");
    assert_eq!(dag.constant(dag.operand(adjust, 1)), Some(63));
    let (slot, value) = stored(&dag, load);
    assert_eq!(slot, EVT::I64);
    assert_eq!(dag.opcode(value), ISD::FSub, "the offset is taken off first");
}

/// An `i64` result is asked of the target before the type legalizer splits it
/// (the target's action for `i64` is Custom): the `fist` slot is `i64`, and
/// the result is read back as two dwords.
#[test]
fn an_i64_result_is_lowered_by_the_target_then_split() {
    use super::super::legalize_types::TypeLegalizer;
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::Simple(MVT::F64), 5);
    let converted = dag.get_node(ISD::FpToSInt, EVT::I64, &[x]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[converted]);
    dag.set_root(low);
    let mut hooks = X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false };
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut hooks);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::I32), "the low dword of the slot");
    let fist = SDValue { node: dag.operand(root, 0).node, res: 0 };
    assert_eq!(dag.opcode(fist), x86isd::FP_TO_INT_IN_MEM);
    assert_eq!(stored(&dag, root).0, EVT::I64);
}

struct StoresFist;

impl crate::backend::selectiondag::dag_combiner::CombineHooks for StoresFist {
    fn store_fp_to_int_in_memory(
        &self,
        _float: EVT,
        _int: EVT,
    ) -> bool {
        true
    }
}

fn store_of_fptosi(
    volatile: bool,
    extra_use: bool,
) -> (SelectionDAG, SDValue) {
    use crate::backend::selectiondag::dag_combiner::{CombineLevel, DagCombiner};
    let mut dag = dag();
    let x = dag.get_register(EVT::Simple(MVT::F32), 5);
    let ptr = dag.get_register(EVT::I16, 6);
    let converted = dag.get_node(ISD::FpToSInt, EVT::I32, &[x]);
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 2,
        volatile,
        base: MemBase::Unknown,
        ext: None,
        space: 0,
        ordering: None,
    };
    let entry = dag.entry_node();
    let store = dag.get_store(entry, converted, ptr, info);
    let root = if extra_use {
        let other = dag.get_register(EVT::I16, 7);
        let keep = dag.get_store(store, converted, other, info);
        dag.get_token_factor(&[store, keep])
    } else {
        store
    };
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut StoresFist, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

/// `store (fptosi x), p` was `fist` to a frame temporary, a reload and the
/// store; it is one `fist` on the store's pointer.
#[test]
fn a_store_of_fptosi_is_one_fist_on_the_pointer() {
    let (dag, root) = store_of_fptosi(false, false);
    assert_eq!(dag.opcode(root), x86isd::FP_TO_INT_IN_MEM);
    assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::Register, "the float is stored as is");
    assert_eq!(dag.opcode(dag.operand(root, 2)), ISD::Register, "into the store's pointer");
}

/// A volatile store keeps its own store of the converted value.
#[test]
fn a_volatile_store_of_fptosi_is_not_folded() {
    let (dag, root) = store_of_fptosi(true, false);
    assert_eq!(dag.opcode(root), ISD::Store);
}

/// A converted value used again is still produced by `fptosi`.
#[test]
fn a_store_of_a_shared_fptosi_is_not_folded() {
    let (dag, root) = store_of_fptosi(false, true);
    assert_ne!(dag.opcode(root), x86isd::FP_TO_INT_IN_MEM);
    let first = dag.operand(root, 0);
    assert_eq!(dag.opcode(first), ISD::Store);
    assert_eq!(dag.opcode(dag.operand(first, 1)), ISD::FpToSInt);
}
