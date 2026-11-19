use super::actions::LoadExt;
use super::dump::normalized;
use super::*;

/// An extending load keeps its narrow memory type and prints the
/// extension as the pin does (`zext from i8`); a plain load prints
/// neither. Loads were never extending, so a combine could not make one.
#[test]
fn an_extending_load_dumps_its_extension() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = dag.get_global_address(EVT::I32, 0, 0);
    dag.global_names = vec!["g".to_string()].into();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Global(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let plain = dag.get_load(EVT::I8, entry, ptr, info);
    let wide = dag.get_ext_load(LoadExt::ZExtLoad, EVT::I32, entry, ptr, info);
    assert_ne!(plain, wide, "the extension is part of the node");
    assert_eq!(dag.value_type(wide), EVT::I32);
    let joined = dag.get_token_factor(&[SDValue { node: plain.node, res: 1 }, SDValue { node: wide.node, res: 1 }]);
    dag.set_root(joined);
    let dump = dag.dump();
    assert!(dump.contains("i32,ch = load<(load (s8) from @g), zext from i8>"), "{dump}");
    let flat = normalized(&dump);
    assert!(flat.contains("load<zext from i8>"), "{flat}");
    assert!(flat.contains("i8,ch = load t"), "{flat}");
}

/// `zext (freeze (load i8))` stayed a narrow load under a `zero_extend`, so a
/// wasm32 `i8` promoted to `i32` kept an `and 255` (bench_jpeg_fixed -O1). The
/// pin's `tryToFoldExtOfLoad` (`Frozen`) makes it `assertzext (freeze
/// (zextload))`.
#[test]
fn a_zero_extend_of_a_frozen_load_is_the_asserted_extending_load() {
    use super::dag_combiner::{CombineLevel, NoCombines, combine};

    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = dag.get_global_address(EVT::I32, 0, 0);
    dag.global_names = vec!["g".to_string()].into();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Global(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let load = dag.get_load(EVT::I8, entry, ptr, info);
    let frozen = dag.get_node(ISD::Freeze, EVT::I8, &[load]);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[frozen]);
    let out = dag.get_global_address(EVT::I32, 0, 0);
    let chain = SDValue { node: load.node, res: 1 };
    let store = dag.get_store(chain, wide, out, MemInfo { mem_vt: EVT::I32, ..info });
    dag.set_root(store);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let stored = dag.operand(dag.get_root(), 1);
    assert_eq!(dag.opcode(stored), ISD::AssertZext, "{}", dag.dump());
    let frozen = dag.operand(stored, 0);
    assert_eq!(dag.opcode(frozen), ISD::Freeze, "{}", dag.dump());
    let load = dag.operand(frozen, 0);
    assert_eq!(dag.opcode(load), ISD::Load, "{}", dag.dump());
    assert_eq!(dag.value_type(load), EVT::I32, "the load is the extending one:\n{}", dag.dump());
}
