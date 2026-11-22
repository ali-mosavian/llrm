//! A monotonic atomic load or store of at most 32 bits is a plain `move`
//! (M68kInstrAtomics.td), a read-modify-write or compare-exchange a
//! `__sync_*` call: QuickrBasic `pmou.c` was refused "instruction load".

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_monotonic_atomic_load_and_store_are_moves() {
    let source = include_str!("../data/atomic_load_store.ll");
    for cpu in ["68000", "68020"] {
        let text = assembly(source, cpu_of(cpu).unwrap(), Level::O2).unwrap_or_else(|why| panic!("{cpu}: {why}"));
        assert!(text.contains("flag"), "{cpu}: no access:\n{text}");
        assert!(text.contains("jsr\t__sync_fetch_and_add_4"), "{cpu}: no add routine:\n{text}");
        assert!(text.contains("jsr\t__sync_val_compare_and_swap_4"), "{cpu}: no cas routine:\n{text}");
    }
}
