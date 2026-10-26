use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A load through `gs` (address space 256) is not the load through the same
/// pointer in space 0: the matcher picks a segment override by it. Without
/// the field CSE merged them into one near load.
#[test]
fn a_load_in_another_address_space_is_another_node() {
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let near = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Unknown, ext: None, space: 0 };
    let far = MemInfo { space: 256, ..near };
    let (a, b) = (dag.get_load(EVT::I32, chain, ptr, near), dag.get_load(EVT::I32, chain, ptr, far));
    assert_ne!(a, b);
    assert_eq!(a, dag.get_load(EVT::I32, chain, ptr, near));
    let joined = dag.get_token_factor(&[SDValue { node: a.node, res: 1 }, SDValue { node: b.node, res: 1 }]);
    dag.set_root(joined);
    assert!(dag.dump().contains("(load (s32) from unknown-address, addrspace 256)"), "{}", dag.dump());
}
