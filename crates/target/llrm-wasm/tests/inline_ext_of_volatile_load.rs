//! parity/algebra -Os: `bench_algebra` (four `sext` of `load volatile i16`)
//! costs the inliner 60 at the pin and read 80 here, over the threshold 75: the
//! call stayed in `main` where clang inlines it (58 instructions run against
//! clang's 56). The pin prices the extension of any load at none where the
//! target has the extending load.

use std::path::Path;
use std::process::Command;

#[test]
fn a_callee_of_extended_volatile_loads_is_inlined_at_os() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/inline_ext_of_volatile_load.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-Os", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    let main = asm.split("__original_main:").nth(1).expect("`main` is emitted");
    let main = main.split("\nmain:").next().unwrap_or(main);
    assert!(!main.contains("call\tbench_algebra"), "`bench_algebra` stayed a call in `main`:\n{asm}");
}
