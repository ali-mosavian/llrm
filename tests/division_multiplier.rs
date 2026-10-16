//! A division by a constant is a multiply by its reciprocal where the multiply,
//! priced by its operand, is cheaper.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// The part of the listing from the function `name` to the next `endp`.
fn function(
    text: &str,
    name: &str,
) -> String {
    let start = text.find(&format!("_{name} proc")).unwrap();
    text[start..].split("endp").next().unwrap().to_owned()
}

/// The 486's multiply ends early on its r/m operand: 10 + max(bits, 3). The
/// bits of the dividend were read inverted (the leading zeros of the mask of
/// known-zero bits, not its leading ones), so a dividend of 30 bits read as
/// unknown and was priced at the middle of the range: `mul`, 41 clocks, for
/// `div`'s 40. And one of 16 bits, a quarter of the price, read as unknown too.
#[test]
fn test_a_dividend_of_30_bits_keeps_the_division_and_one_of_16_is_multiplied() {
    let text = listing(
        "unsigned wide(unsigned x) { return (x & 0x3fffffff) / 10; }
unsigned narrow(unsigned x) { return (x & 0xffff) / 10; }
unsigned unknown(unsigned x) { return x / 10; }
unsigned nonnegative(unsigned x) { return (x & 0x7fffffff) / 10; }",
    );
    let (wide, narrow, unknown) = (function(&text, "wide"), function(&text, "narrow"), function(&text, "unknown"));
    let nonnegative = function(&text, "nonnegative");
    assert!(wide.contains("div ") && !wide.contains("mul "), "{wide}");
    assert!(narrow.contains("mul ") && !narrow.contains("div "), "{narrow}");
    assert!(unknown.contains("mul ") && !unknown.contains("div "), "an unknown dividend is gcc's: {unknown}");
    // No more than "not negative" is not a length: a loop counter below INT_MAX
    // runs far lower.
    assert!(nonnegative.contains("mul ") && !nonnegative.contains("div "), "{nonnegative}");
}
