//! SD443's second IPSCCP replaced the arguments of `lrN` (whose address
//! `check_locals` took before inlining) by `@check_locals.y` and `@src`; the
//! pin's one IPSCCP leaves them, and the copy loops stay word-wise there
//! (`memory_expansion` -O2 wasm32: +248 B). Needs `LLRM_LLC` and
//! `LLRM_CLANG_WASM`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_function_called_with_constants_after_inlining_keeps_its_parameters_on_wasm() {
    for name in ["LLRM_LLC", "LLRM_CLANG_WASM"] {
        assert!(std::env::var_os(name).is_some(), "{name} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wasm-ipsccp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/second_ipsccp.c");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "wasm32")
        .env("LLRM_WASM_IR", "1")
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let ir = String::from_utf8_lossy(&run.stderr);
    assert!(ir.lines().any(|line| line.starts_with("define internal") && line.contains("@lr1(ptr")), "{ir}");
}
