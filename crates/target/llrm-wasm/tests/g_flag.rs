//! `-g` on this target's compiler: a flag the driver knows and cannot do yet
//! is said so, not called unrecognized (`g-for-all-targets-card.md` step 1).

use std::process::Command;

/// `llrm-cc -g` on this target failed with the compiler's `unrecognized
/// argument -g`, as if the flag were a typo.
#[test]
fn a_debug_flag_is_refused_as_not_yet_written_not_as_unrecognized() {
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args(["-g", "x.ll"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(said.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("llrm-wasm-llc: -g: no debug writer for wasm yet"), "{stderr}");
}
