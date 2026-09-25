//! The HIR interpreter's model of the QB runtime, checked on QB programs.

use crate::hir::execute;
use crate::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn program_on(source: &str, dialect: &str, runtime: &str) -> crate::hir::model::Program {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "model.bas", source.as_bytes());
    qb_driver::parsed(&path, dialect, runtime, None, &[], "column-major", false, false, false, false, false)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn program(source: &str, dialect: &str) -> crate::hir::model::Program {
    program_on(source, dialect, "vbdos")
}

/// What the module-level code prints, under `runtime`.
pub(super) fn printed_on(source: &str, dialect: &str, runtime: &str) -> String {
    let executed = execute::run(&program_on(source, dialect, runtime), "__main", &[]).expect("runs");
    assert_eq!(executed.panic, None, "{}", executed.output);
    executed.output
}

pub(super) fn printed(source: &str) -> String {
    printed_on(source, "vbdos", "vbdos")
}

/// What FUNCTION `f&` returns.
fn returned(source: &str) -> i128 {
    match execute::run(&program(source, "vbdos"), "F&", &[]).expect("runs").value {
        Some(Number::Int(value)) => value.into(),
        other => panic!("F& returned {other:?}"),
    }
}

#[test]
fn floating_literals_hold_their_value() {
    // Static places got fresh zeroed memory, so every float literal read 0.
    assert_eq!(returned("FUNCTION f&\nDIM l AS LONG\nl = 40000#\nf& = l \\ 2\nEND FUNCTION\n"), 20000);
}

#[test]
fn print_spaces_numbers_and_zones_commas() {
    assert_eq!(printed("PRINT 5; -3; 70000&\n"), " 5 -3  70000 \n");
    // Zones start at columns 1 and 15, so 2's digit is in column 16.
    assert_eq!(printed("PRINT 1, 2\n"), format!(" 1{}2 \n", " ".repeat(13)));
    assert_eq!(printed("PRINT \"a\";\nPRINT \"b\"\n"), "ab\n");
    assert_eq!(printed("PRINT\n"), "\n");
}

#[test]
fn string_functions_on_both_literal_layouts() {
    let source = "DIM a AS STRING, b AS STRING\n\
        a = \"hello\"\n\
        b = a + \" world\"\n\
        PRINT b; LEN(b)\n\
        PRINT LEFT$(b, 2); RIGHT$(b, 3); MID$(b, 2, 3); MID$(b, 9)\n\
        PRINT INSTR(b, \"o\"); INSTR(6, b, \"o\"); INSTR(b, \"z\")\n\
        PRINT UCASE$(a); LTRIM$(\"  x\"); RTRIM$(\"y  \"); \"|\"\n\
        PRINT STRING$(3, \"*\"); SPACE$(2); CHR$(65); ASC(\"B\")\n\
        PRINT STR$(42); STR$(-7); HEX$(255); OCT$(8)\n\
        IF a < b THEN PRINT \"less\"\n";
    let expected = "hello world 11 \nherldellrld\n 5  8  0 \nHELLOxy|\n***  A 66 \n 42-7FF10\nless\n";
    assert_eq!(printed_on(source, "vbdos", "vbdos"), expected);
    assert_eq!(printed_on(source, "qb45", "qb45"), expected);
}

#[test]
fn floats_print_as_measured() {
    assert_eq!(printed("DIM d AS DOUBLE\nd = 1 / 4\nPRINT d; STR$(d)\n"), " .25  .25\n");
    assert_eq!(printed("DIM x AS SINGLE\nx = 2.5\nPRINT x * 2\n"), " 5 \n");
}

