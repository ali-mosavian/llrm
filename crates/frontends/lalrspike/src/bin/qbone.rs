//! Parse each file with the LALR grammar and print the verdict.
use qbfront::Dialect;
use qbfront::generated_parser::lex;

fn main() {
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        let verdict = match lex(&source, Dialect::VbDos) {
            Err(error) => format!("lexer: {}", error.message),
            Ok(tokens) => lalrspike::parse_qb(&tokens).map(|_| "ok".to_owned()).unwrap_or_else(|e| e.chars().take(150).collect()),
        };
        println!("{path}: {verdict}");
    }
}
