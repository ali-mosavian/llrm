//! Fourteen unsigned/signed compare groups over volatile loads (a reduction of
//! word_compare_branches) compiled to 300 instructions at `-O3`: the
//! scheduler's backtrack accepted a candidate whose flag producer reaches the
//! register it forces first (`WillCreateCycle` checks the assigned-register
//! predecessors too), re-cloned the flag setters and loaded everything up
//! front. The pin's count on the same IR is 288. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn instructions() -> usize {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-flag-schedule-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/flag_interference_schedule.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).lines().filter(|l| l.split_once(":\t").is_some()).count()
}

#[test]
fn compare_groups_schedule_in_no_more_instructions_than_the_pin() {
    let count = instructions();
    assert!(count <= 288, "{count} instructions, the pin makes 288");
}
