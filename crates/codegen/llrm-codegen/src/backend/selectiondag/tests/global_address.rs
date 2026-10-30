use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

fn legalized(offset: i64) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let global = dag.get_global_address(EVT::I32, 3, offset);
    dag.set_root(global);
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

/// 71 of 158 functions of the shadow corpus stopped at "GlobalAddress:
/// convert node to libcall". The pin makes `Wrapper (TargetGlobalAddress
/// g)`, which the matcher turns into `mov reg, offset g`.
#[test]
fn a_global_is_a_wrapper_of_the_target_address() {
    let (dag, root) = legalized(0);
    assert_eq!(dag.opcode(root), x86isd::WRAPPER);
    let target = dag.operand(root, 0);
    assert_eq!(dag.opcode(target), ISD::TargetGlobalAddress);
    assert_eq!(dag.node(target).data, NodeData::Global { symbol: 3, offset: 0 });
}

/// `&g[2]` folds into the symbol; a negative offset is added after, as the
/// pin does when the code model would not take it.
#[test]
fn a_non_negative_offset_folds_and_a_negative_one_is_added() {
    let (dag, root) = legalized(8);
    assert_eq!(dag.node(dag.operand(root, 0)).data, NodeData::Global { symbol: 3, offset: 8 });
    let (dag, root) = legalized(-4);
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(dag.node(dag.operand(dag.operand(root, 0), 0)).data, NodeData::Global { symbol: 3, offset: 0 });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xFFFF_FFFC));
}
