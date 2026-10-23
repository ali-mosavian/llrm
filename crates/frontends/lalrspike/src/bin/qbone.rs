//! Parse each file with the LALR grammar and print the verdict.
use qbfront::Dialect;

fn main() {
    let typed = std::env::var("QB_PARSER").is_ok_and(|v| v == "typed");
    let mut spans = Vec::new();
    for path in std::env::args().skip(1) {
        let source = lalrspike::dos_source(&std::fs::read(&path).unwrap());
        spans.clear();
        let verdict = if typed {
            lalrspike::qb_ast::parse(&source, Dialect::VbDos).map(|_| "ok".to_owned()).unwrap_or_else(|e| format!("{}:{} {}", e.span.line, e.span.start, e.message))
        } else {
            lalrspike::parse_qb_stream(&source, Dialect::VbDos, &mut spans).map(|_| "ok".to_owned()).unwrap_or_else(|e| e.chars().take(150).collect())
        };
        println!("{path}: {verdict}");
    }
}
