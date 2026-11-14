//! Of the registers that cost the same, the one that encodes shorter.

use std::process::Command;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// bench/matmul at -Os zeroed its inner-loop counter in `r8d` (`xor r8d, r8d`,
/// a REX prefix) with `ebx`, already pushed and equally free, left unused:
/// 325 B against 324. The rule (DEVIATE C5) decides only between registers equal
/// on unpaid push/pop, so `r8d` is right once the function pushes no `rbx`
/// (#9314's use-list-order LSR left matmul without it: `ebx` would cost a
/// push/pop). The premise is asserted, not assumed.
#[test]
fn a_counter_takes_a_register_that_needs_no_rex_prefix_when_the_cost_is_equal() {
    let text = listing(include_str!("../bench/matmul/matmul.c"), "-Os");
    let rbx_already_paid = text.contains("push rbx");
    assert!(!rbx_already_paid || !text.contains("xor r8d, r8d"), "{text}");
}
