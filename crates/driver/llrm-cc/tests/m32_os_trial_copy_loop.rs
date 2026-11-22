//! x86-32 -Os: x_transpose copies a 32-word row back as a loop (7 instructions a word) where the pin calls `memcpy`:
//! 142,340 clocks against clang's 122,625 (+16%) for 23 B. `KeepCopyLoop` priced the call as one unit and skipped the
//! clock cap, so the smaller loop always won. Both forms are now priced at the loop's own trip weight with the call's
//! `rep movsd` cost, and the loop stays only within the clock cap. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn asm_of(source: &Path) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-copy-loop-{}-{}", std::process::id(), source.as_os_str().len()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn a_row_copy_of_run_time_length_is_the_memcpy_call_where_the_call_runs_less() {
    let text = asm_of(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/llrm-x86-m32/vsgcc/kernels/x_transpose/x_transpose.c"));
    assert!(text.contains("call memcpy"), "the row copy stayed a loop:\n{text}");
}

/// The other side: a copy of a known small count is no call at any size (the backend moves the words inline), and
/// pricing the call at the unknown-count weight must not make one.
#[test]
fn a_copy_of_three_known_words_is_never_a_memcpy_call() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-copy-three-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("copy3.c");
    std::fs::write(&path, "extern int s[3], d[3];\nvoid copy3(void) { int i; for (i = 0; i < 3; i++) d[i] = s[i]; }\n").unwrap();
    let text = asm_of(&path);
    assert!(!text.contains("call memcpy"), "a three-word copy became a call:\n{text}");
}
