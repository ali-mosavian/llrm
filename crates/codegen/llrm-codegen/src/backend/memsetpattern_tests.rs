/// The shadow run tries the DAG path on every function, so it needs the same
/// expanded input as `--isel=dag`: it was handed the intrinsic and 9 functions
/// of the m32 corpus (pattern_fill, fill_nest, idioms, loop_fusion, huge/fillw)
/// were refused `intrinsic call llvm.experimental.memset.pattern`.
#[test]
fn the_shadow_run_expands_patterns_as_the_dag_run_does() {
    use crate::backend::memsetpattern::expands_for;
    use crate::backend::shadow::Selector;
    assert!(expands_for(Selector::Dag) && expands_for(Selector::Shadow));
    assert!(!expands_for(Selector::Old), "the old selector keeps the intrinsic");
}
