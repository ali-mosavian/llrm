//! The LALR parsers against the hand-written ones, on every program the
//! repository has: nothing the old parsers take may be refused.

use std::path::{Path, PathBuf};
use std::process::Command;

use qbfront::Dialect;
use qbfront::front::scan::scan as lex;

fn tracked(extension: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new("git").arg("-C").arg(&root).args(["ls-files", &format!("*.{extension}")]).output().unwrap();
    // corpus/ holds generated oracle programs, many of them deliberately malformed
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.contains("lalrspike/corpus/"))
        .map(|line| root.join(line))
        .collect()
}

#[test]
fn every_nib_program_the_hand_parser_takes_the_lalr_parser_takes() {
    let files = tracked("nib");
    assert!(files.len() > 200, "found {} files", files.len());
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(tokens) = llrm_nib::lexer::lex(&source) else { continue };
        if llrm_nib::parser::parse(tokens.clone()).is_ok() {
            lalrspike::parse_nib(&tokens).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
    }
}

/// QuickrBASIC's `x += 1`, a token prepass in the table-driven parser. As a
/// production it clashes with `foo -1`, a call with a negative argument.
const NOT_YET: [&str; 1] = ["src/semantic/prelude.bas"];

#[test]
fn every_qb_program_the_table_driven_parser_takes_the_lalr_parser_takes() {
    let files = tracked("bas");
    assert!(files.len() > 500, "found {} files", files.len());
    for path in files {
        if NOT_YET.iter().any(|name| path.ends_with(name)) {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let Some(dialect) =
            [Dialect::QuickBasic45, Dialect::VbDos, Dialect::Quickr].into_iter().find(|&d| qbfront::parse(&source, d).is_ok())
        else {
            continue;
        };
        let tokens = lex(&source, dialect).unwrap();
        lalrspike::parse_qb(&tokens).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}
