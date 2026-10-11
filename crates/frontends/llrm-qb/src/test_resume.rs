//! RESUME to a label.

use super::compile as qb_compile;
use super::driver as qb_driver;
use super::test_hir::written;

/// What compiling `source` for the llrm runtime gives: the assembly, or the refusal.
fn compiled(source: &str) -> Result<(), String> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "RESUME.BAS", source.as_bytes());
    let program = qb_driver::parsed(&path, &qb_driver::Frontend::new("qb45", "qb45"), None).map_err(|error| error.0)?;
    let codegen = llrm_driver::m16_options(llrm_x86_m16::machine::BASIC.clone());
    qb_compile::assembled(&program, None, &codegen).map(|_| ()).map_err(|error| error.to_string())
}

/// RESUME to a label in the handler's own code was a panic ("no entry found for key"), not a refusal:
/// the label's block is the handler's, which the body cannot continue in.
#[test]
fn test_resume_to_a_label_inside_the_handler_is_refused_not_a_panic() {
    let source = "ON ERROR GOTO handler\nERROR 9\nEND\n\nhandler:\nIF ERR = 9 THEN RESUME inside\ninside:\nPRINT \"x\"\n";
    let error = compiled(source).expect_err("refused");
    assert!(error.contains("RESUME"), "{error}");
}

/// RESUME to a label of the body, the usual one, compiles.
#[test]
fn test_resume_to_a_label_in_the_body_compiles() {
    assert!(compiled("ON ERROR GOTO handler\nERROR 9\nPRINT \"no\"\nafter:\nPRINT \"yes\"\nEND\nhandler:\nRESUME after\n").is_ok());
}
