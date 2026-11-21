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
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("counter-one-register");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fill_counter.c");
    let out = dir.join(format!("fill.{level}.s"));
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
