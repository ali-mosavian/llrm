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
        &mut X86Lowering {
            multiply_early_out: None,
            imul_if_smaller: false, hot_chains: false,
            share_products: false,
            cmov: false,
            sse2: false,
            min_size: false,
            three_address_adds: false,
            i16_mul_chains: false,
            sext_load: false,
            zext_free: false,
            branch_constant_selects: false,
            status_word_tests: false,
            profile: None,
        },
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
    let root = X86Lowering {
        multiply_early_out: None,
        imul_if_smaller: false, hot_chains: false,
        share_products: false,
        cmov: false,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load: false,
        zext_free: false,
        branch_constant_selects: false,
        status_word_tests: false,
        profile: None,
    }
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
    let mut hooks = X86Lowering {
        multiply_early_out: None,
        imul_if_smaller: false, hot_chains: false,
        share_products: false,
        cmov: false,
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
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut hooks);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::I32), "the low dword of the slot");
    let fist = SDValue { node: dag.operand(root, 0).node, res: 0 };
    assert_eq!(dag.opcode(fist), x86isd::FP_TO_INT_IN_MEM);
    assert_eq!(stored(&dag, root).0, EVT::I64);
}

/// m64 double_to_uint64 -O1: an unsigned `i64` from an SSE double was a
/// compare, `setae`, `shl 63` and `xor` around a `subsd` (10 instructions, 40
/// B) where the pin's `LowerFP_TO_INT` (X86ISelLowering.cpp#L22414-L22449)
/// converts the value and the value less 2^63 and ors the first with the
/// second under the first's sign mask (7 instructions, 32 B). The i32 at m32
/// is the same with 2^31 and a shift of 31.
#[test]
fn an_unsigned_conversion_from_sse_is_a_small_and_a_big_under_the_sign_mask() {
    for (float, pointer, to, bits) in
        [(MVT::F64, EVT::I64, EVT::I64, 63), (MVT::F32, EVT::I64, EVT::I64, 63), (MVT::F64, EVT::I32, EVT::I32, 31)]
    {
        let mut dag = SelectionDAG::new(DagConfig { pointer, ..DagConfig::default() });
        let x = dag.get_register(EVT::Simple(float), 5);
        let converted = dag.get_node(ISD::FpToUInt, to, &[x]);
        let root = X86Lowering {
            multiply_early_out: None,
            imul_if_smaller: false, hot_chains: false,
            share_products: false,
            cmov: false,
            sse2: true,
            min_size: false,
            three_address_adds: false,
            i16_mul_chains: false,
            sext_load: false,
            zext_free: false,
            branch_constant_selects: false,
            status_word_tests: false,
            profile: None,
        }
        .lower_operation(&mut dag, converted)
        .expect("lowered");
        let what = format!("{float:?} to {to:?}");
        assert_eq!(dag.opcode(root), ISD::Or, "{what}");
        let (small, kept) = (dag.operand(root, 0), dag.operand(root, 1));
        assert_eq!((dag.opcode(small), dag.operand(small, 0)), (ISD::FpToSInt, x), "{what}: the value as is");
        assert_eq!(dag.opcode(kept), ISD::And, "{what}");
        let (big, mask) = (dag.operand(kept, 0), dag.operand(kept, 1));
        assert_eq!(dag.opcode(big), ISD::FpToSInt, "{what}");
        assert_eq!(dag.opcode(dag.operand(big, 0)), ISD::FSub, "{what}: the value less 2^{bits}");
        assert_eq!(dag.opcode(mask), ISD::Sra, "{what}");
        assert_eq!((dag.operand(mask, 0), dag.constant(dag.operand(mask, 1))), (small, Some(bits)), "{what}");
    }
}
