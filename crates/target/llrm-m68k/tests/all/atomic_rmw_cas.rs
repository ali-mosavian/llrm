//! A 68020 has `cas`: a compare-exchange and a read-modify-write (a `cas` loop,
//! `AtomicExpandPass`) are inline there, as `shouldExpandAtomicRMWInIR` and
//! `ATOMIC_CMP_SWAP Legal` make them. They were `__sync_*` calls the QB runtime
//! has no `__sync_fetch_and_add_4` for (pmou.c: link error at every level); a
//! 68000 still calls them (no `cas`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn text(
    cpu: &str,
    level: Level,
) -> String {
    let source = include_str!("../data/atomic_rmw_cas.ll");
    assembly(source, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu}: {why}"))
}

#[test]
fn a_68020_read_modify_write_and_compare_exchange_use_cas() {
    for level in [Level::Os, Level::O2] {
        let text = text("68020", level);
        assert!(text.matches("cas.l").count() >= 5 && text.contains("cas.b"), "{text}");
        assert!(!text.contains("__sync"), "{text}");
    }
}

#[test]
fn a_68000_read_modify_write_is_still_the_sync_call() {
    let source = include_str!("../data/atomic_rmw_sync.ll");
    let text = assembly(source, cpu_of("68000").unwrap(), Level::Os).unwrap();
    assert!(text.contains("__sync_fetch_and_add_4") && !text.contains("cas."), "{text}");
}
