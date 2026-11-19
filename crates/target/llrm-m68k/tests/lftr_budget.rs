//! At a smaller size `IndVarSimplify` rewrote `i + 1 < n` to `i + 1 == smax(n,
//! 1)`: a guard around the loop and a count-down register for 38 B, against
//! gcc's 22 B for the compare (bench/kernels/fib). The limit costs an
//! operation, so the size build keeps the compare.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const FIB: &str = include_str!("../bench/kernels/fib.ll");

fn at(level: Level) -> String {
    assembly(FIB, cpu_of("68000").unwrap(), level).expect("compiles")
}

#[test]
fn a_size_build_keeps_the_loop_compare_and_has_no_guard() {
    let out = at(Level::Os);
    // The argument's offset depends on the registers the function saves (8 with a pushed `%d2`, 4 without).
    let compares_with_the_argument =
        out.lines().any(|line| line.starts_with("\tcmp.l\t(") && line.contains(",%sp), %d"));
    assert!(compares_with_the_argument, "the loop does not compare with the argument:\n{out}");
    assert!(!out.contains("cmpi.l\t#1"), "a guard around the loop:\n{out}");
}

/// The speed build used to keep the guard at the 68000 (budget 4); an `smax`
/// limit is not worth a second counter at any CPU or level. The test once
/// looked for `cmpi.l #1`, which the output no longer spells.
#[test]
fn a_speed_build_keeps_the_loop_compare_too() {
    let out = at(Level::O2);
    assert!(out.contains("cmp.l"), "the loop is not a compare:\n{out}");
    assert!(!out.contains("subq.l\t#1, %d"), "a second counter beside the limit:\n{out}");
}
