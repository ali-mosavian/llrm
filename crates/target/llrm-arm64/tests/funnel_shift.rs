//! `fshl`/`fshr` (`LowerFunnelShift`): the legalizer refused them ("custom
//! lowering of FShl"), so float_in_memory_loop.c had no arm64 cell.

use std::process::Command;

fn body(name: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/funnel_shift.ll");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(ir)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// A constant `fshl` by 5 is `extr` by 27; a constant `fshl` of i64 by 70 is
/// `extr` by 58 (the amount mod 64).
#[test]
fn a_constant_funnel_shift_is_one_extr_by_the_wrapped_amount() {
    assert!(body("fl32c").contains("extr w0, w0, w1, #27"), "{}", body("fl32c"));
    assert!(body("fl64c").contains("extr x0, x0, x1, #58"), "{}", body("fl64c"));
    assert!(body("fr32c").contains("extr w0, w0, w1, #9"), "{}", body("fr32c"));
}

/// A variable amount is expanded to shifts: it lowers, with no `extr`.
#[test]
fn a_variable_funnel_shift_lowers_to_shifts() {
    for name in ["fl32v", "fl64v"] {
        let text = body(name);
        assert!(!text.contains("extr"), "{text}");
        assert!(text.contains("lsl") || text.contains("lsr"), "{text}");
    }
}
