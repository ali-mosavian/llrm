//! `x AS LONG` and `x%`.

use llrm_qb::semantic::{Options, compile_with_options};
use llrm_qb::{Dialect, parse};

fn compiled(source: &str) -> Result<String, String> {
    let module = parse(source, Dialect::QuickBasic45).map_err(|error| error.message)?;
    compile_with_options(&module, "t", Dialect::QuickBasic45, "llrm", &Options::default())
        .map_err(|error| format!("{error:?}"))
}

/// BC 4.5: `DIM x AS LONG` then `x%` is "Duplicate definition"
/// (tests/qbrt/suffix_names.bas has the forms BC accepts).
#[test]
fn a_suffix_of_another_type_than_the_declared_one_is_a_duplicate_definition() {
    let error = compiled("DIM x AS LONG\nx% = 4\nPRINT x\n").expect_err("refused");
    assert!(error.contains("Duplicate definition"), "{error}");
    assert!(compiled("DIM x AS LONG\nx& = 4\nPRINT x\n").is_ok());
    assert!(compiled("DIM x AS LONG\nx = 4\nPRINT x&\n").is_ok());
}

/// The same for an array: real BC refuses `a%(1)` after `DIM a(5) AS LONG` (a
/// Severe Error) and accepts `a&(1)`; llrm-qb took `a%(1)` for a third array.
#[test]
fn an_array_suffix_of_another_type_than_the_declared_one_is_a_duplicate_definition() {
    compiled("DIM a(5) AS LONG\na%(1) = 3\nPRINT a(1)\n").expect_err("refused");
    assert!(compiled("DIM a(5) AS LONG\na&(1) = 3\nPRINT a(1)\n").is_ok());
    assert!(compiled("DIM a(5) AS LONG\na(1) = 3\nPRINT a&(1)\n").is_ok());
}

/// A name with `@` is a CURRENCY variable, converted from its integer value.
#[test]
fn a_currency_name_compiles() {
    let module = parse("DIM det@\ndet@ = 1\n", Dialect::VbDos).expect("parses");
    compile_with_options(&module, "t", Dialect::VbDos, "llrm", &Options::default()).expect("compiles");
}

/// A CURRENCY literal is read as CURRENCY, not as a SINGLE.
#[test]
fn a_currency_literal_compiles() {
    let module = parse("x = 1@\n", Dialect::VbDos).expect("a CURRENCY literal is BC's");
    compile_with_options(&module, "t", Dialect::VbDos, "llrm", &Options::default()).expect("compiles");
}
