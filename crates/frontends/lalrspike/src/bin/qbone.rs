//! Parse each file with the LALR grammar and print the verdict.
use qbfront::Dialect;

fn main() {
    let mut spans = Vec::new();
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        spans.clear();
        let verdict = lalrspike::parse_qb_stream(&source, Dialect::VbDos, &mut spans).map(|_| "ok".to_owned()).unwrap_or_else(|e| e.chars().take(150).collect());
        println!("{path}: {verdict}");
    }
}
