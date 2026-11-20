//! The 68000's full-unroll bar at -O2/-O3 is 400 (the pin's is 300, ours had
//! 32): `word_compares` -O3 left its 14-trip compare loop rolled at 26
//! instructions a trip, the 68000 running 8,628 clk where gcc's unrolled copy
//! ran 6,450. The 68020 keeps 32 (cache: unrolled, word_compares ran 49,581 ->
//! 61,030 clk), and -Os never unrolls it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const ORDERS: &str = include_str!("../data/unroll_bar_orders.ll");

fn instructions(
    cpu: &str,
    level: Level,
) -> usize {
    let asm = assembly(ORDERS, cpu_of(cpu).unwrap(), level).expect("compiles");
    asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

#[test]
fn a_fourteen_trip_loop_of_twenty_six_instructions_is_unrolled_on_the_68000_at_speed() {
    for level in [Level::O2, Level::O3] {
        let count = instructions("68000", level);
        assert!(count > 300, "68000 {level:?}: {count} instructions, the loop stayed rolled");
    }
}

#[test]
fn the_same_loop_stays_rolled_on_the_68020_and_at_os() {
    for level in [Level::O2, Level::O3] {
        let count = instructions("68020", level);
        assert!(count < 150, "68020 {level:?}: {count} instructions, the cache holds the rolled loop");
    }
    let count = instructions("68000", Level::Os);
    assert!(count < 150, "68000 -Os: {count} instructions, -Os unrolled the loop");
}
