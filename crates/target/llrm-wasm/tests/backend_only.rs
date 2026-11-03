use std::path::Path;
use std::process::Command;

fn calls(backend_only: bool) -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/backend_only.ll");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"));
    command.args(["-S", "-O1", "-o", "-"]).arg(ir);
    if backend_only {
        command.env("LLRM_WASM_BACKEND_ONLY", "1");
    }
    let out = command.output().expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).matches("call\tgcd").count()
}

/// Backend-gap numbers ran the mid-end a second time: recgcd -O1 inlined the
/// `gcd` calls its first run had left, and the "backend" gap (+13 B) was the
/// inliner's. With `LLRM_WASM_BACKEND_ONLY` the IR goes to the backend as is.
#[test]
fn backend_only_keeps_the_calls_the_mid_end_would_inline() {
    assert_eq!(calls(false), 0, "the mid-end inlines gcd at -O1");
    assert_eq!(calls(true), 4);
}
