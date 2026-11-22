//! A counted loop whose compare reads the counter before its increment keeps
//! the counter in one index register: `TwoAddress` moves the compare above the
//! tied `inc` (`rescheduleKillAboveMI`), so the old value dies there. Before,
//! the loop copied the counter to `y` every trip (`txa; tay; ...; cpy; inx`:
//! 18 clocks a trip against 12 and 4 more bytes, `long_double_constant_store`).
//! Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    assembly_of("tests/data/fill_counter.c", level)
}

fn assembly_of(
    source: &str,
    level: &str,
) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("counter-one-register");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
    let out = dir.join(format!("{}.{level}.s", src.file_stem().unwrap().to_string_lossy()));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_counter_compared_before_its_increment_moves_between_no_registers() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(level);
        let body = &text[text.find("\nfill:").expect("fill")..];
        let body = body.split("\nfill_b3:").next().unwrap();
        assert!(body.contains("sta junk,"), "{level}: no indexed store (premise):\n{body}");
        for transfer in ["tax", "tay", "txa", "tya"] {
            assert!(
                !body.contains(&format!("  {transfer}\n")),
                "{level}: the counter is copied to a second register:\n{body}"
            );
        }
    }
}

/// `inc` of `a` is `clc; adc #1` on the 6502 and writes carry: the compare
/// moved above that step lost its flags and `bcs` read the step's carry, 256
/// trips (`counter_compare_after_step`, first version of the move). The exit
/// is tested either on the old value before the step (`cmp #11; bcs`) or on
/// the new one after it (`cmp #12; beq`, the exit-compare immediate is free);
/// never `bcs` on the step's carry.
#[test]
fn a_compare_is_not_moved_above_a_step_that_writes_carry() {
    for level in ["-O2", "-Os"] {
        let text = assembly_of("../../../tests/run/c/counter_compare_after_step.c", level);
        // the counter in `a` is compared with `cmp`; in `x` or `y` (`inx`/`iny` keep carry) with `cpx`/`cpy`
        let ops = ["cmp", "cpx", "cpy"];
        let before = ops.iter().any(|op| text.contains(&format!("  {op} #11\n  bcs")));
        let steps = ["adc #1", "inx", "iny"];
        let after = steps
            .iter()
            .any(|step| ops.iter().any(|op| text.contains(&format!("{step}\n  {op} #12\n  beq"))));
        assert!(before || after, "{level}: the loop's compare is gone:\n{text}");
        let stale = ops.iter().any(|op| text.contains(&format!("adc #1\n  {op} #11\n  bcs")));
        assert!(!stale, "{level}: `bcs` reads the step's carry:\n{text}");
    }
}
