//! m32 -O2 (SD-DIVMAG): `x / -3` and `x % -3` of one dividend multiply once.
//! The quotient used the negative divisor's own magic and the remainder (`x % -c`
//! is `x % c`) the positive one: two `imul`, 13 to 42 clocks each on the 486
//! (div_constants_long -O2: 27,108 `imul` against gcc's 21,072, +4.1% clk).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_quotient_and_remainder_by_a_negative_constant_multiply_once() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("divmag-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), "void qr(long x, long *o) { o[0] = x / -3; o[1] = x % -3; }\n").unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let multiplies = asm.lines().filter(|line| line.trim_start().starts_with("imul")).count();
    assert_eq!(multiplies, 1, "x / -3 and x % -3 took {multiplies} multiplies:\n{asm}");
}
