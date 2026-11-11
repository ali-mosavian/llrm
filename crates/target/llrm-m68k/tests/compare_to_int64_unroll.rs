//! `tests/run/c/compare_to_int64.c` at -O3, as the driver gets it (`clang -O3
//! -Xclang -disable-llvm-passes`): nested 3x3 loops of `long long` sums. The
//! pin folds the program to two calls of `report`. Loop unswitching freezes
//! the hoisted guard `icmp eq %iv, 0`; the pin's InstCombine
//! (`foldFreezeIntoRecurrence`) drops the freeze and the `nsw` of the outer
//! step, which lets the unrolled inner copies fold against the guard. Without
//! that fold 171 instructions stayed (the outer loop priced 59 and 67 against
//! the pin's 51 and 37).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

fn instructions(cpu: Cpu) -> usize {
    let ir = include_str!("data/compare_to_int64_o3.ll");
    let out = assembly(ir, cpu, Level::O3).expect("compiles");
    out.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

#[test]
fn nested_loops_under_a_frozen_guard_fold_whole() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let count = instructions(cpu);
        assert!(count < 20, "{cpu:?}: {count} instructions, the loops stay");
    }
}
