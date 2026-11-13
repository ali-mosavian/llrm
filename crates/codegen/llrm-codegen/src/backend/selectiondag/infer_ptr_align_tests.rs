use super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::*;

fn load_at(
    dag: &mut SelectionDAG,
    address: SDValue,
    align: u32,
) -> SDValue {
    let entry = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::I32,
        align,
        volatile: false,
        base: MemBase::Value(std::num::NonZeroU32::new(1).unwrap()),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.mem_locations.push(MemLocation { pointer: llrm_mir::module::Operand::Block(llrm_mir::module::BlockId(0)), tbaa: None, bytes: 4 });
    let load = dag.get_load(EVT::I32, entry, address, info);
    let root = dag.get_node(ISD::Add, EVT::I32, &[load, load]);
    let store = dag.get_store(SDValue { node: load.node, res: 1 }, root, address, info);
    dag.set_root(store);
    load
}

fn align_of(
    dag: &SelectionDAG,
    node: u32,
) -> u32 {
    match dag.node(SDValue { node, res: 0 }).data {
        NodeData::Mem(info) => info.align,
        _ => panic!("not an access"),
    }
}

/// A load annotated `align 1` of a global aligned to 16, 8 bytes in, is
/// 8-aligned (`SelectionDAG::InferPtrAlign`, applied by `visitLOAD`): left at
/// 1, `memory_expansion` and the loads after it stayed unaligned where the pin
/// has the global's alignment.
#[test]
fn a_load_through_an_aligned_global_gets_the_inferred_alignment() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    dag.global_names = vec!["g".to_string()].into();
    dag.global_alignments.insert(0, 16);
    let address = dag.get_global_address(EVT::I32, 0, 8);
    let load = load_at(&mut dag, address, 1);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    assert_eq!(align_of(&dag, load.node), 8, "{}", dag.dump());
}

/// The same through a frame object: `FrameIndex + 4` of a 16-aligned object is
/// 4-aligned.
#[test]
fn a_load_through_a_frame_object_gets_the_inferred_alignment() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    dag.frame_alignments = vec![16];
    let frame = dag.get_frame_index(EVT::I32, 0);
    let four = dag.get_constant(4, EVT::I32);
    let address = dag.get_node(ISD::Add, EVT::I32, &[frame, four]);
    let load = load_at(&mut dag, address, 1);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    assert_eq!(align_of(&dag, load.node), 4, "{}", dag.dump());
}
