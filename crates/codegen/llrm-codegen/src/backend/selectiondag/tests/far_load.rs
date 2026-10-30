use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::{ES, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A far load is one `les`: the offset is its result and the selector is
/// read from `ES` after it, on its chain. Two word loads cost two
/// instructions and let anything between them change `ES`.
#[test]
fn a_far_load_is_les_and_a_read_of_es() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I16, 7);
    let chain = dag.entry_node();
    let info =
        MemInfo { mem_vt: MVT::Far.into(), align: 2, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 , ordering: None };
    let far = dag.get_load(MVT::Far.into(), chain, ptr, info);
    let offset = dag.get_node(ISD::Truncate, EVT::I16, &[far]);
    dag.set_root(offset);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let les = dag.get_root();
    assert_eq!(
        (dag.opcode(les), les.res, dag.value_type(les)),
        (x86isd::LES, 0, EVT::I16),
        "the offset is the les result"
    );
    assert_eq!((dag.operand(les, 0), dag.operand(les, 1)), (chain, ptr));
    // The chain users of the original load follow the read of ES.
    let copy_chain = dag.users(SDValue { node: les.node, res: 1 }).next().expect("something reads the les chain");
    let copy = SDValue { node: copy_chain, res: 0 };
    assert_eq!((dag.opcode(copy), dag.value_type(copy)), (ISD::CopyFromReg, EVT::I16));
    assert_eq!(dag.node(dag.operand(copy, 1)).data, NodeData::Register(ES));
}
