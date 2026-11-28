//! The exit test of a counted loop and the counters LSR builds for it, built
//! with the C driver at -O2.

mod common;

use std::process::Command;

fn assembly_of(
    name: &str,
    source: &str,
    flags: &[&str],
) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lsr-exit-test").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.c"));
    std::fs::write(&file, source).unwrap();
    let made = Command::new(common::pipeline::llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O2", "-S", "-w", "-o", "o.s"])
        .args(flags)
        .arg(&file)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(dir.join("o.s")).unwrap()
}

/// `buf[(i * 5 + 3) & 63]` over 300 trips kept a 16-bit counter beside the
/// `i * 5 + 3` counter (its high byte stepped each trip and never read) and
/// tested the exit on the first: llvm-mos tests it on the second, `cpy #223`
/// and `cmp #5`, one counter (ring: 41,773 clocks against 30,184, 2026-11-22).
/// It takes the 16-bit counter's test on i16 (`n8` made it illegal for
/// `IndVarSimplify`'s test replacement) and an exit compare the cost model
/// does not rate by the bits of its constant.
#[test]
fn the_exit_of_a_masked_index_loop_is_tested_on_the_index_counter() {
    let text = assembly_of(
        "ring_b",
        "long buf[64];\nlong sum(void)\n{\n    long total = 0;\n    int i;\n    for (i = 0; i < 300; ++i) total += buf[(i * 5 + 3) & 63];\n    return total;\n}\n",
        &["-fno-inline-functions"],
    );
    let body = text.split("sum:").nth(1).unwrap_or_else(|| panic!("{text}"));
    let body = body.split(".segment").next().unwrap();
    assert!(body.contains("cpy #223"), "the exit is not tested on the {{3,+,5}} counter:\n{body}");
    assert_eq!(body.matches("adc #5").count(), 1, "{body}");
}

/// A dense switch's table lookup, run on 28 of the 256 trips, made LSR step
/// four counters (`m - 192`, the table pointer, `m * 16`, `-m`) every trip and
/// shift `m` into `m * 16` before the loop: 247 B and 20,829 clocks where the
/// counter alone, and the arithmetic at the lookup, are 164 B and 18,883
/// (switch_peeled_table -O2, with the exit test taken on `m`). A register
/// whose uses do not run every trip pays its step more than once.
#[test]
fn a_counter_for_a_use_that_runs_on_a_few_trips_is_not_made() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/switch_peeled_table.c"),
    )
    .unwrap();
    let text = assembly_of("peeled", &source, &["-fsigned-char", "-fwhole-program"]);
    let steps = text.matches("\n  inc rc").count();
    assert!(steps <= 2, "{steps} counters are stepped each trip, two are the loop's:\n{text}");
}

/// A counter for arithmetic that runs on some trips (the switch arms of
/// `byte` in a loop) was priced like one for an address and built eight
/// registers' worth of counters: `main` 884 B and 6,722 clocks where it is
/// 771 B and 5,957 (switch_jump_table -O2, 2026-11-22).
#[test]
fn arithmetic_on_some_trips_does_not_get_a_counter() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/switch_jump_table.c"),
    )
    .unwrap();
    let text = assembly_of(
        "jump_table",
        &source,
        &["-fsigned-char", "-fwhole-program", "-fno-unroll-loops", "-fno-peel-loops"],
    );
    let pushes = text.matches("\n  pha").count();
    assert!(pushes <= 5, "{pushes} cells are saved and the loop's counters live in them:\n{text}");
}

/// `buf[i] = (long)(i + 1) * 3 - 7 + n` over 64 trips stored through a pointer
/// that stepped by 4 with a carry into its high byte, `ldy #k` before each of
/// the four stores (63 instructions); llvm-mos stores `sta buf,y` on one byte
/// index (ring loop 1, 2026-11-22).
#[test]
fn a_store_through_a_stepping_array_index_uses_a_byte_index() {
    let text = assembly_of(
        "ring_a",
        "long buf[64];\nvoid fill(long n)\n{\n    int i;\n    for (i = 0; i < 64; ++i) buf[i] = (long)(i + 1) * 3 - 7 + n;\n}\n",
        &["-fno-inline-functions"],
    );
    let body = text.split("\nfill:").nth(1).unwrap_or_else(|| panic!("{text}"));
    let body = body.split(".segment").next().unwrap();
    for store in ["sta buf,y", "sta buf+1,y", "sta buf+2,y", "sta buf+3,y"] {
        assert!(body.contains(store), "no `{store}`:\n{body}");
    }
    assert!(!body.contains("),y"), "a store goes through a pointer cell:\n{body}");
}
