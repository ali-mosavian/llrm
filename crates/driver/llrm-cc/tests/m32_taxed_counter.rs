//! A `short` counter on m32 is a 32-bit one: every 16-bit operation on the
//! 486 and P5 pays an operand-size prefix. `ring` -O2 ran
//! `cmp cx, 30003` per trip, 67177 clk against gcc's 61179.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32taxed-{level}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv"])
        .arg("-o")
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_taxed_counter.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

#[test]
fn test_a_short_counter_masked_into_an_index_is_not_a_16_bit_register() {
    for level in ["-O2", "-O3"] {
        let asm = compiled(level);
        let sixteen: Vec<&str> =
            asm.lines().map(str::trim).filter(|line| line.contains(" cx,") || line.ends_with(" cx")).collect();
        assert!(sixteen.is_empty(), "{level}: a 16-bit counter register:\n{}", sixteen.join("\n"));
    }
}
