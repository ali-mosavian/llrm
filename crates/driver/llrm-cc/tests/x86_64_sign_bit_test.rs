//! x86-64: `(x >= 0) + y` is `not; shr; lea` (the pin's
//! `foldExtendedSignBitTest`: `zext i1 (setgt X, -1)` is `srl (not X), N-1`),
//! not `test; setns` and an add. The fold was not translated: signbit_idioms
//! -O2 ran 1,013 instructions over gcc (+1.4%). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn body(name: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sign-bit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/sign_bit_added.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    text.split(&format!("{name} proc near"))
        .nth(1)
        .and_then(|rest| rest.split(&format!("{name} endp")).next())
        .expect("a body")
        .to_string()
}

#[test]
fn a_sign_test_added_to_a_value_is_a_shift_with_no_setcc_at_x86_64() {
    for name in ["ge_added", "ge_added_long"] {
        let text = body(name);
        assert!(text.contains("shr"), "{name} has no shr:\n{text}");
        assert!(!text.contains("setns") && !text.contains("test "), "{name} tests and sets a byte:\n{text}");
    }
}
