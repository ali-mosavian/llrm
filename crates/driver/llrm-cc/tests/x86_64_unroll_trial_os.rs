//! x86-64 -Os: a two-trip loop of a double that folds when unrolled stayed a loop (138 B; gcc peels it to 19 B,
//! float_in_place_update.c). The -Os unroll trial (DEVIATE UNROLL-OS-TRIAL) is on at m64 and keeps the unroll where the
//! object is smaller. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_two_trip_double_loop_that_folds_is_unrolled_at_x86_64_os() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("unroll-trial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("u.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/unroll_trial_os.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("divsd"), "the loop stayed rolled:\n{text}");
}

/// An unroll that only removes a 16 B constant table grew the code 88 -> 98 B: the trial counted data as code
/// (`call_cleanup_keeps_pair.c`). The strict trial compares code alone.
#[test]
fn an_unroll_that_only_drops_constants_is_not_kept_at_x86_64_os() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("unroll-trial-code-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("u.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/unroll_trial_constants.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let sizes = Command::new("size").arg("-A").arg(&object).output().unwrap();
    let sizes = String::from_utf8_lossy(&sizes.stdout).into_owned();
    let text: u32 = sizes
        .lines()
        .find_map(|line| line.strip_prefix(".text"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no .text in:\n{sizes}"));
    assert!(text <= 88, ".text is {text} B, the rolled code is 88:\n{sizes}");
}
