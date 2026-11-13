//! `visitLIFETIME_END`: m32 `variadic` -Os kept two stores of the advanced
//! `va_list` pointer into a slot nothing reads (`sub esp,8; mov [esp],eax`
//! twice, +38 B against the pin's backend on the same IR); the pin removes a
//! store lying wholly inside an object just before its `LIFETIME_END`.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

/// A store of `value` to `slot + offset`, then the end of the slot's lifetime;
/// the slot is 8 bytes. The root after the combiner and whether the store
/// is still on its chain.
fn after_store_and_end(
    offset: i64,
    volatile: bool,
) -> (bool, SelectionDAG) {
    let mut dag = dag();
    dag.frame_object_bytes = vec![0, 0, 0, 8];
    let frame = dag.get_target_frame_index(EVT::I32, 3);
    let pointer = if offset == 0 {
        frame
    } else {
        let at = dag.get_constant(offset as u128, EVT::I32);
        dag.get_node(ISD::Add, EVT::I32, &[frame, at])
    };
    let value = dag.get_register(EVT::I32, 5);
    let entry = dag.entry_node();
    let info =
        MemInfo { mem_vt: EVT::I32, align: 4, volatile, base: MemBase::Stack(3), ext: None, space: 0, ordering: None };
    let store = dag.get_store(entry, value, pointer, info);
    let end = dag.get_node(ISD::LifetimeEnd, EVT::OTHER, &[store, frame]);
    dag.set_root(end);
    let mut hooks = X86Lowering {
        multiply_early_out: None,
        imul_if_smaller: false,
        hot_chains: false,
        share_products: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load: false,
        zext_free: false,
        branch_constant_selects: false,
        profile: None,
    };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let kept = dag.opcode(dag.operand(root, 0)) == ISD::Store;
    (kept, dag)
}

/// m32 `variadic` -Os kept the two stores of the advanced `va_list` pointer
/// into a slot nothing reads (+38 B against the pin's backend on the same IR).
#[test]
fn a_store_inside_the_object_just_before_its_lifetime_ends_is_removed() {
    for offset in [0, 4] {
        let (kept, dag) = after_store_and_end(offset, false);
        assert!(!kept, "offset {offset}");
        assert_eq!(dag.opcode(dag.get_root()), ISD::LifetimeEnd);
    }
}

/// A store at the object's end or a volatile one is observable; removing
/// either drops a write.
#[test]
fn a_store_past_the_object_or_a_volatile_one_stays() {
    assert!(after_store_and_end(8, false).0, "a store at the object's end is outside it");
    assert!(after_store_and_end(0, true).0, "a volatile store stays");
}
