//! `sfm16_float_f2i` at -O1 kept `and w, w, #0x7f800000; cmp w, w12` (a
//! constant register and a mask): a compare against `0x4f000000`, which no
//! `cmp` encodes, was never shifted down. The pin's `SimplifySetCC` makes it
//! `(x >> 24) u< 0x4f`; the loop paid 100 clocks of 880.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O1", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn compare(
    mask: Option<&str>,
    cond: &str,
    constant: &str,
) -> String {
    let and = mask.map_or(String::new(), |m| format!("  %v = and i32 %x, {m}\n"));
    let operand = if mask.is_some() { "%v" } else { "%x" };
    assembly(&format!(
        "define i32 @f(i32 %x) {{\n{and}  %c = icmp {cond} i32 {operand}, {constant}\n  %r = zext i1 %c to i32\n  ret i32 %r\n}}\n"
    ))
}

#[test]
fn an_unsigned_compare_against_a_high_constant_shifts_the_operand() {
    let text = compare(None, "ult", "1325400064");
    assert!(text.contains("ubfm") && text.contains("#79"), "{text}");
    assert!(!text.contains("0x4f000000") && !text.contains("mov"), "{text}");
}

#[test]
fn an_inclusive_bound_shifts_the_operand_by_the_ones_below_it() {
    let text = compare(None, "ugt", "1325400063");
    assert!(text.contains("ubfm") && (text.contains("#78") || text.contains("#79")), "{text}");
    assert!(!text.contains("1325400063") && !text.contains("mov"), "{text}");
}

#[test]
fn a_masked_exponent_compare_keeps_no_constant_register() {
    let text = compare(Some("2139095040"), "ult", "1325400064");
    assert!(text.contains("#79") && !text.contains("mov"), "{text}");
}

#[test]
fn a_negated_power_of_two_mask_equality_is_a_shifted_compare() {
    let text = compare(Some("-256"), "eq", "65280");
    assert!(text.contains("ubfm") && text.contains("#255"), "{text}");
    assert!(!text.contains("and") && !text.contains("mov"), "{text}");
}
