//! `AlwaysInlinerPass` runs before the inliner, so a call it makes direct is
//! priced with the callee's other uses gone.

/// QuickrBasic operands.c -Os (`oper_i_and`): `hot_op(S, hot_i_and)` is
/// `alwaysinline`, and the call of `hot_i_and` it brings was priced while the
/// pointer to it still counted as a use: no last-call bonus (cost 85 against
/// the pin's -14915), and the size rule kept the call. 90 wrappers kept,
/// operands.c +5,872 B over clang.
#[test]
fn a_call_an_always_inline_callee_makes_direct_is_the_callees_last() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/always_inline_order.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    for wrapper in ["w1:", "w2:"] {
        assert!(!asm.lines().any(|line| line == wrapper), "{wrapper} kept as a function:\n{asm}");
    }
}
