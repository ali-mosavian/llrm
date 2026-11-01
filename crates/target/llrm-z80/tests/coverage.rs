//! Selection coverage: one small function per operation and type, signed as well as unsigned, so a gap in the
//! legalizer or the tables is a failing test and not a find. Every function must select. `sext i8 to i16` was refused
//! for want of a pattern and no unsigned bench kernel noticed.

use llrm_z80::dagisel::function::select_function;
use llrm_z80::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

/// The functions that were refused, with why; empty when everything selects.
fn refused(cases: &[(String, String)]) -> Vec<String> {
    let mut bad = Vec::new();
    for (name, body) in cases {
        let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{name}: {why}\n{text}"));
        let name = name.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            select_function(&module, "f", Cpu::Z80).map(|_| ())
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(why)) => bad.push(format!("{name}: {why}")),
            Err(_) => bad.push(format!("{name}: panicked")),
        }
    }
    bad
}

fn binary(
    op: &str,
    bits: u32,
) -> (String, String) {
    (
        format!("{op} i{bits}"),
        format!("define i{bits} @f(i{bits} %a, i{bits} %b) {{\n  %r = {op} i{bits} %a, %b\n  ret i{bits} %r\n}}"),
    )
}

/// The second operand is a constant, so the function takes one argument and needs no stack slot.
fn binary_const(
    op: &str,
    bits: u32,
) -> (String, String) {
    (
        format!("{op} i{bits} by a constant"),
        format!("define i{bits} @f(i{bits} %a) {{\n  %r = {op} i{bits} %a, 77\n  ret i{bits} %r\n}}"),
    )
}

fn compare_const(
    cc: &str,
    bits: u32,
) -> (String, String) {
    (
        format!("icmp {cc} i{bits} with a constant"),
        format!(
            "define i{bits} @f(i{bits} %a) {{\n  %c = icmp {cc} i{bits} %a, -77\n  %r = select i1 %c, i{bits} %a, i{bits} 5\n  ret i{bits} %r\n}}"
        ),
    )
}

fn compare(
    cc: &str,
    bits: u32,
) -> (String, String) {
    (
        format!("icmp {cc} i{bits}"),
        format!(
            "define i{bits} @f(i{bits} %a, i{bits} %b) {{\n  %c = icmp {cc} i{bits} %a, %b\n  %r = select i1 %c, i{bits} %a, i{bits} %b\n  ret i{bits} %r\n}}"
        ),
    )
}

fn cast(
    op: &str,
    from: u32,
    to: u32,
) -> (String, String) {
    (
        format!("{op} i{from} to i{to}"),
        format!("define i{to} @f(i{from} %a) {{\n  %r = {op} i{from} %a to i{to}\n  ret i{to} %r\n}}"),
    )
}

const OPS: [&str; 13] =
    ["add", "sub", "and", "or", "xor", "mul", "udiv", "sdiv", "urem", "srem", "shl", "lshr", "ashr"];
const DIVIDES: [&str; 4] = ["udiv", "sdiv", "urem", "srem"];
const CONDS: [&str; 10] = ["eq", "ne", "ult", "ule", "ugt", "uge", "slt", "sle", "sgt", "sge"];

#[test]
fn arithmetic_selects_at_every_width() {
    let mut cases = Vec::new();
    for bits in [8, 16] {
        for op in OPS {
            cases.push(binary(op, bits));
        }
    }
    for op in OPS.iter().filter(|op| !DIVIDES.contains(op)) {
        cases.push(binary_const(op, 32));
    }
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

/// A long divide is a call of `__divsi3` and its kin, two longs: eight bytes of arguments, seven registers, so the
/// stack (row 4). The type legalizer has to take the target's libcall name for the expanded type, as the pin's
/// `ExpandIntRes_SDIV` does; today it knows only the x86 inline `i64` routine.
#[test]
#[ignore = "row 4: stack arguments (due 2026-11-04)"]
fn divides_of_a_long_select() {
    let cases: Vec<_> = DIVIDES.iter().map(|op| binary_const(op, 32)).collect();
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

/// Two `i32` arguments are eight bytes and the registers hold seven, so the second one goes on the stack: row 4.
#[test]
#[ignore = "row 4: stack arguments (due 2026-11-04)"]
fn arithmetic_on_two_longs_selects() {
    let cases: Vec<_> = OPS.iter().map(|op| binary(op, 32)).collect();
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn signed_and_unsigned_compares_select_at_every_width() {
    let mut cases = Vec::new();
    for bits in [8, 16] {
        for cc in CONDS {
            cases.push(compare(cc, bits));
        }
    }
    for cc in CONDS {
        cases.push(compare_const(cc, 32));
    }
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

/// Row 4, as above.
#[test]
#[ignore = "row 4: stack arguments (due 2026-11-04)"]
fn compares_of_two_longs_select() {
    let cases: Vec<_> = CONDS.iter().map(|cc| compare(cc, 32)).collect();
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn extensions_and_truncations_select_across_widths() {
    let mut cases = Vec::new();
    for (from, to) in [(8, 16), (8, 32), (16, 32)] {
        cases.push(cast("zext", from, to));
        cases.push(cast("sext", from, to));
        cases.push(cast("trunc", to, from));
    }
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn shifts_by_constants_select_at_every_width() {
    let mut cases = Vec::new();
    for bits in [8, 16, 32] {
        for op in ["shl", "lshr", "ashr"] {
            for k in [1, 3, bits - 1] {
                cases.push((
                    format!("{op} i{bits} by {k}"),
                    format!("define i{bits} @f(i{bits} %a) {{\n  %r = {op} i{bits} %a, {k}\n  ret i{bits} %r\n}}"),
                ));
            }
        }
    }
    let bad = refused(&cases);
    assert!(bad.is_empty(), "{} refused:\n{}", bad.len(), bad.join("\n"));
}
