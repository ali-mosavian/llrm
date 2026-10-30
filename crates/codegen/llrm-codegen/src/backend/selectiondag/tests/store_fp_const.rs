use super::super::dag::{MemBase, MemInfo};
use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn store_of(
    vt: MVT,
    bits: u128,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let vt = EVT::Simple(vt);
    let ptr = dag.get_register(EVT::I32, 5);
    let value = dag.get_constant_fp(bits, vt);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: vt, align: 4, volatile: false, base: MemBase::Unknown, ext: None, space: 0, ordering: None };
    let store = dag.get_store(chain, value, ptr, info);
    dag.set_root(store);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

/// `store float 1.5, p` loaded the constant onto the x87 stack and stored it
/// (`LD_Fp32m` + `ST_Fp32m`); the pin stores the bits as an integer (bench
/// `nbody_single`, `x_nbody_f`: 12 and 20 `MOV32mi` the pin has where we had
/// `ST_Fp32m`).
#[test]
fn a_float_constant_is_stored_as_the_integer_of_its_bits() {
    let (dag, root) = store_of(MVT::F32, 0x3FC0_0000);
    assert_eq!(dag.opcode(root), ISD::Store);
    let value = dag.operand(root, 1);
    assert_eq!(dag.value_type(value), EVT::I32);
    assert_eq!(dag.constant(value), Some(0x3FC0_0000));
}

/// A double the FPU cannot load directly (not 0.0 or 1.0) is two `i32` stores.
#[test]
fn a_double_constant_is_two_dword_stores() {
    let (dag, root) = store_of(MVT::F64, 0x3FF8_0000_0000_0000);
    assert_eq!(dag.opcode(root), ISD::TokenFactor);
    let values: Vec<Option<u128>> = (0..2).map(|i| dag.constant(dag.operand(dag.operand(root, i), 1))).collect();
    assert_eq!(values, [Some(0), Some(0x3FF8_0000)]);
}

/// 1.0 is `fld1`: the FPU loads it, so the store is left alone.
#[test]
fn a_double_one_is_left_to_the_fpu() {
    let (dag, root) = store_of(MVT::F64, 0x3FF0_0000_0000_0000);
    assert_eq!(dag.opcode(root), ISD::Store);
    assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::ConstantFP);
}
