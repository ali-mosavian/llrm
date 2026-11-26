//! m32 -O1 walks the call graph twice (`Tti::inline_second_round_at_o1`).
//! The second walk was gated on "no byte price" for "-O1", so pricing bytes at
//! -O1 (`speed_rate_at_o1`) turned it off without a word: bodies the first
//! walk had grown were never simplified again, and a module of many-armed
//! synthesis loops came out a quarter larger.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_o1_walks_twice_with_the_byte_price_on() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("o1-walk2-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("stages")).unwrap();
    std::fs::write(dir.join("d.c"), "static int twice(int a) { return a * 2; }\nint f(int a) { return twice(a); }\n")
        .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_MIR_STAGES", dir.join("stages"))
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O1", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let walks = std::fs::read_dir(dir.join("stages"))
        .unwrap()
        .filter(|one| one.as_ref().unwrap().file_name().to_string_lossy().ends_with("-walk.ll"))
        .count();
    assert_eq!(walks, 2, "the call graph is walked {walks} time(s) at -O1");
}
