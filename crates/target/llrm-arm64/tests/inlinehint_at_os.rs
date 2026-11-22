//! A callee declared `inline` is inlined at -Os where the pin's hint
//! threshold passes it, though it grows the code. The -Os trial
//! (INLINEHINT-GROWTH-TRIAL, `hinted_calls_trial.rs`) then keeps the calls
//! where the object is smaller, so this pins the inliner's yield with the
//! trial off (`LLRM_ARM64_NO_TRIAL`): the fixture's two copies would grow the
//! object.

fn calls_leaf(hint: &str) -> bool {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/inlinehint_at_os.ll").replace("HINT", hint)).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_NO_TRIAL", "1")
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap().lines().any(|line| line.trim() == "bl leaf" || line.trim() == "b leaf")
}

/// QuickrBasic operands.c -Os: `hot_operand` (`static inline`, 26 units
/// against two 3-unit calls) stayed a call under the by-size bound, the pin
/// inlines it at cost 115 under its hint threshold 325: 15,344 B against
/// 9,464 B.
#[test]
fn an_inline_callee_that_grows_the_code_is_inlined_at_os() {
    assert!(!calls_leaf("inlinehint"), "the pin's hint threshold passes it");
    assert!(calls_leaf(""), "the control: without the hint the size bound keeps the call");
}
