//! `nbody_single` -O3 as the mid-end leaves it (`data/nbody_single_o3.ll`): a 108-node region of float
//! arithmetic that the pin's scheduler keeps below the register file by picking from the bottom, and a
//! scheduler that picked from the top for pressure spilled 247 float values to the stack against 200.
//!
//! Counts are stack loads and stores in `bench_nbody_single` (`-S -O3`). Measured with each translation
//! left out of the same binary: both in place 200; without the ExitSU edge (`buildSchedGraph`'s latency to the
//! region end, a def nothing reads) 215; with `tryPressure` comparing the size of a rise between a top and a
//! bottom candidate 247. The limit sits between 200 and 215.

use std::process::Command;

fn spill_operations() -> usize {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let input = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/nbody_single_o3.ll");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O3", "-o"])
        .arg(&output)
        .arg(input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = std::fs::read_to_string(&output).unwrap();
    let body = text.split("bench_nbody_single:").nth(1).expect("the kernel is in the output");
    let body = body.split(".Lfunc_end").next().unwrap();
    body.lines()
        .filter(|line| {
            let mut words = line.split_whitespace();
            matches!(words.next(), Some("ldr" | "str" | "ldp" | "stp")) && line.contains("[sp")
        })
        .count()
}

#[test]
fn the_unrolled_nbody_loop_spills_no_more_than_the_pin_s_picks_allow() {
    let spills = spill_operations();
    assert!(spills <= 205, "{spills} stack loads and stores in bench_nbody_single (200 expected)");
}
