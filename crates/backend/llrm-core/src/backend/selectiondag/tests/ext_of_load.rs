use super::super::actions::LoadExt;
use super::super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    volatile: bool,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I8, align: 1, volatile, base: MemBase::Stack(1), ext: None };
    dag.get_load(EVT::I8, chain, ptr, info)
}

fn folded(
    op: ISD,
    volatile: bool,
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let byte = load(&mut dag, volatile);
    let wide = dag.get_node(op, EVT::I32, &[byte]);
    dag.set_root(wide);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    (dag, root)
}

fn ext_of(
    dag: &SelectionDAG,
    load: SDValue,
) -> (EVT, EVT, Option<LoadExt>) {
    let NodeData::Mem(info) = dag.node(load).data else { panic!("a load") };
    (dag.value_type(load), info.mem_vt, info.ext)
}

/// `(zext (load i8 p))` stayed a load and a `zext`, so selection emitted
/// `mov al, [p]` and `movzx`, where the pin has one `movzx eax, byte [p]`
/// (10 pin-only MOVZX/MOVSX rows of the m32 shadow).
#[test]
fn an_extension_of_a_load_is_an_extending_load() {
    for (op, ext) in
        [(ISD::ZeroExtend, LoadExt::ZExtLoad), (ISD::SignExtend, LoadExt::SExtLoad), (ISD::AnyExtend, LoadExt::ExtLoad)]
    {
        let (dag, root) = folded(op, false);
        assert_eq!(dag.opcode(root), ISD::Load, "{op:?}");
        assert_eq!(ext_of(&dag, root), (EVT::I32, EVT::I8, Some(ext)), "{op:?}");
    }
}

/// A volatile load keeps its width: the extension stays a node.
#[test]
fn a_volatile_load_is_not_extended() {
    let (dag, root) = folded(ISD::ZeroExtend, true);
    assert_eq!(dag.opcode(root), ISD::ZeroExtend);
}

/// The load's chain users follow the new load, or the load could be
/// scheduled after what it must precede.
#[test]
fn the_chain_of_the_old_load_moves_to_the_extending_load() {
    let mut dag = dag();
    let byte = load(&mut dag, false);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[byte]);
    let chain = SDValue { node: byte.node, res: 1 };
    let ptr = dag.get_register(EVT::I32, 8);
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(2), ext: None };
    let store = dag.get_store(chain, wide, ptr, info);
    dag.set_root(store);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    let ordering = dag.operand(root, 0);
    assert_eq!((dag.opcode(ordering), ordering.res), (ISD::Load, 1));
    assert_eq!(ext_of(&dag, SDValue { node: ordering.node, res: 0 }).2, Some(LoadExt::ZExtLoad));
}
