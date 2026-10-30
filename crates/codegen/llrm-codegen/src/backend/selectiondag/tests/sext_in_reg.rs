use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;

/// `(sra (shl x, 31), 31)` stayed a shift pair (`shl; sar`) where the pin
/// makes `sign_extend_inreg x, i1` (bench `crc`: SAR32ri only we have).
#[test]
fn a_shift_pair_by_one_amount_is_a_sign_extension_in_register() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let by = dag.get_constant(31, EVT::I8);
    let up = dag.get_node(ISD::Shl, EVT::I32, &[x, by]);
    let root = dag.get_node(ISD::Sra, EVT::I32, &[up, by]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SignExtendInReg);
    assert_eq!(dag.node(dag.operand(root, 1)).data, NodeData::ValueType(EVT::I1));
}

/// `sign_extend_inreg x, i1` is `0 - (x & 1)`: the legalizer stopped on it
/// ("SIGN_EXTEND_INREG: convert node to libcall").
#[test]
fn a_one_bit_sign_extension_legalizes_to_a_negated_mask() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let ty = dag.get_value_type_node(EVT::I1);
    let root = dag.get_node(ISD::SignExtendInReg, EVT::I32, &[x, ty]);
    dag.set_root(root);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Sub);
    assert_eq!(dag.constant(dag.operand(root, 0)), Some(0));
    let mask = dag.operand(root, 1);
    assert_eq!(dag.opcode(mask), ISD::And);
    assert_eq!(dag.constant(dag.operand(mask, 1)), Some(1));
}
