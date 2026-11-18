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

/// `u - (u / v) * v` of two byte counters kept `__mulsi3` for `q * v` (u8b: 1
/// call per trip, 68000 -O2): the quotient is a byte, the counter a
/// range-proved byte, and the narrowing wrapper the pipeline made was folded
/// back into the counter by the target's own strength reduction before
/// selection. Made after it, selection sees two halves and emits `mulu`.
#[test]
fn a_quotient_times_a_counter_is_a_half_multiply_at_o2() {
    assert_eq!(calls("68000", Level::O2, "__mulsi3"), 0);
    let asm = assembly(KERNEL, cpu_of("68000").unwrap(), Level::O2).unwrap();
    assert!(asm.lines().any(|line| line.trim_start().starts_with("mulu")), "{asm}");
}

/// #9726 narrows a proven 16-bit multiply at every level: a `mulu` (2 B) where the call is two pushes, a `jsr` and a
/// pop. The kernel calls `sink` each trip: without it -Os evaluates the whole loop to a constant (8 B) and no
/// multiply is left to look at.
#[test]
fn a_proven_halfword_multiply_is_a_mulu_at_size_and_o1() {
    for level in [Level::Os, Level::O1] {
        assert_eq!(calls("68000", level, "__mulsi3"), 0, "{level:?}");
        let asm = assembly(KERNEL, cpu_of("68000").unwrap(), level).unwrap();
        assert!(asm.lines().any(|line| line.trim_start().starts_with("mulu")), "{level:?}:\n{asm}");
    }
}
