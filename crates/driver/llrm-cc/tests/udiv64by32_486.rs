//! `llrm.x86.udiv64by32`, declared with an asm label, is the `div` instruction
//! (DEVIATE DV1): `hi:lo / d` with the remainder in the high half of the
//! `unsigned long long` result. Needs the Open Watcom tree (tools/ow-fetch.sh)
//! and the m16 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned long long udiv64by32(unsigned hi, unsigned lo, unsigned d) __asm__(\"llrm.x86.udiv64by32\") __attribute__((const));\nunsigned g(unsigned hi, unsigned lo, unsigned d, unsigned *r)\n{\n    unsigned long long x = udiv64by32(hi, lo, d);\n    *r = (unsigned)(x >> 32);\n    return (unsigned)x;\n}\n";

#[test]
fn the_builtin_is_one_div_and_no_call() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("udiv64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args(["-O2", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    assert_eq!(text.lines().filter(|line| line.trim_start().starts_with("div ")).count(), 1, "{text}");
    assert!(!text.contains("call") && !text.contains("udiv64by32"), "{text}");
}
