//! x86-64 -O1 takes `-funroll-loops` (DEVIATE UNROLL-O1) where code never
//! grows: a two-trip double loop that folds is unrolled on trial
//! (float_in_place_update.c: 145 B, gcc 28), and a loop that would only be
//! copied is not (a 64-trip dot product grew 125 -> 246 B when the pin's
//! partial unroll ran). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn compile(
    source: &str,
    mode: &str,
) -> std::path::PathBuf {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("unroll-o1-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.out");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", mode, "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    asm
}

fn assembly(source: &str) -> String {
    std::fs::read_to_string(compile(source, "-S")).unwrap()
}

fn text_bytes(source: &str) -> u32 {
    let object = compile(source, "-c");
    let sizes = Command::new("size").arg("-A").arg(&object).output().unwrap();
    let sizes = String::from_utf8_lossy(&sizes.stdout).into_owned();
    sizes
        .lines()
        .find_map(|line| line.strip_prefix(".text"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no .text in:\n{sizes}"))
}

#[test]
fn a_two_trip_double_loop_that_folds_is_unrolled_at_x86_64_o1() {
    let text = assembly("unroll_trial_os.c");
    assert!(!text.contains("divsd"), "the loop stayed rolled:\n{text}");
}

#[test]
fn a_loop_that_would_only_be_copied_stays_rolled_at_x86_64_o1() {
    let text = text_bytes("unroll_o1_partial.c");
    assert!(text <= 140, ".text is {text} B, the rolled code is 137");
}
