//! x86-64: SROA cuts an odd-sized slice (a 5-byte struct is an `i40`) where the
//! pin does, so a call with constant odd-sized arguments folds once inlined.
//! The `alloca i40` stayed (DEVIATE `selects_odd_sized_integers`, default no),
//! the SZ2 trial priced `fives` inlined as larger, and struct_odd_sizes `main`
//! kept the call (-O1 172 B, pin 152). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_call_with_a_constant_five_byte_struct_is_inlined_and_folded_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("odd-size-struct-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/odd_size_struct_inline.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body =
        text.split("main proc near").nth(1).and_then(|rest| rest.split("main endp").next()).expect("a main body");
    assert!(!body.contains("call fives"), "fives was not inlined:\n{body}");
}
