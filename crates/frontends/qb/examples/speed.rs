//! Parse and check speed: `speed DIALECT FILE...` reports lex, parse and
//! full-front time over the files.
use std::time::Instant;

use llrm_qb::{Dialect, front, source};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = Dialect::parse(&args[0]).expect("a dialect");
    let texts: Vec<String> = args[1..]
        .iter()
        .filter_map(|p| source::load_with_map(std::path::Path::new(p), &[]).ok())
        .map(|l| l.text)
        .filter(|t| front::parse(t, dialect).is_ok())
        .collect();
    let bytes: usize = texts.iter().map(String::len).sum();
    let lines: usize = texts.iter().map(|t| t.lines().count()).sum();
    let rounds = 20;
    let time = |f: &dyn Fn(&str)| {
        let start = Instant::now();
        for _ in 0..rounds {
            texts.iter().for_each(|t| f(t));
        }
        start.elapsed().as_secs_f64() / rounds as f64
    };
    let lex = time(&|t| drop(front::scan::scan(t, dialect)));
    let ast = time(&|t| drop(front::ast::parse(t, dialect)));
    let all = time(&|t| drop(front::parse(t, dialect)));
    println!("{} programs, {lines} lines, {bytes} bytes", texts.len());
    for (name, s) in [("lex", lex), ("lex+parse", ast), ("lex+parse+check", all)] {
        println!(
            "{name:16} {:7.2} ms  {:6.2} MB/s  {:7.0} klines/s",
            s * 1e3,
            bytes as f64 / s / 1e6,
            lines as f64 / s / 1e3
        );
    }
}
