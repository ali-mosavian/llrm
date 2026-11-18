//! The shared `MachineCSE` shares the quotient and the zero of a loop's
//! counters across its calls, and they spill: `c_ushort_3` of
//! div_constants_short -Os on the 68020 grew from 320 to 336 B with it on.
//! A function it changed is built both ways and the smaller kept.
//! `div_ushort3_raw.ll` is clang's output with no passes.

use llrm_m68k::compile::{Options, items_bytes, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for};

const CASE: &str = include_str!("fixtures/div_ushort3_raw.ll");

#[test]
fn a_function_the_shared_cse_makes_larger_is_built_without_it() {
    let module = lowered_for(CASE, Cpu::M68020, Level::Os).expect("lowers");
    let bytes = |options| items_bytes(&items_with(&module, "c_ushort_3", Cpu::M68020, options).expect("compiles"));
    let size = Options { size: true, mid_end: false, ..Options::default() };
    let without = bytes(Options { shared_cse: false, ..size });
    let with = bytes(size);
    assert!(with <= without, "shared_cse default: {with} B, without: {without} B");
}
