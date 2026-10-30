use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// `phi float [x, a], [undef, b]` leaves a float `undef` in the DAG, and an
/// x87 target expands it: the legalizer answered "convert node to
/// libcall" and refused SB_BUILD block b8 of the m32 shadow. After: zero.
#[test]
fn an_undef_float_is_zero() {
    for vt in [MVT::F32, MVT::F64, MVT::F80] {
        let vt = EVT::Simple(vt);
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
        let undef = dag.get_undef(vt);
        let info = MemInfo {
            mem_vt: vt,
            align: 4,
            volatile: false,
            base: MemBase::Stack(1),
            ext: None,
            space: 0,
            ordering: None,
        };
        let store = dag.get_store(chain, undef, ptr, info);
        dag.set_root(store);
        DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false })
            .run()
            .unwrap();
        let root = dag.get_root();
        assert!(!reaches_undef(&dag, root), "{vt:?}: an undef is left");
    }
}

fn reaches_undef(
    dag: &SelectionDAG,
    v: SDValue,
) -> bool {
    dag.opcode(v) == ISD::Undef || (0..dag.node(v).ops.len()).any(|i| reaches_undef(dag, dag.operand(v, i)))
}
