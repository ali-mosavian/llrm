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
