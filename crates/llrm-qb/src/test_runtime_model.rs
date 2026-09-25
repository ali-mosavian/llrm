//! The HIR interpreter's model of the QB runtime, checked on QB programs.

use llrm_core::hir::execute;
use llrm_core::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn program_on(source: &str, dialect: &str, runtime: &str) -> llrm_core::hir::model::Program {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "model.bas", source.as_bytes());
    qb_driver::parsed(&path, &qb_driver::Frontend::new(dialect, runtime), None)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn program(source: &str, dialect: &str) -> llrm_core::hir::model::Program {
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


#[test]
fn int_floors_beyond_long() {
    // INT went through a LONG: INT(4000000000#) printed -294967296.
    let source = "DIM a AS DOUBLE, b AS DOUBLE\na = 4000000000#: b = -4000000000.5#\nPRINT INT(a); INT(b); INT(2.5#); INT(-2.5#)\n";
    assert_eq!(printed(source), " 4000000000 -4000000001  2 -3 \n");
}

#[test]
fn fix_truncates_toward_zero() {
    // FIX(2.3) was 3 and FIX(-2.3) was -3: both corrections applied at once.
    let source = "DIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE, d AS DOUBLE, e AS DOUBLE\n\
        a = 2.3#: b = 2.7#: c = -2.3#: d = -2.7#: e = -4000000000.5#\n\
        PRINT FIX(a); FIX(b); FIX(c); FIX(d); FIX(e)\n";
    assert_eq!(printed(source), " 2  2 -2 -2 -4000000000 \n");
}

#[test]
fn convert_rounds_to_nearest_even() {
    // The executor truncated CONVERT, where the x87 rounds: CLNG(2.7) read 2.
    let source = "DIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE\na = 2.7#: b = 2.5#: c = 3.5#\nPRINT CLNG(a); CINT(b); CINT(c)\n";
    assert_eq!(printed(source), " 3  2  4 \n");
}

#[test]
fn a_bare_function_name_assigns_its_result() {
    // PDS 7.1's measured pdqcall.bas: `ordered = ...` inside FUNCTION ordered&
    // went to a new variable, so the function returned 0.
    let source = "PRINT ordered&(50); greet$\n\
        FUNCTION ordered& (x AS LONG)\nordered = x - 11\nEND FUNCTION\n\
        FUNCTION greet$\ngreet = \"hi\"\nEND FUNCTION\n";
    assert_eq!(printed(source), " 39 hi\n");
}

#[test]
fn a_dim_inside_a_loop_declares_its_type() {
    // The declaration pass skipped block bodies, so y stayed SINGLE: 2.6.
    let source = "FOR i% = 1 TO 1\nDIM y AS INTEGER\ny = 2.6\nPRINT y\nNEXT\n";
    assert_eq!(printed_on(source, "vbdos", "vbdos"), " 3 \n");
}

#[test]
fn bounds_of_a_static_array_run_in_the_model() {
    // The segment word of the descriptor's far pointer read as 0, so the
    // model took the "not allocated" path into an unmodelled B$LBND.
    let source = "DIM a(1 TO 3) AS INTEGER\nPRINT LBOUND(a); UBOUND(a)\n";
    assert_eq!(printed_on(source, "vbdos", "vbdos"), " 1  3 \n");
}

#[test]
fn len_of_a_concatenation_is_its_length() {
    // LEN took a string it could not name as a place for a variable to size.
    let source = "a$ = \"ab\"\nPRINT LEN(a$ + \"cde\")\n";
    assert_eq!(printed_on(source, "vbdos", "vbdos"), " 5 \n");
}
