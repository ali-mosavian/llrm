//! A division by a constant is a multiply by its reciprocal where the multiply,
//! priced by its operand, is cheaper.

mod common;

fn listing(
    source: &str,
    isel: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", &format!("--isel={isel}"), "-S", "-o", "a.s", "a.c"])
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
///
/// `wide` keeping its division is the 486 deviation: the pin multiplies it
/// (clang-24 `imull`), the old selector prices the early-out and keeps `div`.
/// The 16-bit case runs on `--isel=dag`: CVP (its second slot, #2398) narrows
/// the divide to i16, which the old selector does not expand (`div cx`) and the
/// pin and the DAG selector multiply. Flip condition: the DAG default lands
/// before the main merge (docs: flip-conditions).
#[test]
fn test_a_dividend_of_30_bits_keeps_the_division_and_one_of_16_is_multiplied() {
    let text = listing(
        "unsigned wide(unsigned x) { return (x & 0x3fffffff) / 10; }
unsigned unknown(unsigned x) { return x / 10; }
unsigned nonnegative(unsigned x) { return (x & 0x7fffffff) / 10; }",
        "old",
    );
    let (wide, unknown, nonnegative) =
        (function(&text, "wide"), function(&text, "unknown"), function(&text, "nonnegative"));
    assert!(wide.contains("div ") && !wide.contains("mul "), "{wide}");
    assert!(unknown.contains("mul ") && !unknown.contains("div "), "an unknown dividend is gcc's: {unknown}");
    // No more than "not negative" is not a length: a loop counter below INT_MAX
    // runs far lower.
    assert!(nonnegative.contains("mul ") && !nonnegative.contains("div "), "{nonnegative}");
    let narrow = function(&listing("unsigned narrow(unsigned x) { return (x & 0xffff) / 10; }", "dag"), "narrow");
    // Multiplied by the magic number: the multiply by 52429 is a chain of `lea`s (MUL_IMM, one clock
    // each) on this selector, not a `mul`; what matters is that no `div` is left and the high part is shifted out.
    assert!(!narrow.contains("div ") && narrow.contains("shr eax, 19"), "{narrow}");
}
