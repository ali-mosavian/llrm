//! A refused source is told where and what, in words: the line and column, the
//! name or token at fault, and what to do. BC's name for the error stays in the
//! message for anyone who knows it.

use qbfront::{parse, Dialect};

/// The error for `source`, with its place as `line:column`.
fn refused(
    source: &str,
    dialect: Dialect,
) -> (String, qbfront::ParseError) {
    let error = parse(source, dialect).expect_err("refused");
    (format!("{}:{}", error.span.line, error.span.start + 1), error)
}

#[test]
fn a_reserved_word_used_as_a_name_is_named_with_the_remedy() {
    let (at, error) = refused("x% = 1\r\nCOLOR = 4\r\n", Dialect::VbDos);
    assert_eq!(at, "2:1");
    assert!(error.message.contains("`COLOR` cannot be used as a name"), "{}", error.message);
    assert!(error.message.contains("reserved word"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("rename")), "{error:?}");
    assert_eq!(error.code, "Identifier expected");
}

#[test]
fn an_array_used_before_it_is_dimensioned_names_the_array() {
    let (at, error) = refused("y% = z%(3)\r\n", Dialect::QuickBasic45);
    assert_eq!(at, "1:6");
    assert!(error.message.contains("`z%` is used as an array before any DIM"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("DIM z%(")), "{error:?}");
}

#[test]
fn an_unfinished_statement_says_which_statement_and_that_the_line_ended() {
    let (at, error) = refused("PRINT (1 +\r\n", Dialect::VbDos);
    assert_eq!(at, "1:11");
    assert!(error.message.contains("the PRINT statement is incomplete"), "{}", error.message);
}

#[test]
fn a_missing_token_is_what_was_expected() {
    let (_, error) = refused("x% = (1 + 2\r\n", Dialect::VbDos);
    assert!(error.message.contains("expected `)`"), "{}", error.message);
}

#[test]
fn a_jump_to_nowhere_names_the_label() {
    let (at, error) = refused("x% = 1\r\nGOTO missing\r\n", Dialect::QuickBasic45);
    assert_eq!(at, "2:6");
    assert!(error.message.contains("no label `missing`"), "{}", error.message);
}

#[test]
fn a_source_valid_for_another_dialect_says_which() {
    let (_, error) = refused("DIM a(1 TO 3)\r\nSUB p ()\r\nEND SUB\r\n", Dialect::QuickBasic45);
    let help = error.help.expect("help");
    assert!(help.contains("VBDOS (--dialect vbdos)"), "{help}");
}

#[test]
fn quickr_syntax_in_a_microsoft_dialect_points_at_the_quickr_profile() {
    let (_, error) = refused("x = 1\r\nx += 1\r\n", Dialect::VbDos);
    assert!(error.help.expect("help").contains("--dialect quickr"));
}

#[test]
fn every_error_has_words_and_a_place() {
    for (source, dialect) in [
        ("DIM x AS Nope\r\n", Dialect::VbDos),
        ("FOR i% = 1 TO\r\n", Dialect::QuickBasic45),
        ("x$ = 5\r\n", Dialect::Pds71),
        ("PRINT 1 +* 2\r\n", Dialect::QuickBasic45),
    ] {
        let error = parse(source, dialect).expect_err(source);
        assert!(error.span.line >= 1, "{source}");
        assert!(error.message.len() > error.code.len() + 10, "{source}: {}", error.message);
        assert!(error.message.contains("[BC: "), "{source}: {}", error.message);
    }
}

/// The codes of every error the front raises on the BC corpus's programs (5,000
/// small programs that reach most checks) all have an entry in `messages.txt`:
/// none is left to the fallback wording.
#[test]
fn every_error_the_corpus_raises_is_worded() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../lalrspike/corpus/programs");
    let Ok(entries) = std::fs::read_dir(&directory) else { return };
    let mut unworded = std::collections::BTreeMap::<String, String>::new();
    for entry in entries.filter_map(Result::ok) {
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let text = text.replace('\n', "\r\n");
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            if let Err(error) = qbfront::front::parse_coded(&text, dialect) {
                let worded = qbfront::front::explain::explain(error.clone(), &text);
                if !worded.message.contains(" [BC: ") || worded.message.starts_with(&format!("{} (at", error.code)) {
                    unworded.entry(error.code.clone()).or_insert_with(|| entry.path().display().to_string());
                }
            }
        }
    }
    assert!(unworded.is_empty(), "errors with no words in messages.txt (code: an example): {unworded:#?}");
}
