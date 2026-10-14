//! `x AS LONG` and `x%`.

use qbfront::semantic::{compile_with_options, Options};
use qbfront::{parse, Dialect};

fn compiled(source: &str) -> Result<String, String> {
    let module = parse(source, Dialect::QuickBasic45).expect("parses");
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
