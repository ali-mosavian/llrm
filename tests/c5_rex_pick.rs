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

/// The positive control: matmul with an early value kept across two calls, so
/// `rbx` is pushed and free once the loops start. The counter is then zeroed
/// in `ebx` (no REX prefix), not `r8d`.
#[test]
fn a_counter_takes_the_already_pushed_rbx_over_a_rex_register() {
    let source = include_str!("../bench/matmul/matmul.c")
        .replace("extern void report(long value);", "extern void report(long value);\nextern long make(void);")
        .replace(
            "    unsigned long checksum = 0;\n",
            "    unsigned long checksum = 0;\n    long early = make();\n    report(early);\n    report(early + 1);\n",
        );
    let text = listing(&source, "-Os");
    assert!(text.contains("push rbx"), "premise: rbx is pushed\n{text}");
    assert!(!text.contains("xor r8d, r8d"), "{text}");
    assert!(text.contains("xor ebx, ebx"), "{text}");
}
