use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::ES;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn legalized(store: bool) -> (SelectionDAG, SDValue, [SDValue; 2]) {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let (chain, value) = (dag.entry_node(), dag.get_register(EVT::I16, 9));
    let info = MemInfo { mem_vt: EVT::I16, align: 2, volatile: false, base: MemBase::Unknown, ext: None, space: 259 , ordering: None };
    let root = if store { dag.get_store(chain, value, far, info) } else { dag.get_load(EVT::I16, chain, far, info) };
    let root = if store { root } else { SDValue { node: root.node, res: 1 } };
    dag.set_root(root);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    (dag, root, [offset, selector])
}

/// A load or store through a far pointer stopped the type legalizer ("Load:
/// expand operand", "Store: expand operand", the m16 `wc16_mixed` and
/// `ia16_abi` programs). The access is at the offset word and `ES` takes the
/// selector first.
#[test]
fn an_access_through_a_far_pointer_is_at_the_offset_after_es_takes_the_selector() {
    for store in [false, true] {
        let (dag, root, [offset, selector]) = legalized(store);
        let access = SDValue { node: root.node, res: 0 };
        let (ptr_at, kind) = if store { (2, ISD::Store) } else { (1, ISD::Load) };
        assert_eq!(dag.opcode(access), kind);
        assert_eq!(dag.operand(access, ptr_at), offset, "the access is at the offset word");
        let set_es = dag.operand(access, 0);
        assert_eq!(dag.opcode(set_es), ISD::CopyToReg, "ES is set before the access");
        assert_eq!(dag.node(dag.operand(set_es, 1)).data, NodeData::Register(ES));
        assert_eq!(dag.operand(set_es, 2), selector);
    }
}
