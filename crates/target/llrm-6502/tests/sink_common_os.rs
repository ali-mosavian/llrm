//! -Os sinks common code in every SimplifyCFG (`Tuning::sink_common_at_size`,
//! gcc's -fcrossjumping): the two arms of the recursion's `t + s` share the
//! add, which becomes one add of two phis (`%sink1`). The 6502 left the flag
//! off, and the arms kept an add each at -Os (QB m32 shrank 23 KB with it).
//! The assertion is on the IR the passes leave. Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn stages(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sink-common{level}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/recursive_stack_args.c");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .env("LLRM_MIR_STAGES", &dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(dir.join("o.s"))
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut text = String::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ll") {
            text += &std::fs::read_to_string(path).unwrap();
        }
    }
    text
}

#[test]
fn os_sinks_the_common_add_of_two_arms_and_the_speed_levels_do_not() {
    assert!(stages("-Os").contains("%sink"), "-Os: the common add was not sunk");
    assert!(!stages("-O2").contains("%sink"), "-O2: code was sunk (premise: the speed levels keep the pin's default)");
}
