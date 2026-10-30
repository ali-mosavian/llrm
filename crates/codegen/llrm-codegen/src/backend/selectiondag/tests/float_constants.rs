use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

fn legalized(
    bits: u128,
    vt: EVT,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let constant = dag.get_constant_fp(bits, vt);
    dag.set_root(constant);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// 8 shadow functions stopped at "ConstantFP: convert node to libcall". The
/// x87 makes 0.0 and 1.0 (and their negatives) with `fldz`/`fld1`: those
/// constants stay.
#[test]
fn zero_and_one_stay_as_constants() {
    for bits in [0u128, 0x3FF0_0000_0000_0000, 0xBFF0_0000_0000_0000, 1u128 << 63] {
        let (dag, root) = legalized(bits, EVT::Simple(MVT::F64));
        assert_eq!(dag.opcode(root), ISD::ConstantFP, "{bits:#x}");
    }
}

/// Any other constant is loaded from the constant pool through the target's
/// `Wrapper`; one a float holds exactly is a float load extended to a double.
#[test]
fn other_constants_are_loaded_from_the_pool() {
    let (dag, root) = legalized(2.5f64.to_bits().into(), EVT::Simple(MVT::F64));
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::Simple(MVT::F64)));
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a load") };
    assert_eq!(info.mem_vt, EVT::Simple(MVT::F32), "2.5 fits a float: the pool holds the float");
    // a plain f64 load of the 4-byte entry read 8 bytes: wc_mixed passed 3.0 as
    // a garbage double (want 234, got 224)
    assert!(info.ext.is_some(), "the float entry is read by an extending load");
    let address = dag.operand(root, 1);
    assert_eq!(dag.opcode(address), x86isd::WRAPPER);
    assert_eq!(dag.opcode(dag.operand(address, 0)), ISD::TargetConstantPool);
    assert_eq!(dag.constant_pool, vec![(u128::from(2.5f32.to_bits()), EVT::Simple(MVT::F32))]);

    let (dag, root) = legalized(0.1f64.to_bits().into(), EVT::Simple(MVT::F64));
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a load") };
    assert_eq!(info.mem_vt, EVT::Simple(MVT::F64), "0.1 is not a float");
    assert_eq!(dag.constant_pool, vec![(0.1f64.to_bits().into(), EVT::Simple(MVT::F64))]);
}

/// Equal constants share one pool entry.
#[test]
fn equal_constants_share_a_pool_entry() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let a = dag.get_constant_fp(0.1f64.to_bits().into(), EVT::Simple(MVT::F64));
    let b = dag.get_constant_fp(3.0f32.to_bits().into(), EVT::Simple(MVT::F32));
    let c = dag.get_constant_fp(0.1f64.to_bits().into(), EVT::Simple(MVT::F64));
    let sum = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F64), &[a, c]);
    let ext = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[b]);
    let both = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F64), &[sum, ext]);
    dag.set_root(both);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }).run().unwrap();
    assert_eq!(dag.constant_pool.len(), 2, "0.1 once, 3.0f once");
}
