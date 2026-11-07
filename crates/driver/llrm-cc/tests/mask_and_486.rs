//! On the 486 and the Pentium `movzx r32,r8` takes three clocks and `and
//! r32,imm` one, so a byte mask is an `and` at speed and keeps the shorter
//! `movzx` for size (MA1). Needs the Open Watcom tree (tools/ow-fetch.sh) and
//! the m16 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned mask(unsigned a, unsigned b) { return (a + b) & 255; }\n";

/// A missing tool path fails the test: a silent skip reported these tests
/// green on 2026-11-04 without running them.
fn require_env(
    is_set: impl Fn(&str) -> bool,
    ow_tree: bool,
) {
    assert!(ow_tree, "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(is_set("LLRM_CLANG_M16"), "LLRM_CLANG_M16 is not set: this test needs it and does not skip");
}

#[test]
#[should_panic(expected = "the Open Watcom tree is missing")]
fn a_missing_tool_path_fails_instead_of_skipping() {
    require_env(|_| false, false);
}

fn asm(
    source: &str,
    target: &str,
    level: &str,
    tag: &str,
) -> Option<String> {
    require_env(|name| std::env::var_os(name).is_some(), llrm_cc::ow_present());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("maskand-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .args([level, "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    Some(std::fs::read_to_string(dir.join("d.s")).unwrap())
}

/// At speed the mask is an `and`: no `movzx`.
#[test]
fn at_speed_the_byte_mask_is_an_and_on_the_486() {
    for (target, tag) in [("x86_32-wat", "m32")] {
        let Some(text) = asm(SOURCE, target, "-O2", tag) else { return };
        assert!(!text.contains("movzx"), "{tag}: {text}");
        assert!(text.contains(", 255"), "{tag}: {text}");
    }
}

/// For size they keep the shorter forms.
#[test]
fn at_os_the_byte_mask_keeps_the_shorter_movzx() {
    for (target, tag) in [("x86_32-wat", "m32os")] {
        let Some(text) = asm(SOURCE, target, "-Os", tag) else { return };
        assert!(text.contains("movzx"), "{tag}: {text}");
    }
}

/// A masked value still read afterwards keeps the `movzx`: the `and` would
/// need a `mov` first (mk4, 2026-11-04: nbody_fixed -O3 +25024 clocks, +31000
/// instructions).
#[test]
fn at_speed_a_mask_of_a_live_value_keeps_the_movzx() {
    let live = "unsigned mask(unsigned a, unsigned b) { unsigned s = a + b; return (s & 255) * 3 + s; }\n";
    let text = asm(live, "x86_32-wat", "-O2", "m32live").unwrap();
    assert!(text.contains("movzx"), "{text}");
}

/// A halfword mask keeps the `movzx`: as an `and` it moved the allocation in
/// nbody_fixed -O3 (2026-11-04: +25024 clocks, +48 B, a loop counter spilled).
#[test]
fn at_speed_a_halfword_mask_keeps_the_movzx() {
    let half = "unsigned mask(unsigned a, unsigned b) { return (a + b) & 65535; }\n";
    let text = asm(half, "x86_32-wat", "-O2", "m32half").unwrap();
    assert!(text.contains("movzx"), "{text}");
}
