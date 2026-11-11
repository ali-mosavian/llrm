//! `pha ... pla` around code that needs `a` while it holds a live value:
//! `known_loads_walks` -O1 spent 41,552 of 250,190 clocks in them (2026-11-11).
//! Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(source: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("brackets-of-a");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(source);
    let out = dir.join(format!("{source}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O1", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// Two literals stored into cells while `a` is live were two brackets, the
/// second saving what the first had restored (24 clocks, now 15).
#[test]
fn two_literal_stores_with_a_live_share_one_bracket() {
    let text = assembly("a_live_two_literals.c");
    assert!(!text.contains("  pla\n  pha\n"), "a restore and a save back to back:\n{text}");
    assert!(text.matches("  pha\n").count() <= 1, "{text}");
}

/// `x` to `y` or back with `a` live went through `a` between a `pha` and a
/// `pla` (11 clocks); a cell nothing holds does it in 6.
#[test]
fn an_index_copy_with_a_live_does_not_go_through_the_stack() {
    let text = assembly("a_live_index_copy.c");
    for through in ["  pha\n  tya\n  tax\n  pla\n", "  pha\n  txa\n  tay\n  pla\n"] {
        assert!(!text.contains(through), "{text}");
    }
}
