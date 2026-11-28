use std::path::Path;
use std::process::Command;

/// DEVIATE SZ1 held every copy of `classify` at -O1, though the pin's own cost
/// of each was below zero (-15..-5: the constant arguments fold the body).
/// uint64_to_float -O1 came out 241 B against the pin's 96.
#[test]
fn an_inline_the_pin_prices_below_zero_is_taken_at_o1() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/size_rule_negative_cost.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let calls: Vec<&str> =
        asm.lines().filter(|line| line.trim().starts_with("call") && line.contains("classify")).collect();
    assert!(calls.is_empty(), "{calls:?}");
}
