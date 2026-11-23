//! x86-64 `long double` compares set the flags with `fucomi`/`fucomip` (every
//! x86-64 CPU has them). They were `fcom; fnstsw ax; sahf`: dbl_fn -Os ran
//! 193M instructions against clang's 167M.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x64fcomi{level}{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-fPIE", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_x87_fcomi.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

#[test]
fn test_a_long_double_compare_sets_the_flags_without_the_status_word() {
    for level in ["-Os", "-O2"] {
        let asm = compiled(level);
        for banned in ["fnstsw", "sahf"] {
            assert!(!asm.contains(banned), "{level}: {banned} kept:\n{asm}");
        }
        assert!(asm.contains("fucomip st(0), st(1)"), "{level}: no fucomip in g:\n{asm}");
        assert!(asm.contains("fucomi st(0), st(1)"), "{level}: no fucomi in h:\n{asm}");
    }
}
