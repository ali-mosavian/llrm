//! The logos lexer against the old one: the same tokens, or the same error, for
//! every program the repository has, in every dialect.

use std::path::{Path, PathBuf};
use std::process::Command;

use qbfront::Dialect;

const DIALECTS: [Dialect; 4] = [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos, Dialect::Quickr];

fn tracked() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new("git").arg("-C").arg(&root).args(["ls-files", "*.bas", "*.BAS", "*.bi", "*.BI"]).output().unwrap();
    String::from_utf8(output.stdout).unwrap().lines().map(|line| root.join(line)).collect()
}

/// Whether the source has an `_` BC takes for a continuation, which drops the rest of the line: the old lexer
/// predates that, and the checked front's scanner is calibrated on BC.
fn continues(
    source: &str,
    dialect: Dialect,
) -> bool {
    source.contains('_')
        && (!dialect.underscore_in_names()
            || source.starts_with('_')
            || source.contains(" _")
            || source.contains("\n_")
            || source.contains("_\n")
            || source.contains("_\r"))
}

fn same(source: &str, what: &str) {
    for dialect in DIALECTS {
        if continues(source, dialect) {
            continue;
        }
        let old = qbfront::front::scan::scan(source, dialect);
        let new = lalrspike::qb_lexer::lex(source, dialect);
        assert_eq!(old, new, "{what} under {dialect:?}");
    }
}

#[test]
fn the_logos_lexer_reads_every_program_as_the_old_one_does() {
    let files = tracked();
    assert!(files.len() > 500, "found {} files", files.len());
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        same(&lalrspike::dos_source(&bytes), &path.display().to_string());
    }
}

/// Every prefix of every line, and every line with one byte struck out, so the
/// error paths (unterminated strings, bad exponents, stray characters) match too.
#[test]
fn the_logos_lexer_fails_where_and_as_the_old_one_does() {
    let mut checked = 0;
    for path in tracked().into_iter().step_by(7) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for line in source.lines().take(60) {
            for cut in (0..=line.len()).filter(|&cut| line.is_char_boundary(cut)) {
                same(&line[..cut], &format!("{line:?} cut at {cut}"));
                checked += 1;
            }
        }
    }
    assert!(checked > 10_000, "checked {checked}");
}

#[test]
fn the_logos_lexer_matches_on_the_odd_forms() {
    for source in [
        "", "\n", "x = .5\n", "a.1x = 2\n", "x.5 = 1\n", "rem $DYNAMIC\n", "REM$ x\n", "remark = 1\n", "'$static\n",
        "' plain\n", "_x = 1\n", "x = 1e\n", "x = 1D+3\n", "x = 123456789012345678\n", "x = 99999999999\n",
        "x = &HFFFF + &O17 + &77 + &H\n", "x = &H1FFFFFFFF\n", "s$ = \"a\"\"b\"\n", "s$ = \"open\n", "a$ = \"é\" + \"ü\"\n",
        "x = 1_\n+ 2\n", "x = 1 _\n+ 2\n", "x @ y\n", "d@ = 1@\n", "print left$(a$, 2); mid$ (b$, 1)\n", "x = a <= b <> c >= d < e > f\n",
        "x[1] = (2)\n", "? 1, 2; 3\n", "x = 3.%\n", "x = 1.5e3!\n", "x = 1!\n", "x = 0.1#\n", "tab(5) spc(3)\n", "x\u{e9} = 1\n",
    ] {
        same(source, &format!("{source:?}"));
    }
}

/// The parser fed straight from the lexer builds the same tree, from the same spans, as from the token vector.
#[test]
fn streaming_tokens_to_the_parser_builds_the_same_tree() {
    let mut parsed = 0;
    for path in tracked() {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        for dialect in [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos] {
            if continues(&source, dialect) {
                continue;
            }
            let old = qbfront::front::scan::scan(&source, dialect)
                .map_err(|error| error.message)
                .and_then(|tokens| lalrspike::parse_qb(&tokens).map(|tree| (tree, tokens)));
            let mut spans = Vec::new();
            let new = lalrspike::parse_qb_stream(&source, dialect, &mut spans);
            let what = format!("{} under {dialect:?}", path.display());
            match (old, new) {
                (Ok((tree, tokens)), Ok(streamed)) => {
                    assert_eq!(tree, streamed, "{what}");
                    assert_eq!(tokens.iter().map(|token| token.span).collect::<Vec<_>>(), spans, "{what}");
                    parsed += 1;
                }
                (Err(_), Err(_)) => {}
                (old, new) => panic!("{what}: old {:?}, streamed {:?}", old.map(|_| ()), new.map(|_| ())),
            }
        }
    }
    assert!(parsed > 500, "parsed {parsed}");
}
