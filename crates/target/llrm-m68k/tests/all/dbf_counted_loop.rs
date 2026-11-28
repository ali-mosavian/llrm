//! A counted fill loop ended `addq.w #1,%d0; bne`: 14 clocks a trip on the
//! 68000 where `dbf` is 10 (mem_length_boundaries_m32 -Os: 4,242,970 clk
//! against gcc's 4,003,742). Counted loops of up to 65536 trips now end in
//! `dbf`; one of more (70000) must not, as `dbf` counts the low word only.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn body(
    source: &str,
    cpu: &str,
    level: Level,
) -> String {
    let asm = assembly(source, cpu_of(cpu).unwrap(), level).expect("compiles");
    let body = asm.split("fill:").nth(1).expect("the function is emitted");
    body.split("\n.size").next().unwrap_or(body).to_owned()
}

#[test]
fn a_counted_loop_of_20000_trips_ends_in_dbf_on_the_68000_and_68020() {
    let source = include_str!("../data/dbf_fill_20000.ll");
    for cpu in ["68000", "68020"] {
        let text = body(source, cpu, Level::Os);
        assert!(text.contains("dbf\t"), "{cpu}: no dbf:\n{text}");
        assert!(text.contains("#19999"), "{cpu}: the counter starts at the trips less one:\n{text}");
        assert!(!text.contains("bne"), "{cpu}: the old exit test stays:\n{text}");
    }
}

#[test]
fn the_68040_and_68060_gain_nothing_from_dbf_and_keep_the_loop() {
    let source = include_str!("../data/dbf_fill_20000.ll");
    for cpu in ["68040", "68060"] {
        let text = body(source, cpu, Level::Os);
        assert!(!text.contains("dbf"), "{cpu}:\n{text}");
    }
}

#[test]
fn a_loop_of_70000_trips_does_not_end_in_dbf() {
    let source = include_str!("../data/dbf_fill_70000.ll");
    for cpu in ["68000", "68020"] {
        let text = body(source, cpu, Level::Os);
        assert!(!text.contains("dbf"), "{cpu}: a word counter would run the loop 4464 times:\n{text}");
    }
}

/// The counter shares `%d0` with the result of the call after the loop: the
/// call's `%d0` is a new value, not a read of the counter, so the loop still
/// ends in `dbf` (the unit stayed a `bne` loop once allocation chose `%d0`).
#[test]
fn a_counter_in_the_result_register_ends_in_dbf_when_a_call_follows() {
    let source = include_str!("../data/dbf_fill_then_call.ll");
    for cpu in ["68000", "68020"] {
        let text = body(source, cpu, Level::Os);
        assert!(text.contains("dbf\t"), "{cpu}: no dbf:\n{text}");
    }
}
