//! A switch on a byte with cases of 128 and up at -O1/-O2 selects `ROR32r1`
//! (the bit-test's rotate by one). The emitter looked for the shift amount in
//! the node's operands and stopped at `ROR32r1: missing source`, so the file
//! was refused. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_rotate_by_one_emits_its_amount() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-rotate1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for level in ["-O1", "-O2"] {
        let asm = dir.join("a.s");
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env("LLRM_LLC", &llc)
            .args(["-march=i486", level, "-w", "-S", "-o"])
            .arg(&asm)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/rotate_by_one_switch.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
        let text = std::fs::read_to_string(&asm).unwrap();
        assert!(text.contains("ror eax, 1"), "{level}: no rotate by one:\n{text}");
    }
}
