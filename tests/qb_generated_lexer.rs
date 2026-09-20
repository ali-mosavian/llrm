//! Differential gate for the independent generated-parser source scanner.
//!
//! The qbasic-port source matrix caught scanner regressions that parser-only
//! tests miss: identifiers can become generated reserved words, but they may
//! not move or split while doing so. Syntax is the same VBDOS superset for
//! every semantic profile.

use std::fs;
use std::path::Path;
use std::process::Command;

use llrm::frontend::qb::Dialect;
use llrm::frontend::qb::generated_parser::{self, TokenKind, tables};

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn generated_sources() -> Vec<(String, String)> {
    let directory =
        std::env::temp_dir().join(format!("qbfront-generated-lexer-{}", std::process::id()));
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("remove this test's stale corpus directory");
    }
    let status = Command::new("uv")
        .args(["run", "python", "-m", "tools.qbgen"])
        .arg(&directory)
        .current_dir(workspace())
        .status()
        .expect("uv is required for the checked-in qbgen source corpus");
    assert!(
        status.success(),
        "qbgen must materialize its deterministic corpus"
    );

    let mut paths = fs::read_dir(&directory)
        .expect("qbgen output directory")
        .map(|entry| entry.expect("qbgen directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "BAS"))
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 2_160, "the scanner corpus must not contract");
    let sources = paths
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let source = fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("{}: {error}", path.display());
            });
            (name, source)
        })
        .collect();
    fs::remove_dir_all(&directory).expect("remove this test's generated corpus directory");
    sources
}

#[test]
fn generated_lexer_preserves_token_order_and_spans_over_qbasic_port_corpus() {
    let sources = generated_sources();
    let mut accepted = 0;
    for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
        for (name, source) in &sources {
            let tokens = generated_parser::lex(source, dialect)
                .unwrap_or_else(|error| panic!("{name} rejected for {dialect:?}: {error:?}"));
            accepted += 1;
            assert!(
                !tokens.is_empty(),
                "{name} produced no tokens for {dialect:?}"
            );
            let mut previous = (1, 0, 0);
            for token in tokens {
                let current = (token.span.line, token.span.start, token.span.end);
                assert!(
                    current >= previous,
                    "{name} moved backwards for {dialect:?}: {previous:?} then {current:?}"
                );
                assert!(
                    token.span.start <= token.span.end,
                    "{name} reversed a span for {dialect:?}: {token:?}"
                );
                previous = current;
            }
        }
    }
    assert_eq!(accepted, 6_480, "the full generated lexical corpus ran");
}

#[test]
fn recovered_qbasic_forms_remain_explicit_generated_lexer_extensions() {
    let tokens = generated_parser::lex("? &377", Dialect::QuickBasic45).unwrap();
    assert_eq!(
        tokens[0].kind,
        TokenKind::Reserved(tables::token_id("tkQMark").unwrap())
    );
    assert_eq!(tokens[1].kind, TokenKind::Integer(255, None));
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[1].span.start, 2);
}
