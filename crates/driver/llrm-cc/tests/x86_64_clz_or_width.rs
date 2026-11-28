//! x86-64: `x ? __builtin_clzl(x) : 64` is a `bsr` and no branch. Long mode
//! priced `ctlz.i64` from the i486 rows (10, the pin's x86-64 rows say 3, and 2
//! for the zero-poison call), so SimplifyCFG kept the branch and InstCombine's
//! select-of-ctlz fold never ran (bitcount_u64 -O1, +6 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_64_bit_clz_guarded_by_a_zero_test_has_no_branch_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("clz-or-width-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/clz_or_width.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body =
        text.split("lead0q proc near").nth(1).and_then(|rest| rest.split("lead0q endp").next()).expect("a lead0q body");
    let branches: Vec<&str> = body.lines().map(str::trim).filter(|line| line.starts_with('j')).collect();
    assert!(branches.is_empty(), "a branch for the zero test: {branches:?}\n{body}");
    assert!(body.contains("bsr"), "no bsr:\n{body}");
}
