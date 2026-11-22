//! BACKTHREAD-REL on the m68k. `word_compare_branches` asks eight compares of
//! one pair of words (`a < b`, `a <= b`, ... signed and unsigned) one `if` after
//! the other. Each `if` joins a path that decides the next and one that does
//! not, so no path knew the next compare and every path ran all eight: 57,727 clk
//! at 68020 -O2 against gcc's 46,450 (`threadfull1` and `vrp1` thread each
//! deciding path). Threading by relation leaves 50,710 clk and 8 B fewer; the
//! module is the driver's input for the program at -O2.

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_ladder_of_compares_of_one_pair_is_threaded_by_relation() {
    let module = include_str!("../fixtures/word_compare_branches_o2.ll");
    let out = assembly(module, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    // 113 lines of assembly without the threading, 106 with it.
    let lines = out.lines().count();
    assert!(lines <= 110, "{lines} lines, the compares were not threaded:\n{out}");
}

/// The 68000 is not threaded: its full-unroll bar of 400 would unroll the
/// smaller loop 14 times (276 -> 2,224 B at -O2 for clocks the harness can only
/// bound).
#[test]
fn the_68000_keeps_the_loop_rolled() {
    let module = include_str!("../fixtures/word_compare_branches_o2.ll");
    let out = assembly(module, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    let lines = out.lines().count();
    // 115 lines rolled; threaded, the loop is unrolled to over 800.
    assert!(lines <= 200, "{lines} lines, the 68000 loop was unrolled:\n{out}");
}
