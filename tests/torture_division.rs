//! Failures of gcc.c-torture/execute that a source program shows: each a C program, its listing read.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-march=i486"])
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// A 64-bit remainder by a variable in a 32-bit segment ran the helper's 16-bit-segment bytes, `66` prefixes and all
/// (920501-2, every level): the listing's inline bytes carry none.
#[test]
fn a_division_by_a_variable_of_64_bits_carries_no_operand_size_prefix() {
    let text =
        listing("unsigned long long f(unsigned long long a, unsigned long long b) { return a % b; }\n", &["-O2"]);
    let bytes: Vec<&str> =
        text.lines().filter_map(|line| line.trim().strip_prefix("db ")).flat_map(|line| line.split(',')).collect();
    assert!(bytes.len() > 20, "the helper is inline: {text}");
    assert_ne!(bytes[0].trim(), "066h", "the helper opens with an operand-size prefix: {text}");
}
