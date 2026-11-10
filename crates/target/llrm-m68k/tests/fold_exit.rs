//! With LoopTermFold on, the full m68k flow keeps the pointer-compare exit.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};
use llrm_m68k::machine::with_loop_term_fold;

/// `bench/library/partition.c` as clang emits it (-O1, passes disabled): the
/// bench `.ll` has no `inbounds`/`nuw`, so the fold cannot expand an exit value.
const PARTITION: &str = include_str!("fixtures/partition_clang_o1.ll");

/// Compares with an address register: the end pointer the fold tests against.
fn address_compares(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("cmp") && line.contains("%a")).count()
}

/// A second loop strength reduction after LoopTermFold rebuilt partition's
/// count-to-zero exit (`subq.l #4, %d2; bne`) from the end-pointer compare the
/// fold had written: fold on gave the same loop as fold off, 98 B at 68000 -O2
/// against gcc's 92 B, which ends on a compare with the end pointer.
#[test]
fn a_folded_exit_survives_to_the_assembly() {
    let off = assembly(PARTITION, Cpu::M68000, Level::O2).expect("compiles");
    let on = with_loop_term_fold(true, || assembly(PARTITION, Cpu::M68000, Level::O2)).expect("compiles");
    assert_eq!(address_compares(&off), 0, "the control already ends on a pointer compare:\n{off}");
    assert!(address_compares(&on) > 0, "no compare with an address register with the fold on:\n{on}");
}
