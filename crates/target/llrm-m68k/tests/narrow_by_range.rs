//! A byte-wide counter divided by another in a loop was `__udivsi3`: the loop's
//! exit test bounds both to 0..255, and LazyValueInfo carries no loop bound, so
//! the divide stayed 32 bits wide. div8_os 68000 -O2 ran 5,900,252 clocks;
//! narrowed it is `divu` (4,344,092, -26%).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const KERNEL: &str = include_str!("data/div_byte_counters.ll");

fn calls(
    cpu: &str,
    level: Level,
    routine: &str,
) -> usize {
    let asm = assembly(KERNEL, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
    asm.lines().filter(|line| line.trim_start().starts_with("jsr") && line.contains(routine)).count()
}

#[test]
fn a_divide_of_counters_proved_to_fit_a_byte_is_not_a_libcall_at_o2() {
    assert_eq!(calls("68000", Level::O2, "__udivsi3"), 0);
}

#[test]
fn the_libcall_stays_where_the_extensions_cost_more_bytes() {
    assert_eq!(calls("68000", Level::Os, "__udivsi3"), 1, "-Os keeps the smaller call");
    assert_eq!(calls("68020", Level::O2, "__udivsi3"), 0, "the 68020 has divu.l: nothing to narrow to, no call either");
}
