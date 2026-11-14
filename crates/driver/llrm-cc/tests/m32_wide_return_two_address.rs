//! x86-32: both halves of an `edx:eax` return are known to TwoAddress. A `ret`
//! carries two requirements where the pin has two `COPY`s, and scanning the low
//! half's chain marked the whole `ret` processed: the high half had no register
//! to end in, `adc` kept the sign word as its tied operand and the sum was
//! moved after it (`adc ecx, edx; mov edx, ecx`: wc_mixed `llrm_wide` -Os +2
//! B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wide-return-{}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/wide_return.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn a_wide_sum_is_built_in_edx_and_not_moved_there() {
    let text = assembly("-Os");
    assert!(text.contains("mul "), "premise: the product is a widening multiply:\n{text}");
    assert!(!text.contains("mov edx, ecx"), "the sum is moved into edx after the add:\n{text}");
}
