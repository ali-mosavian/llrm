//! `if (p[i] > 3) count++` over `unsigned short` at -O3 on arm64: the
//! vectorizer's four-lane epilogue reduces a `<4 x i16>`, which had no
//! lowering: `llrm-arm64-llc: @g: CannotSelect("Add")` and no object.
//! The pin selects `addv h0, v0.4h`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned short g(const unsigned short *p, int n)\n{\n    unsigned short count = 0;\n    for (int i = 0; i < n; i++)\n        if (p[i] > 3) count++;\n    return count;\n}\n";

#[test]
fn a_four_lane_epilogue_reduction_compiles() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("reduce64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("g.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "arm64-linux")
        .args(["-O3", "-w", "-S", "-ffreestanding", "-o"])
        .arg(dir.join("g.s"))
        .arg(dir.join("g.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let asm = std::fs::read_to_string(dir.join("g.s")).unwrap();
    assert!(asm.contains("addv\th0, v0.4h") || asm.contains("addv h"), "{asm}");
}
