//! A C function's call contract was found by parsing the 64 KB runtime table (8 M instructions, 3% of a typical C compile) only
//! to learn that the table has no row for it. Alone in its process: no other test may have forced the table first.

use std::sync::LazyLock;

#[test]
fn a_name_outside_the_runtime_is_worst_case_without_parsing_the_table() {
    let by_call = llrm_qbruntime::per_call(&[(0, "bench_nbody_fixed_".to_owned())].into_iter().collect(), "", &Default::default());
    assert_eq!(by_call[&0], llrm_qbruntime::worst("bench_nbody_fixed_"));
    assert_eq!(llrm_qbruntime::contract(Some("_main")), llrm_qbruntime::worst("_main"));
    assert!(LazyLock::get(&llrm_qbruntime::CONTRACTS).is_none(), "the contract table was parsed");
    assert!(LazyLock::get(&llrm_qbruntime::VARIANTS).is_none(), "the variants were built");
}
