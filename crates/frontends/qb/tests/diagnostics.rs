//! A refused source is told where and what, in words: the line and column, the
//! name or token at fault, and what to do. BC's name for the error stays in the
//! message for anyone who knows it.

use llrm_qb::{Dialect, parse};

/// The error for `source`, with its place as `line:column`.
fn refused(
    source: &str,
    dialect: Dialect,
) -> (String, llrm_qb::ParseError) {
    let error = parse(source, dialect).expect_err("refused");
    (format!("{}:{}", error.span.line, error.span.start + 1), error)
}

#[test]
fn a_reserved_word_used_as_a_name_is_named_with_the_remedy() {
    let (at, error) = refused("x% = 1\r\nCOLOR = 4\r\n", Dialect::VbDos);
    assert_eq!(at, "2:1");
    assert!(error.message.contains("`COLOR` is a reserved word and cannot be used as a name"), "{}", error.message);
    assert!(error.message.contains("reserved word"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("rename")), "{error:?}");
    assert_eq!(error.code, "Identifier expected");
}

#[test]
fn an_array_used_before_it_is_dimensioned_names_the_array() {
    let (at, error) = refused("y% = z%(3)\r\n", Dialect::QuickBasic45);
    assert_eq!(at, "1:6");
    assert!(error.message.contains("`z%` is followed by `(`, so BC reads it as an array"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("DIM z%(")), "{error:?}");
}

#[test]
fn an_unfinished_statement_says_which_statement_and_what_it_waits_for() {
    let (at, error) = refused("PRINT (1 +\r\n", Dialect::VbDos);
    assert_eq!(at, "1:11");
    assert!(error.message.contains("the PRINT statement ends after `+`, which needs a value"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("PRINT is written:")), "{error:?}");
    let (_, error) = refused("PRINT\r\nFOR i = 1 TO\r\n", Dialect::VbDos);
    assert!(error.message.contains("the FOR statement is incomplete"), "{}", error.message);
}

#[test]
fn a_wrong_argument_count_says_how_many_were_given_and_taken() {
    let (at, error) = refused("SUB Show\r\nEND SUB\r\nCALL Show(1)\r\n", Dialect::QuickBasic45);
    assert_eq!(at, "3:6");
    assert!(error.message.contains("`Show` is called with 1 argument, but takes no arguments"), "{}", error.message);
}

#[test]
fn a_mismatched_next_names_both_variables() {
    let (_, error) = refused("FOR i = 1 TO 3\r\nNEXT j\r\n", Dialect::QuickBasic45);
    assert!(
        error.message.contains("this NEXT names `j`, but the FOR loop it closes counts with `i`"),
        "{}",
        error.message
    );
}

#[test]
fn a_type_mismatch_says_what_each_side_is() {
    let (_, error) = refused("x = 5\r\nPRINT y$ + x\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("mixes a string with a number"), "{}", error.message);
}

#[test]
fn a_const_says_what_it_cannot_hold() {
    let (_, error) = refused("CONST k = 2 ^ 3\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("BC does not allow `^` there"), "{}", error.message);
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
    // The 12,898 programs are generated (corpus/programs.py), not kept.
    let directory = tempfile::tempdir().expect("creates a directory");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lalrspike/corpus/programs.py");
    let made = std::process::Command::new("python3").arg(&script).arg(directory.path()).status().expect("runs python3");
    assert!(made.success(), "programs.py failed");
    let entries = std::fs::read_dir(directory.path()).expect("the generated programs");
    let mut unworded = std::collections::BTreeMap::<String, String>::new();
    for entry in entries.filter_map(Result::ok) {
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        if entry.path().extension().is_none_or(|one| one != "bas") {
            continue;
        }
        let text = text.replace('\n', "\r\n");
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            if let Err(error) = llrm_qb::front::parse_coded(&text, dialect) {
                let worded = llrm_qb::front::explain::explain(error.clone(), &text);
                if !worded.message.contains(" [BC: ") || worded.message.starts_with(&format!("{} (at", error.code)) {
                    unworded.entry(error.code.clone()).or_insert_with(|| entry.path().display().to_string());
                }
            }
        }
    }
    assert!(unworded.is_empty(), "errors with no words in messages.txt (code: an example): {unworded:#?}");
}

#[test]
fn an_unclosed_block_names_the_block_and_its_closer() {
    let (at, error) = refused("x% = 1\r\nIF x% THEN\r\n  y% = 2\r\n", Dialect::QuickBasic45);
    assert_eq!(at, "2:1");
    assert!(error.message.contains("this block IF is never closed"), "{}", error.message);
    assert!(error.help.as_deref().is_some_and(|help| help.contains("END IF")), "{error:?}");
}

#[test]
fn a_closer_with_nothing_to_close_or_the_wrong_block_open_says_so() {
    let (_, error) = refused("NEXT\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("`NEXT` belongs to a FOR loop, but none is open here"), "{}", error.message);
    let (_, error) = refused("IF 1 THEN\r\nFOR i% = 1 TO 2\r\nEND IF\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("`END IF` comes while the FOR loop is still open"), "{}", error.message);
}

#[test]
fn a_constant_that_does_not_fit_says_the_type_and_range() {
    let (_, error) = refused("x% = 32768\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("does not fit an INTEGER variable (-32768 to 32767)"), "{}", error.message);
}

#[test]
fn a_parameter_mismatch_names_the_argument_and_both_types() {
    let source = "DECLARE SUB p (a%)\r\ns$ = \"a\"\r\nCALL p(s$)\r\nSUB p (a%)\r\nEND SUB\r\n";
    let (_, error) = refused(source, Dialect::QuickBasic45);
    assert!(
        error.message.contains("argument 1 of `p` is a string, but the parameter is declared as an INTEGER"),
        "{}",
        error.message
    );
}

#[test]
fn a_bad_line_in_a_type_says_how_a_field_is_written() {
    let (_, error) = refused("TYPE T\r\n  a% = 1\r\nEND TYPE\r\n", Dialect::QuickBasic45);
    assert!(error.message.contains("not a valid field of the TYPE"), "{}", error.message);
}
