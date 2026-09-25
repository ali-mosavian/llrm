//! The HIR interpreter's model of the QB runtime, checked on QB programs.

use crate::hir::execute;
use crate::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn program(source: &str, dialect: &str) -> crate::hir::model::Program {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "model.bas", source.as_bytes());
    qb_driver::parsed(&path, dialect, "vbdos", None, &[], "column-major", false, false, false, false, false)
        .unwrap_or_else(|error| panic!("{error}"))
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
