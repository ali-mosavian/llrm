//! x86-64 -Os: LSR dropped a rewrite its own size order rated worse than the
//! loop as written. queens' `safe`, inlined into `place`, kept a pointer, a
//! count and a negated count: 67 instructions and a `neg`, 60 without
//! (-24 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_row_test_loop_at_os_keeps_one_index_not_three_counters() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsr-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lsr_queens.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let instructions = text
        .lines()
        .filter(|line| line.starts_with("    ") && line.trim_start().starts_with(char::is_alphabetic))
        .count();
    assert!(!text.contains("    neg "), "a negated counter is kept:\n{text}");
    assert!(instructions <= 62, "{instructions} instructions, 60 wanted:\n{text}");
}
