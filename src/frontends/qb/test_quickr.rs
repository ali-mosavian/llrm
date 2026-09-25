//! QuickrBASIC (`quickr`): sized integers and mandatory declarations.

use crate::hir::execute;
use crate::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn compiled(source: &str) -> Result<crate::hir::model::Program, String> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "quickr.bas", source.as_bytes());
    qb_driver::parsed(&path, "quickr", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .map_err(|error| error.to_string())
}

/// What FUNCTION `f` returns when the HIR interpreter runs it.
fn returned(source: &str) -> i128 {
    let program = compiled(source).unwrap_or_else(|error| panic!("{error}"));
    match execute::run(&program, "F&", &[]).expect("runs").value {
        Some(Number::Int(value)) => value.into(),
        other => panic!("F returned {other:?}"),
    }
}

#[test]
fn quickr_runs_vbdos_programs() {
    assert_eq!(returned("FUNCTION f&\n  DIM a AS LONG\n  a = 40000\n  f& = a \\ 2\nEND FUNCTION\n"), 20000);
}
