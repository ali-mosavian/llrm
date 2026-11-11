//! `return a < b` at m32 -O2 was `cmp; setb al; and eax, 255` (11 B): the
//! `MaskByAnd` pass turned the `movzx` into an `and` before `FixupSetCC` could
//! make it `xor eax, eax; cmp; setb al` (8 B, the pin's). long_compare_chain
//! +131 B against the pin llc. Needs `LLRM_PIN_CLANG` and `LLRM_LLC`
//! (a built llrm-llc).

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_o2_a_compare_result_is_zeroed_before_the_compare_not_masked() {
    for name in ["LLRM_PIN_CLANG", "LLRM_LLC"] {
        assert!(std::env::var_os(name).is_some(), "{name} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("setcc-mask-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "unsigned long ult(unsigned long a, unsigned long b) { return a < b; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-O2", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(!listing.contains("and eax, 255"), "the result is masked:\n{listing}");
    assert!(listing.contains("xor eax, eax"), "the result is not zeroed first:\n{listing}");
}
