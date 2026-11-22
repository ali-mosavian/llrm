//! BACKTHREAD-REL. A ladder of compares of one pair (`a < b`, `a <= b`, `a > b`,
//! `a >= b`, as `word_compare_branches` has) was four compares on every path:
//! each `if` is a join of a path that decides the next and one that does not,
//! so no edge knew the next compare. gcc's `threadfull1` and `vrp1` thread each
//! deciding path (68020 -O2 `word_compare_branches`: 57,727 clk against gcc's
//! 46,450). Threading by relation makes the ladder one compare, then one more
//! on the path that is not `a < b`.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn compares(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("cmp")).count()
}

#[test]
fn probe() {
    let module = include_str!("../fixtures/word_compare_branches_o2.ll");
    let out = assembly(module, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    panic!("compares={} lines={}", compares(&out), out.lines().count());
}
