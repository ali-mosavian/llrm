//! The unroll trial prices the runtime routines a text links. Needs `llrm-cc`
//! built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

/// `tail_calls` at -Os: `framed`'s rolled loop calls `__mulsi3_fast_byte`. The
/// trial counted the loop's code but not the routine, so the unrolled constants
/// (kept at -6 B) lost to the rolled loop (+10 B) once the add-from-memory
/// forms shrank it; linked, the rolled program was 89 B and 37% clocks worse.
#[test]
fn the_unroll_trial_counts_the_routine_a_rolled_loop_calls() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("trial-counts-runtime");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/tail_calls.c");
    let out = dir.join("tail_calls.s");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args([
            "--target=6502-sim65",
            "-Os",
            "-fwhole-program",
            "-fno-inline-functions",
            "-fno-inline-functions-called-once",
        ])
        .args(["-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let calls = text.lines().filter(|l| l.contains("jsr __mulsi3")).count();
    assert_eq!(calls, 0, "{calls} multiply calls: the loop stayed rolled");
}
