//! x86-64 `-fexcess-precision=standard`: an integer converted to float or double was stored to the stack and
//! loaded back (a rounding the x87 needs and scalar SSE does not), 5-6 B per conversion, int_compared_with_float.c
//! +32 B against clang. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn an_integer_converted_to_float_is_not_rounded_through_the_stack_on_sse() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("float-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-fexcess-precision=standard", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/float_store_sse.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("ptr [rsp], xmm"), "the conversion was stored to the stack:\n{text}");
}
