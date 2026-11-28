//! x86-64 -O1 tries the inlines its size bound refused (DEVIATE
//! INL-O1-TRIAL-M64): uint64_to_float.c kept six calls of `classify` and its
//! conversions, 280 B where clang's folded program is 65 B. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_call_that_shrinks_the_caller_is_inlined_at_x86_64_o1() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inline-o1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/inline_o1_trial.c"))
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
    assert!(text <= 100, ".text is {text} B, the folded program is 65:\n{sizes}");
}
