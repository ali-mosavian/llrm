//! A call contract was found by parsing the 64 KB runtime table (8 M instructions, 3% of a typical C compile, and the same for
//! a BASIC program that asks of one routine). The table is Rust data from the build; a lookup builds the one row it wants.
//! Alone in its process: no other test may have forced the whole table first.

use std::sync::LazyLock;

#[test]
fn a_lookup_builds_one_row_and_parses_nothing() {
    let by_call = llrm_qbruntime::per_call(&[(0, "bench_nbody_fixed_".to_owned()), (1, "B$SPAC".to_owned())].into_iter().collect(), "vbdos", &Default::default());
    assert_eq!(by_call[&0], llrm_qbruntime::worst("bench_nbody_fixed_"));
    assert!(by_call[&1].evidence.starts_with("VBDCL10E.LIB"), "a routine with a family variant takes it");
    assert_eq!(llrm_qbruntime::contract(Some("_main")), llrm_qbruntime::worst("_main"));
    assert_eq!(llrm_qbruntime::contract(Some("B$HARY")).name, "B$HARY");
    assert!(LazyLock::get(&llrm_qbruntime::CONTRACTS).is_none(), "the whole table was built");
}
