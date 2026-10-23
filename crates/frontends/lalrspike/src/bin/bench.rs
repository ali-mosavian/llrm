//! Parse time: the existing parsers against the lalrpop ones, on the files both take.
//!
//!     bench FILE...        (.nib and .bas; build with --release)

use std::time::{Duration, Instant};

use qbfront::Dialect;

const ROUNDS: u32 = 20;

fn time<T>(rounds: u32, mut work: impl FnMut() -> T) -> Duration {
    let started = Instant::now();
    for _ in 0..rounds {
        std::hint::black_box(work());
    }
    started.elapsed() / rounds
}

fn report(name: &str, files: usize, bytes: usize, rows: &[(&str, Duration)]) {
    println!("{name}: {files} files, {} KB", bytes / 1024);
    for (what, took) in rows {
        let seconds = took.as_secs_f64();
        println!("  {what:<28} {:>9.2} ms   {:>7.1} MB/s", seconds * 1e3, bytes as f64 / seconds / 1e6);
    }
}

fn nib(paths: &[String]) {
    let sources: Vec<String> = paths.iter().filter(|p| p.ends_with(".nib")).filter_map(|p| std::fs::read_to_string(p).ok()).collect();
    let both: Vec<String> = sources
        .into_iter()
        .filter(|s| llrm_nib::lexer::lex(s).is_ok_and(|t| llrm_nib::parser::parse(t.clone()).is_ok() && lalrspike::parse_nib(&t).is_ok()))
        .collect();
    let bytes = both.iter().map(String::len).sum();
    let lex = time(ROUNDS, || both.iter().map(|s| llrm_nib::lexer::lex(s).unwrap().len()).sum::<usize>());
    let tokens: Vec<_> = both.iter().map(|s| llrm_nib::lexer::lex(s).unwrap()).collect();
    let hand = time(ROUNDS, || tokens.iter().map(|t| llrm_nib::parser::parse(t.clone()).is_ok()).filter(|ok| *ok).count());
    let lalr = time(ROUNDS, || tokens.iter().map(|t| lalrspike::parse_nib(t).is_ok()).filter(|ok| *ok).count());
    report("Nib", both.len(), bytes, &[("lexer", lex), ("hand-written parser", hand), ("lalrpop parser", lalr)]);
}

fn qb(paths: &[String]) {
    let dialect = Dialect::VbDos;
    let sources: Vec<String> = paths.iter().filter(|p| p.to_lowercase().ends_with(".bas")).filter_map(|p| std::fs::read(p).ok()).map(|b| lalrspike::dos_source(&b)).collect();
    let both: Vec<String> = sources
        .into_iter()
        .filter(|s| qbfront::parse(s, dialect).is_ok() && qbfront::generated_parser::lex(s, dialect).is_ok_and(|t| lalrspike::parse_qb(&t).is_ok()))
        .collect();
    let bytes = both.iter().map(String::len).sum();
    let lex = time(ROUNDS, || both.iter().map(|s| qbfront::generated_parser::lex(s, dialect).unwrap().len()).sum::<usize>());
    let logos = time(ROUNDS, || both.iter().map(|s| lalrspike::qb_lexer::lex(s, dialect).unwrap().len()).sum::<usize>());
    let old = time(ROUNDS, || both.iter().filter(|s| qbfront::parse(s, dialect).is_ok()).count());
    let tokens: Vec<_> = both.iter().map(|s| qbfront::generated_parser::lex(s, dialect).unwrap()).collect();
    let lalr = time(ROUNDS, || tokens.iter().filter(|t| lalrspike::parse_qb(t).is_ok()).count());
    let vector = time(ROUNDS, || both.iter().filter(|s| lalrspike::parse_qb(&qbfront::generated_parser::lex(s, dialect).unwrap()).is_ok()).count());
    let mut spans = Vec::new();
    let streamed = time(ROUNDS, || {
        both.iter().filter(|s| {
            spans.clear();
            lalrspike::parse_qb_stream(s, dialect, &mut spans).is_ok()
        }).count()
    });
    report("QB", both.len(), bytes, &[
        ("old lexer + lalrpop", vector),
        ("logos streamed + lalrpop", streamed),("lexer", lex), ("logos lexer", logos), ("table-driven lex+parse+AST", old), ("lalrpop parser (no lex)", lalr)]);
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    nib(&paths);
    qb(&paths);
}
