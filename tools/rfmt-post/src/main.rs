//! `rfmt-post [--pre] [--stats] FILE...`: rewrite rustfmt output in place.
//! `--pre` only does the steps before rustfmt's second run (long `matches!`
//! broken, long trailing comments moved up); `--stats` prints what changed.
//! `rfmt-post [--pre] --stdin`: the same on stdin, the result on stdout;
//! nothing is written when it fails.
//! `rfmt-post --long-comments [--all] --stdin`: lists the lines of comments
//! past column 80 that nothing can break and are not exempt (`--all`: the
//! exempt ones too, with why); exit 1 if there are any of the former.

use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Totals {
    changed: usize,
    chains: usize,
    too_wide: usize,
    matches: usize,
    hoisted: usize,
    wrapped: usize,
    failed: bool,
}

fn transform(
    text: &str,
    pre: bool,
) -> Result<(String, rfmt_post::Stats), String> {
    match pre {
        true => rfmt_post::pre(text),
        false => rfmt_post::format(text),
    }
}

fn run(
    path: &str,
    pre: bool,
    totals: &Mutex<Totals>,
) {
    let text = std::fs::read_to_string(path).expect("readable file");
    let result = transform(&text, pre);
    let mut totals = totals.lock().unwrap();
    match result {
        Ok((out, stats)) => {
            totals.chains += stats.chains;
            totals.too_wide += stats.too_wide;
            totals.matches += stats.matches;
            totals.hoisted += stats.hoisted;
            totals.wrapped += stats.wrapped;
            if out != text {
                std::fs::write(path, out).expect("writable file");
                totals.changed += 1;
            }
        }
        Err(error) => {
            eprintln!("{path}: {error}; left as rustfmt wrote it");
            totals.failed = true;
        }
    }
}

fn long_comments(all: bool) -> ExitCode {
    let mut text = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("<stdin>: {error}");
        return ExitCode::FAILURE;
    }
    let found = match rfmt_post::scan(&text) {
        Ok(found) => found,
        Err(error) => {
            eprintln!("<stdin>: {error}");
            return ExitCode::from(2);
        }
    };
    let mut bad = false;
    for long in found.iter().filter(|l| all || !l.kind.exempt()) {
        println!("{}:{:?}", long.line, long.kind);
        bad |= !long.kind.exempt();
    }
    if bad { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn filter(pre: bool) -> ExitCode {
    let mut text = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("<stdin>: {error}");
        return ExitCode::FAILURE;
    }
    match transform(&text, pre) {
        Ok((out, _)) => {
            std::io::stdout().write_all(out.as_bytes()).expect("writable stdout");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("<stdin>: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.remove(i)).is_some();
    let (stats, pre, stdin) = (flag("--stats"), flag("--pre"), flag("--stdin"));
    if flag("--long-comments") {
        return long_comments(flag("--all"));
    }
    if stdin {
        return filter(pre);
    }
    let (next, totals) = (AtomicUsize::new(0), Mutex::new(Totals::default()));
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(1, |n| n.get()) {
            scope.spawn(|| {
                while let Some(path) = args.get(next.fetch_add(1, Ordering::Relaxed)) {
                    run(path, pre, &totals);
                }
            });
        }
    });
    let totals = totals.into_inner().unwrap();
    if stats {
        eprintln!(
            "{} files changed, {} chains split, {} left (line too wide), {} matches! broken, {} comments moved up, {} \
             broken",
            totals.changed, totals.chains, totals.too_wide, totals.matches, totals.hoisted, totals.wrapped
        );
    }
    if totals.failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
