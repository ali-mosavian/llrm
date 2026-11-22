//! jpeg_fixed -Os 68000 (+172 B against gcc): `put32` (a 4-trip store loop, eight cold sites) and `next16` were copied
//! into `bench_jpeg_fixed`. The units price a copy at 15 against a call at 18; the backend writes the copies bigger
//! than the callee and the calls (-34 B kept as calls). A cold site the units accept is built as a call too, and the
//! smaller finished code is kept.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// The cold callees the units accept stay calls when the copies are bigger.
#[test]
fn a_cold_callee_the_units_accept_stays_a_call_when_the_copies_are_bigger() {
    let ir = include_str!("../data/jpeg_fixed_bench.ll");
    let asm = assembly(ir, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    for callee in ["put32", "next16"] {
        assert!(asm.contains(&format!("{callee}:")), "{callee} was copied into its sites");
    }
}
