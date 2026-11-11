//! A store loop became a `memset` call at `-Os`: three pushes, `jsr` and `lea`
//! (bench/library/memset 60 B against gcc's 44 B loop, memcpy 46 against 38).
//! `Tti::memory_idioms_at_optsize` keeps the loop in a size build; speed
//! builds still call the library.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const MEMSET: &str = include_str!("../bench/library/memset.ll");
const MEMCPY: &str = include_str!("../bench/library/memcpy.ll");

#[test]
fn a_size_build_keeps_its_fill_and_copy_loops() {
    for cpu in ["68000", "68020"] {
        for (name, ir, routine) in [("memset", MEMSET, "memset"), ("memcpy", MEMCPY, "memcpy")] {
            let out = assembly(ir, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
            assert!(!out.contains(&format!("jsr\t{routine}")), "{cpu} {name}: a library call at -Os:\n{out}");
        }
    }
}

#[test]
fn a_speed_build_still_calls_the_library_for_a_fill() {
    let out = assembly(MEMSET, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(out.contains("memset"), "-O2 no longer calls memset:\n{out}");
}
