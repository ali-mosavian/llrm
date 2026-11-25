//! x86-32 -Os: the keep-hinted-calls trial found the smaller object (-428 B of
//! 717 in all) and put it back because the calls add work over the 2% clock cap
//! of `os_trial_clock_cap_percent`. Bytes decide for a hinted callee, as in
//! gcc, which keeps the calls. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn text_bytes() -> u64 {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-hint-uncapped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/os_hint_trial_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let headers = Command::new("objdump").args(["-h"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&headers.stdout)
        .lines()
        .find(|line| line.split_whitespace().nth(1) == Some(".text"))
        .and_then(|line| u64::from_str_radix(line.split_whitespace().nth(2)?, 16).ok())
        .expect(".text size")
}

#[test]
fn a_hinted_callee_with_many_sites_stays_a_call_where_the_object_is_smaller() {
    let bytes = text_bytes();
    assert!(bytes <= 300, "{bytes} bytes of .text: the 15 copies were kept (637 B), the calls are 209 B (gcc 236 B)");
}
