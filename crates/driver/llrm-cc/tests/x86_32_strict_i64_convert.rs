//! `#pragma STDC FENV_ACCESS ON` makes a 64-bit integer to floating-point
//! conversion `llvm.experimental.constrained.sitofp` (and back, `fptosi`). On
//! x86-32 the x87 does them by memory (`fild qword`, `fistp qword`); the
//! legalizer refused both: `Unsupported StrictSIntToFp i64 "expand operand"`,
//! `StrictFpToSInt i64 "expand result"`, so QuickrBasic's `pfpu.c` did not
//! compile. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_strict_64_bit_conversion_is_fild_and_fistp_on_the_x87() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-strict-i64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for level in ["-O1", "-Os"] {
        let asm = dir.join("a.s");
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env("LLRM_LLC", &llc)
            .args(["-march=i486", level, "-w", "-S", "-o"])
            .arg(&asm)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/strict_i64_convert.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
        let text = std::fs::read_to_string(&asm).unwrap();
        assert!(text.contains("fild qword ptr"), "{level}: no fild:\n{text}");
        assert!(text.contains("fistp qword ptr"), "{level}: no fistp:\n{text}");
    }
}
