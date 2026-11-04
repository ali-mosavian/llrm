//! On the 486 and the Pentium `movzx r32,r8` takes three clocks and `and r32,imm`
//! one, so a byte mask and the signed `/2^k` bias are an `and` at speed and keep
//! the shorter `movzx` and `shr` for size (MA1, SD461). mandel O2 grew 2.5%
//! with `shr` for the bias and `movzx` where the 486 mask is one
//! clock. Needs `LLRM_OW` and the m16 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "int bias(int a, int b) { return (a + b) / 256; }\n\
                      unsigned mask(unsigned a, unsigned b) { return (a + b) & 255; }\n";

/// A missing tool path fails the test: a silent skip reported these tests
/// green on 2026-11-04 without running them.
fn require_env(is_set: impl Fn(&str) -> bool) {
    for name in ["LLRM_OW", "LLRM_CLANG_M16"] {
        assert!(is_set(name), "{name} is not set: this test needs it and does not skip");
    }
}

#[test]
#[should_panic(expected = "LLRM_OW is not set")]
fn a_missing_tool_path_fails_instead_of_skipping() {
    require_env(|_| false);
}

fn asm(
    target: &str,
    level: &str,
    tag: &str,
) -> Option<String> {
    require_env(|name| std::env::var_os(name).is_some());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("maskand-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", target)
        .args([level, "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    Some(std::fs::read_to_string(dir.join("d.s")).unwrap())
}

/// At speed the mask and the bias are `and`s: no `movzx eax, al`, no `shr eax,
/// 24`.
#[test]
fn at_speed_the_byte_mask_and_the_sdiv_bias_are_ands_on_the_486() {
    for (target, tag) in [("x86_32-wat", "m32")] {
        let Some(text) = asm(target, "-O2", tag) else { return };
        assert!(!text.contains("movzx"), "{tag}: {text}");
        assert!(!text.contains("shr"), "{tag}: {text}");
        assert!(text.contains(", 255"), "{tag}: {text}");
    }
}

/// For size they keep the shorter forms.
#[test]
fn at_os_the_byte_mask_and_the_sdiv_bias_keep_the_shorter_forms() {
    for (target, tag) in [("x86_32-wat", "m32os")] {
        let Some(text) = asm(target, "-Os", tag) else { return };
        assert!(text.contains("movzx"), "{tag}: {text}");
        assert!(text.contains("shr"), "{tag}: {text}");
    }
}
