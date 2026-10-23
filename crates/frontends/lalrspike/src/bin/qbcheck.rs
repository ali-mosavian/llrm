//! Parse each given `.bas` file with the LALR spike and the table-driven
//! parser, and report where they disagree.

use qbfront::Dialect;
use qbfront::generated_parser::lex;

const DIALECTS: [Dialect; 3] = [Dialect::QuickBasic45, Dialect::VbDos, Dialect::Quickr];

/// `QB_GENERATED=1` runs the grammar generated from the recovered one.
fn parse(tokens: &[qbfront::generated_parser::Token]) -> Result<lalrspike::qb_tree::Tree, String> {
    if std::env::var_os("QB_GENERATED").is_some() { lalrspike::parse_qb(tokens) } else { lalrspike::parse_qb_by_hand(tokens) }
}

fn main() {
    let (mut both, mut neither) = (0, 0);
    let (mut only_hand, mut only_lalr) = (Vec::new(), Vec::new());
    for path in std::env::args().skip(1) {
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        let dialect = DIALECTS.into_iter().find(|&dialect| qbfront::parse(&source, dialect).is_ok());
        let lalr = DIALECTS
            .into_iter()
            .filter(|&d| dialect.is_none_or(|accepted| accepted == d))
            .find_map(|d| lex(&source, d).ok().map(|tokens| parse(&tokens)));
        match (dialect, lalr) {
            (Some(_), Some(Ok(_))) => both += 1,
            (None, Some(Err(_)) | None) => neither += 1,
            (Some(d), Some(Err(error))) => only_hand.push(format!("{path} [{d:?}]: {error}")),
            (Some(d), None) => only_hand.push(format!("{path} [{d:?}]: lexer")),
            (None, Some(Ok(_))) => only_lalr.push(path),
        }
    }
    println!("accepted by both: {both}, rejected by both: {neither}");
    println!("table-driven only: {}", only_hand.len());
    only_hand.iter().for_each(|line| println!("  {}", line.chars().take(260).collect::<String>()));
    println!("LALR only: {}", only_lalr.len());
    only_lalr.iter().for_each(|line| println!("  {line}"));
}
